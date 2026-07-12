//! 평가 하네스 오케스트레이터 (스펙 §8, 설계 2026-07-03).
//! 인프로세스로 Agent::run을 호출한다 — 가짜 LlmClient로 서버 없이 테스트 가능.

pub mod report;
pub mod sandbox;
pub mod task;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Context;

use crate::agent::approval::AutoApprover;
use crate::agent::bounded::{run_bounded, watch_flag, Stopped};
use crate::agent::{Agent, AgentEvent, AgentOutcome};
use crate::config::Config;
use crate::llm::LlmClient;
use crate::session::{now_secs, utc_stamp, Session, Transcript};
use crate::tools::exec::{exec_shell, ExecEnd};
use crate::tools::{Registry, ToolCtx};
use report::{Report, RunOutcome, RunRecord, TaskReport};
use sandbox::Sandbox;
use task::Task;

pub struct EvalOptions {
    pub tasks_dir: PathBuf,
    pub repeats: usize,
    pub base_seed: u64,
    pub timeout_scale: f64,
    /// 취소 신호 후 자연 종료 유예 — CLI 기본 5초, 테스트가 줄인다
    pub cancel_grace: Duration,
}

#[derive(Debug)]
pub struct EvalRun {
    pub report: Report,
    pub report_path: PathBuf,
}

pub async fn run_eval<C: LlmClient>(
    client: &C,
    config: &Config,
    model: &str,
    opts: &EvalOptions,
    project_root: &Path,
) -> anyhow::Result<EvalRun> {
    let tasks = task::load_tasks(&opts.tasks_dir)?;
    let started = Instant::now();
    let started_at = utc_stamp(now_secs());
    let report_dir = create_report_dir(project_root, &started_at)?;

    // 장수 SIGINT 리스너 + 공유 플래그. tokio의 ctrl_c()는 첫 폴링에서 프로세스
    // 기본 SIGINT 동작을 영구 대체하고 등록 이후의 신호만 보므로, select! 창 밖
    // 구간(샌드박스 복사·protected 동기화·check 실행)의 Ctrl+C가 유실되지 않게
    // 리스너 하나를 계속 살려 두고 플래그로 전달한다
    let interrupt = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let listener = tokio::spawn({
        let flag = interrupt.clone();
        async move {
            while tokio::signal::ctrl_c().await.is_ok() {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
    });

    let mut task_reports = Vec::new();
    let mut interrupted = false;
    // run_once의 에러(LLM 에러 → 하네스 중단)로 조기 반환해도 리스너가 정리되도록
    // 루프를 블록으로 감싸 결과를 받은 뒤 abort → 전파 순서로 처리한다
    let loop_result: anyhow::Result<()> = async {
        for t in &tasks {
            if interrupted {
                break;
            }
            let mut runs = Vec::new();
            for repeat in 0..opts.repeats {
                // 페이즈 간(직전 run의 판정/정리 중) 들어온 Ctrl+C도 여기서 잡는다
                if interrupt.load(std::sync::atomic::Ordering::SeqCst) {
                    eprintln!("(중단됨 — 지금까지의 결과로 부분 리포트를 만듭니다)");
                    interrupted = true;
                    break;
                }
                let seed = opts.base_seed + repeat as u64;
                eprintln!("[{}] {}/{} 실행 중… (시드 {seed})", t.name, repeat + 1, opts.repeats);
                match run_once(client, config, model, t, seed, repeat, opts, &report_dir, &interrupt).await? {
                    Some(rec) => {
                        eprintln!(
                            "[{}] {}/{} — {} ({:?}, {}턴, {:.1}s)",
                            t.name, repeat + 1, opts.repeats,
                            if rec.passed { "통과" } else { "실패" },
                            rec.outcome, rec.turns, rec.duration_secs,
                        );
                        runs.push(rec);
                    }
                    None => {
                        eprintln!("(중단됨 — 지금까지의 결과로 부분 리포트를 만듭니다)");
                        interrupted = true;
                        break;
                    }
                }
            }
            task_reports.push(TaskReport::from_runs(t.name.clone(), runs));
        }
        Ok(())
    }
    .await;
    listener.abort();
    loop_result?;

    let report = Report {
        model: model.to_string(),
        base_seed: opts.base_seed,
        repeats: opts.repeats,
        timeout_scale: opts.timeout_scale,
        started_at,
        duration_secs: started.elapsed().as_secs_f64(),
        interrupted,
        total_pass_rate: Report::total_of(&task_reports),
        tasks: task_reports,
    };
    let report_path = report_dir.join("report.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("리포트 저장 실패: {}", report_path.display()))?;
    Ok(EvalRun { report, report_path })
}

/// 과제 1회 실행 → 판정. Ok(None) = Ctrl+C (하네스 중단 신호)
#[allow(clippy::too_many_arguments)]
async fn run_once<C: LlmClient>(
    client: &C,
    config: &Config,
    model: &str,
    t: &Task,
    seed: u64,
    repeat: usize,
    opts: &EvalOptions,
    report_dir: &Path,
    interrupt: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> anyhow::Result<Option<RunRecord>> {
    let mut cfg = config.clone();
    if let Some(mt) = t.spec.max_turns {
        cfg.max_turns = mt;
    }
    // eval은 --auto 의미 — config의 auto_deny_patterns 적용 (스펙 §5·§8)
    let mut approver = AutoApprover::new(&cfg.auto_deny_patterns)?;
    let sb = Sandbox::create(&t.fixture)?;
    let mut ctx = ToolCtx::new(sb.root.clone());
    ctx.command_timeout = Duration::from_secs(cfg.command_timeout_secs);
    let cancel = ctx.cancel.clone();
    let mut agent = Agent::new(client, Registry::guided(), ctx, model.to_string(), &cfg);
    agent.set_seed(seed);
    let transcript_path = report_dir.join(format!("run-{}-{repeat}.jsonl", t.name));
    let transcript = Transcript::create_at(&transcript_path).unwrap_or_else(|e| {
        eprintln!("(실행 기록 파일을 열지 못했습니다: {e} — 기록 없이 진행)");
        Transcript::disabled()
    });
    let mut session = Session::new(agent.initial_history(), transcript);
    let mut turns = 0usize;
    let mut on_event = |ev: AgentEvent<'_>| {
        // 턴 수 = 파싱된 턴(Thought) 수 — 패킹 절삭과 무관하게 정확 (설계 결정)
        if matches!(ev, AgentEvent::Thought(_)) {
            turns += 1;
        }
    };
    let limit = scaled_timeout(t.spec.timeout_secs, opts.timeout_scale);
    let start = Instant::now();
    let bounded = run_bounded(
        agent.run(&mut session, &t.spec.prompt, &mut approver, &mut on_event),
        &cancel,
        Some(limit),
        opts.cancel_grace,
        watch_flag(interrupt),
    )
    .await;
    let elapsed = start.elapsed();
    let outcome = match bounded {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            sb.cleanup();
            // 서버 다운 등 — 남은 과제를 도는 건 무의미, 하네스 에러로 전파 (설계 결정)
            return Err(anyhow::Error::new(e).context(format!("과제 {} 실행 중 LLM 에러 — 하네스 중단", t.name)));
        }
        Err(Stopped::Interrupted) => {
            sb.cleanup();
            return Ok(None);
        }
        Err(Stopped::TimedOut) => {
            let rec = judge(&sb, t, opts, RunOutcome::Timeout, turns, elapsed, seed, repeat, interrupt).await;
            sb.cleanup(); // judge 에러 경로에서도 샌드박스를 정리한 뒤 전파
            return rec;
        }
    };
    let kind = match outcome {
        AgentOutcome::Finished(_) => RunOutcome::Finished,
        AgentOutcome::MaxTurns => RunOutcome::MaxTurns,
        AgentOutcome::RepetitionStop => RunOutcome::RepetitionStop,
        AgentOutcome::ParseFailed(_) => RunOutcome::ParseFailed,
        // eval에선 도달하지 않음(플래그는 run_bounded만 세우고 그 경로는 위에서 반환) — 방어적 매핑
        AgentOutcome::Cancelled => RunOutcome::Timeout,
    };
    let rec = judge(&sb, t, opts, kind, turns, elapsed, seed, repeat, interrupt).await;
    sb.cleanup(); // judge 에러 경로에서도 샌드박스를 정리한 뒤 전파
    rec
}

/// protected 동기화(check보다 먼저 — 스펙 §8) 후 check 종료코드로 판정.
/// outcome과 무관하게 항상 실행된다 (MaxTurns라도 작업이 됐으면 통과가 공정).
/// Ok(None) = check 실행 중 Ctrl+C — 잘린 판정은 기록하지 않는다 (측정 오염 방지)
#[allow(clippy::too_many_arguments)]
async fn judge(
    sb: &Sandbox,
    t: &Task,
    opts: &EvalOptions,
    outcome: RunOutcome,
    turns: usize,
    elapsed: Duration,
    seed: u64,
    repeat: usize,
    interrupt: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> anyhow::Result<Option<RunRecord>> {
    sb.sync_protected(&t.fixture, &t.spec.protected)?;
    let check_timeout = scaled_timeout(t.spec.check_timeout_secs, opts.timeout_scale);
    let check = t.spec.check.clone();
    let root = sb.root.clone();
    // exec_shell은 블로킹(폴링 루프) — 런타임과 인터럽트 리스너를 막지 않게 워커로.
    // cancel로 공유 인터럽트 플래그를 넘겨 check 실행 중 Ctrl+C도 프로세스 그룹을 죽인다
    let cancel = interrupt.clone();
    let exec = tokio::task::spawn_blocking(move || exec_shell(&check, &root, check_timeout, &cancel))
        .await
        .context("check 실행 태스크가 패닉")?
        .with_context(|| format!("과제 {}: check 명령 실행 실패", t.name))?;
    if matches!(exec.end, ExecEnd::Cancelled) {
        return Ok(None);
    }
    let passed = matches!(exec.end, ExecEnd::Done(s) if s.success());
    Ok(Some(RunRecord { repeat, seed, passed, outcome, turns, duration_secs: elapsed.as_secs_f64() }))
}

/// `<root>/.loco/eval/<stamp>/` 생성 + `.loco/.gitignore` 보장 (스펙 §7과 동일 정책)
fn create_report_dir(root: &Path, stamp: &str) -> anyhow::Result<PathBuf> {
    let base = root.join(".loco/eval");
    std::fs::create_dir_all(&base)?;
    let gitignore = root.join(".loco/.gitignore");
    if !gitignore.exists() {
        std::fs::write(&gitignore, "*\n")?;
    }
    for suffix in 0..10 {
        let name = if suffix == 0 { stamp.to_string() } else { format!("{stamp}-{suffix}") };
        let dir = base.join(&name);
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    anyhow::bail!("리포트 디렉터리 이름 충돌이 반복됨")
}

/// timeout × scale — 포화 + 상한 3600초. from_secs_f64는 비유한/음수/오버플로에서
/// 패닉하므로(스펙 M5 §4.2) 유한성 검사 후 클램프한다
fn scaled_timeout(secs: u64, scale: f64) -> Duration {
    const MAX_SECS: f64 = 3600.0;
    let v = secs as f64 * scale;
    let v = if v.is_finite() { v.clamp(0.0, MAX_SECS) } else { MAX_SECS };
    Duration::from_secs_f64(v)
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn scaled_timeout_saturates_and_clamps() {
        assert_eq!(scaled_timeout(300, 1.0), Duration::from_secs(300));
        assert_eq!(scaled_timeout(300, 2.0), Duration::from_secs(600));
        // 상한 3600초 — 거대 값·비유한 배율이 from_secs_f64 패닉을 일으키지 않는다 (스펙 §4.2)
        assert_eq!(scaled_timeout(u64::MAX, 1.0), Duration::from_secs(3600));
        assert_eq!(scaled_timeout(300, f64::INFINITY), Duration::from_secs(3600));
        assert_eq!(scaled_timeout(300, f64::NAN), Duration::from_secs(3600));
        assert_eq!(scaled_timeout(300, -1.0), Duration::from_secs(0));
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::llm::client::LlmError;
    use crate::llm::types::{ChatRequest, ChatResponse, Choice, ResponseMessage};
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct Scripted {
        responses: Mutex<VecDeque<Result<ChatResponse, LlmError>>>,
    }

    impl Scripted {
        fn new(responses: Vec<Result<ChatResponse, LlmError>>) -> Self {
            Self { responses: Mutex::new(responses.into()) }
        }
    }

    impl LlmClient for Scripted {
        async fn chat(&self, _req: &ChatRequest) -> Result<ChatResponse, LlmError> {
            self.responses.lock().unwrap().pop_front().expect("스크립트 소진")
        }
    }

    /// 항상 오래 자는 클라이언트 — 타임아웃 경로 테스트용
    struct Sleepy;
    impl LlmClient for Sleepy {
        async fn chat(&self, _req: &ChatRequest) -> Result<ChatResponse, LlmError> {
            tokio::time::sleep(Duration::from_secs(30)).await;
            Err(LlmError::Api { status: 500, body: "unreachable".into() })
        }
    }

    fn ok(text: &str) -> Result<ChatResponse, LlmError> {
        Ok(ChatResponse {
            choices: vec![Choice {
                message: ResponseMessage { role: "assistant".into(), content: Some(text.into()) },
                finish_reason: Some("stop".into()),
            }],
        })
    }

    fn turn(tool: &str, args: serde_json::Value) -> String {
        serde_json::json!({"thought": "t", "action": {"tool": tool, "args": args}}).to_string()
    }

    fn finish(summary: &str) -> String {
        turn("finish", serde_json::json!({"summary": summary}))
    }

    fn write_task(root: &Path, name: &str, toml: &str, files: &[(&str, &str)]) {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("fixture")).unwrap();
        std::fs::write(dir.join("task.toml"), toml).unwrap();
        for (rel, content) in files {
            let p = dir.join("fixture").join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
    }

    fn opts(tasks_dir: PathBuf) -> EvalOptions {
        EvalOptions {
            tasks_dir,
            repeats: 1,
            base_seed: 0,
            timeout_scale: 1.0,
            cancel_grace: Duration::from_millis(100),
        }
    }

    #[tokio::test]
    async fn pass_flow_syncs_protected_before_check() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "demo",
            r#"
prompt = "answer.txt를 만들어라"
check = "test -f answer.txt && grep -q ORIGINAL data/expected.txt && test ! -f data/extra.txt"
protected = ["data"]
"#,
            &[("data/expected.txt", "ORIGINAL\n")],
        );
        // 보상 해킹 포함 시나리오: 정답 파일 + protected 수정 + protected에 파일 추가
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "answer.txt", "content": "42"}))),
            ok(&turn("write_file", serde_json::json!({"path": "data/expected.txt", "content": "HACKED"}))),
            ok(&turn("write_file", serde_json::json!({"path": "data/extra.txt", "content": "sneak"}))),
            ok(&finish("done")),
        ]);
        let config = Config::default();
        let o = opts(tasks.path().to_path_buf());
        let run = run_eval(&script, &config, "test-model", &o, proj.path()).await.unwrap();

        assert!(!run.report.interrupted);
        assert_eq!(run.report.tasks.len(), 1);
        let t = &run.report.tasks[0];
        assert_eq!(t.name, "demo");
        assert_eq!(t.pass_rate, 1.0, "protected 동기화가 check 전에 일어나야 통과: {:?}", t.runs);
        assert_eq!(t.runs[0].outcome, RunOutcome::Finished);
        assert_eq!(t.runs[0].turns, 4, "Thought 4회 (툴 3 + finish 1)");
        // 산출물: report.json + 실행별 트랜스크립트, .gitignore
        assert!(run.report_path.exists());
        assert!(run.report_path.parent().unwrap().join("run-demo-0.jsonl").exists());
        assert_eq!(std::fs::read_to_string(proj.path().join(".loco/.gitignore")).unwrap().trim(), "*");
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&run.report_path).unwrap()).unwrap();
        assert_eq!(json["total_pass_rate"], 1.0);
    }

    #[tokio::test]
    async fn failed_check_and_per_repeat_seeds() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "demo",
            "prompt = \"p\"\ncheck = \"test -f answer.txt\"\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        // 두 반복 모두 아무 작업 없이 finish → check 실패
        let script = Scripted::new(vec![ok(&finish("없음")), ok(&finish("없음"))]);
        let config = Config::default();
        let mut o = opts(tasks.path().to_path_buf());
        o.repeats = 2;
        o.base_seed = 10;
        let run = run_eval(&script, &config, "m", &o, proj.path()).await.unwrap();
        let t = &run.report.tasks[0];
        assert_eq!(t.pass_rate, 0.0);
        assert_eq!(t.runs[0].seed, 10, "base_seed + 0");
        assert_eq!(t.runs[1].seed, 11, "base_seed + 1 — 반복마다 다른 시드 (스펙 §8)");
    }

    #[tokio::test]
    async fn timeout_is_recorded_and_scaled() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "slow",
            "prompt = \"p\"\ncheck = \"test -f never.txt\"\ntimeout_secs = 1\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        let mut o = opts(tasks.path().to_path_buf());
        o.timeout_scale = 0.05; // 1초 × 0.05 = 50ms — 스케일 배선도 함께 검증
        let config = Config::default();
        let start = Instant::now();
        let run = run_eval(&Sleepy, &config, "m", &o, proj.path()).await.unwrap();
        assert!(start.elapsed() < Duration::from_secs(5), "grace(100ms) 후 즉시 진행");
        let r = &run.report.tasks[0].runs[0];
        assert_eq!(r.outcome, RunOutcome::Timeout);
        assert!(!r.passed);
    }

    #[tokio::test]
    async fn llm_error_aborts_the_harness() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "demo",
            "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        let script = Scripted::new(vec![Err(LlmError::Api { status: 500, body: "down".into() })]);
        let config = Config::default();
        let err = run_eval(&script, &config, "m", &opts(tasks.path().to_path_buf()), proj.path())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("하네스 중단"), "{err:#}");
    }

    #[tokio::test]
    async fn task_max_turns_overrides_config() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "short",
            "prompt = \"p\"\ncheck = \"true\"\nmax_turns = 1\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        // 응답 1개(툴 턴)만 스크립트 — max_turns=1이 적용되면 두 번째 호출 없이 MaxTurns
        let script = Scripted::new(vec![ok(&turn("read_file", serde_json::json!({"path": "nope.txt"})))]);
        let config = Config::default();
        let run = run_eval(&script, &config, "m", &opts(tasks.path().to_path_buf()), proj.path()).await.unwrap();
        let r = &run.report.tasks[0].runs[0];
        assert_eq!(r.outcome, RunOutcome::MaxTurns);
        assert!(r.passed, "check(true)는 outcome과 무관하게 실행·통과");
    }
}
