//! 평가 하네스 오케스트레이터 (스펙 §8, 설계 2026-07-03).
//! 인프로세스로 Agent::run을 호출한다 — 가짜 LlmClient로 서버 없이 테스트 가능.

pub mod integrity;
pub mod procure;
pub mod report;
pub mod sandbox;
pub mod task;
pub mod verify;

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
use report::{EffectiveConfig, Report, RunOutcome, RunRecord, TaskReport};
use sandbox::Sandbox;
use task::Task;

pub struct EvalOptions {
    pub tasks_dir: PathBuf,
    pub repeats: usize,
    pub base_seed: u64,
    pub timeout_scale: f64,
    /// 취소 신호 후 자연 종료 유예 — CLI 기본 5초, 테스트가 줄인다
    pub cancel_grace: Duration,
    /// 과제 이름 정확 일치 필터 — 빈 벡터면 전체 실행 (M10 §7-1)
    pub filters: Vec<String>,
}

#[derive(Debug)]
pub struct EvalRun {
    pub report: Report,
    pub report_path: PathBuf,
}

/// Eval force for M16 `repo_notes`: only basename `tasks-real` keeps the config
/// value; every other tasks_dir (tasks, tasks-large, tempdirs, …) is forced off.
pub fn apply_eval_repo_notes_policy(tasks_dir: &Path, cfg: &mut Config) {
    let is_real = tasks_dir.file_name().and_then(|s| s.to_str()) == Some("tasks-real");
    if !is_real {
        cfg.repo_notes = false;
    }
}

pub async fn run_eval<C: LlmClient>(
    client: &C,
    config: &Config,
    model: &str,
    opts: &EvalOptions,
    project_root: &Path,
) -> anyhow::Result<EvalRun> {
    // M16: one post-policy clone for Agent, registry, and EffectiveConfig (same cfg).
    let mut cfg = config.clone();
    apply_eval_repo_notes_policy(&opts.tasks_dir, &mut cfg);

    let tasks = task::filter_tasks(task::load_tasks(&opts.tasks_dir)?, &opts.filters)?;
    let started = Instant::now();
    let started_at = utc_stamp(now_secs());
    let report_dir = create_report_dir(project_root, &started_at)?;

    // M7 §5: 샌드박스 밖 cargo config 변조 감지 — 시작 시 1회 스냅샷, 매 런 check 전 비교
    let cargo_home = integrity::resolve_cargo_home();
    if cargo_home.is_none() {
        eprintln!("(CARGO_HOME을 해석할 수 없어 해당 감시를 생략합니다 — env 미설정·홈 없음)");
    }
    let cargo_snapshot =
        integrity::CargoConfigSnapshot::take(cargo_home.as_deref(), &std::env::temp_dir());

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
                match run_once(client, &cfg, model, t, seed, repeat, opts, &report_dir, &interrupt, &cargo_snapshot).await? {
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
            // M15 H11: task_dir = fixture의 부모. H8이 `<task_dir>/fixture`를
            // 실체화하므로 이 관계가 두 트리 모두에서 참이다(H3가 불요한 이유)
            let task_dir = t.fixture.parent().expect("fixture는 과제 디렉터리 바로 아래");
            let procure = procure::load(task_dir)?;
            task_reports.push(TaskReport::from_runs(t.name.clone(), runs, procure));
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
        passed_count: task_reports.iter().map(|t| t.passed_count).sum(),
        passed_strict_count: task_reports.iter().map(|t| t.passed_strict_count).sum(),
        false_finish_count: task_reports.iter().map(|t| t.false_finish_count).sum(),
        schema_fallback_count: task_reports.iter().map(|t| t.schema_fallback_count).sum(),
        avg_duration_secs: Report::avg_duration_of(&task_reports),
        tasks: task_reports,
        effective_config: EffectiveConfig {
            base_url: cfg.base_url.clone(),
            temperature: cfg.temperature,
            context_tokens: cfg.context_tokens,
            max_output_tokens: cfg.max_output_tokens,
            max_turns: cfg.max_turns,
            command_timeout_secs: cfg.command_timeout_secs,
            loco_version: env!("CARGO_PKG_VERSION").to_string(),
            repo_notes: cfg.repo_notes,
        },
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
    cargo_snapshot: &integrity::CargoConfigSnapshot,
) -> anyhow::Result<Option<RunRecord>> {
    let mut cfg = config.clone();
    if let Some(mt) = t.spec.max_turns {
        cfg.max_turns = mt;
    }
    // M15 H1·H2 — 과제별 오버라이드는 **ToolCtx·Agent 생성 전에** 전부 적용한다.
    // 아래 ctx.command_timeout과 Agent::new가 이 cfg를 읽으므로 순서가 계약이다
    if let Some(ct) = t.spec.context_tokens {
        cfg.context_tokens = ct;
    }
    if let Some(cts) = t.spec.command_timeout_secs {
        cfg.command_timeout_secs = cts;
    }
    // eval은 --auto 의미 — config의 auto_deny_patterns 적용 (스펙 §5·§8)
    let mut approver = AutoApprover::new(&cfg.auto_deny_patterns)?;
    let sb = Sandbox::create(&t.fixture)?;
    let mut ctx = ToolCtx::new(sb.root.clone());
    ctx.command_timeout = Duration::from_secs(cfg.command_timeout_secs);
    let cancel = ctx.cancel.clone();
    let mut agent = Agent::new(client, Registry::guided(cfg.repo_notes), ctx, model.to_string(), &cfg);
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
    // 폴백 발동 여부는 여기서 **한 번만** 읽는다. 아래 judge 호출이 두 갈래
    // (타임아웃 / 정상)인데 각각에서 게터를 부르면 한쪽만 배선이 끊겨도
    // 테스트가 안 죽는 지점이 생긴다 — 실제로 그랬다(정상 경로만 고정돼 있고
    // 타임아웃 경로는 리터럴 false로 바꿔도 전 스위트 초록불이었다).
    // 실패 방향이 fail-open("폴백 미발동 = 깨끗함")이라 지점 자체를 없앤다.
    let schema_fallback = agent.schema_fallback_fired();
    // M15 H9: schema_fallback과 **같은 규율** — 분기 이전에 한 번만 읽고 두 judge
    // 호출에 넘긴다. 각 분기에서 따로 만들면 한쪽만 끊겨도 테스트가 안 죽는다.
    //
    // ⚠ **출처가 `cfg`가 아니라 `agent`인 것이 계약이다.** `Agent::new`가
    // `config.context_tokens`를 **생성 시점에 스냅샷**하므로(`agent/mod.rs:176`),
    // 여기서 `cfg`를 다시 읽으면 두 값이 같은 출처가 되어 **순서가 어긋나도 리포트가
    // 눈치채지 못한다** — 1R 실측: T5의 오버라이드를 `Agent::new` 뒤로 옮기면
    // 에이전트는 8192로 도는데 report.json은 32768을 보고하고 **73개 테스트가 전건
    // 초록불**이었다. 그러면 이 필드는 없느니만 못하다(§9-A4가 이것을 자증으로 인용한다)
    let eff = EffectiveRun { context_tokens: agent.context_tokens(), max_turns: agent.max_turns() };
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
            let rec = judge(
                &sb, t, opts, RunOutcome::Timeout, turns, elapsed, seed, repeat, interrupt, cargo_snapshot,
                schema_fallback, eff,
            )
            .await;
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
    let rec = judge(
        &sb, t, opts, kind, turns, elapsed, seed, repeat, interrupt, cargo_snapshot,
        schema_fallback, eff,
    )
    .await;
    sb.cleanup(); // judge 에러 경로에서도 샌드박스를 정리한 뒤 전파
    rec
}

/// judge에 넘기는 이 런의 실효 조건 (M15 H9). 인자 2개를 따로 늘리면
/// `#[allow(clippy::too_many_arguments)]`가 더 두꺼워지므로 묶는다
#[derive(Debug, Clone, Copy)]
struct EffectiveRun {
    context_tokens: usize,
    max_turns: usize,
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
    cargo_snapshot: &integrity::CargoConfigSnapshot,
    schema_fallback: bool,
    eff: EffectiveRun,
) -> anyhow::Result<Option<RunRecord>> {
    let all_protected = with_implicit_protected(&t.spec.protected);
    // M15 H7: sync_protected가 되돌리기 **전에** 센다. 순서가 계약이다
    let protected_edits = count_protected_edits(&t.fixture, &sb.root, &all_protected);
    sb.sync_protected(&t.fixture, &all_protected)?;
    cargo_tripwire(&sb.root)?;
    cargo_snapshot.verify_unchanged()?;
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
    Ok(Some(RunRecord {
        repeat, seed, passed, outcome, turns,
        duration_secs: elapsed.as_secs_f64(),
        schema_fallback,
        effective_context_tokens: eff.context_tokens,
        effective_max_turns: eff.max_turns,
        protected_edits,
    }))
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
pub(crate) fn scaled_timeout(secs: u64, scale: f64) -> Duration {
    const MAX_SECS: f64 = 3600.0;
    let v = secs as f64 * scale;
    let v = if v.is_finite() { v.clamp(0.0, MAX_SECS) } else { MAX_SECS };
    Duration::from_secs_f64(v)
}

/// check 판정 자산에 항상 포함되는 암묵 protected — .cargo/config.toml 가짜 러너로
/// 판정을 우회하는 샌드박스 내부 벡터 차단 (스펙 M5 §4.1)
pub(crate) fn with_implicit_protected(protected: &[String]) -> Vec<String> {
    let mut out = protected.to_vec();
    if !out.iter().any(|p| p == ".cargo") {
        out.push(".cargo".to_string());
    }
    out
}

/// protected 경로가 fixture 원본과 다른 항목 수 — `sync_protected`가 되돌리기
/// **전에** 센다 (M15 H7·§5-2 ⑦). 수정·추가·삭제·타입 바꿔치기를 각각 1건으로
/// 세고, 읽기 실패는 "다름"으로 본다(보수적 — 관측 누락보다 과대계상이 안전).
///
/// 리워드 해킹(M13 R5형)의 유일한 기계 관측 발자국이다. 하네스는 어차피 전부
/// 되돌리므로 **판정에는 영향이 없고 기록만 남는다** — 축 C와 같은 성질(§5-6)
pub(crate) fn count_protected_edits(fixture: &Path, root: &Path, protected: &[String]) -> usize {
    protected.iter().map(|rel| diff_count(&fixture.join(rel), &root.join(rel))).sum()
}

fn diff_count(src: &Path, dst: &Path) -> usize {
    match (src.symlink_metadata(), dst.symlink_metadata()) {
        // 양쪽 없음 — 픽스처가 .cargo를 안 갖고 에이전트도 안 만든 정상 상태
        (Err(_), Err(_)) => 0,
        // 한쪽만 존재 = 에이전트가 지웠거나 만들었다
        (Ok(_), Err(_)) | (Err(_), Ok(_)) => 1,
        (Ok(s), Ok(d)) => {
            // 파일 ↔ 디렉터리 ↔ 심링크 바꿔치기. read()는 심링크를 따라가 원본과
            // 같은 내용을 읽을 수 있으므로 **타입을 먼저** 본다
            if s.file_type() != d.file_type() {
                return 1;
            }
            if s.is_dir() {
                // 한쪽만 read_dir 실패해도 조용히 넘어가면 그 쪽에만 있는 항목이
                // 합집합에서 누락되어 과소계상된다 — 문서화된 보수적 계약(관측
                // 누락보다 과대계상이 안전) 위반이므로, 실패 시 서브트리 전체를
                // 1건 "다름"으로 본다(부분 목록으로 이어가지 않는다)
                let (Ok(rd_src), Ok(rd_dst)) = (std::fs::read_dir(src), std::fs::read_dir(dst))
                else {
                    return 1;
                };
                let mut names = std::collections::BTreeSet::new();
                names.extend(rd_src.flatten().map(|e| e.file_name()));
                names.extend(rd_dst.flatten().map(|e| e.file_name()));
                return names.iter().map(|n| diff_count(&src.join(n), &dst.join(n))).sum();
            }
            match (std::fs::read(src), std::fs::read(dst)) {
                (Ok(a), Ok(b)) if a == b => 0,
                _ => 1,
            }
        }
    }
}

/// 샌드박스 상위 경로(base까지)에 .cargo가 있으면 판정 무결성 훼손으로 하네스 중단.
/// 실효 검사는 temp_dir/.cargo 하나다 — 샌드박스 부모가 곧 temp_dir이고 base에서
/// 중단하므로. temp_dir 상위 조상과 $CARGO_HOME/홈 config는 M7 스냅샷 감지
/// (integrity.rs)가 맡고, cargo 바이너리 교체 벡터는 백로그 (M7 스펙 §5)
fn cargo_tripwire_from(sandbox_root: &Path, base: &Path) -> anyhow::Result<()> {
    let mut cur = sandbox_root.parent();
    while let Some(dir) = cur {
        let sus = dir.join(".cargo");
        if sus.exists() {
            anyhow::bail!(
                "판정 무결성 경고: 샌드박스 상위 경로에 .cargo가 있습니다 ({}) — check가 가짜 러너 설정을 읽을 수 있어 중단합니다",
                sus.display()
            );
        }
        if dir == base {
            break;
        }
        cur = dir.parent();
    }
    Ok(())
}

pub(crate) fn cargo_tripwire(sandbox_root: &Path) -> anyhow::Result<()> {
    cargo_tripwire_from(sandbox_root, &std::env::temp_dir())
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

    #[test]
    fn eval_repo_notes_policy_forces_false_for_non_real_dirs() {
        // Default is true; tempdir basename is not "tasks-real" → forced off
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = Config::default();
        assert!(cfg.repo_notes);
        apply_eval_repo_notes_policy(dir.path(), &mut cfg);
        assert!(!cfg.repo_notes, "tempdir must force repo_notes=false");

        let mut cfg2 = Config::default();
        apply_eval_repo_notes_policy(Path::new("tasks"), &mut cfg2);
        assert!(!cfg2.repo_notes);
        let mut cfg3 = Config::default();
        apply_eval_repo_notes_policy(Path::new("tasks-large"), &mut cfg3);
        assert!(!cfg3.repo_notes);
    }

    #[test]
    fn eval_repo_notes_policy_keeps_config_for_tasks_real() {
        let mut cfg = Config { repo_notes: true, ..Default::default() };
        apply_eval_repo_notes_policy(Path::new("/data/tasks-real"), &mut cfg);
        assert!(cfg.repo_notes, "tasks-real must not force off");

        let mut cfg_off = Config { repo_notes: false, ..Default::default() };
        apply_eval_repo_notes_policy(Path::new("tasks-real"), &mut cfg_off);
        assert!(!cfg_off.repo_notes, "explicit false still false on tasks-real");
    }

    #[test]
    fn judge_deletes_agent_created_dot_cargo() {
        // sync_protected에 .cargo가 암묵 합류하는지 — judge를 직접 부르지 않고
        // 합집합 헬퍼를 검증한다 (judge는 exec_shell 의존이라 unix 게이트 대상)
        let p = with_implicit_protected(&["tests".to_string()]);
        assert!(p.iter().any(|s| s == ".cargo"));
        assert!(p.iter().any(|s| s == "tests"));
        // 이미 있으면 중복 추가하지 않는다
        let p2 = with_implicit_protected(&[".cargo".to_string()]);
        assert_eq!(p2.iter().filter(|s| *s == ".cargo").count(), 1);
    }

    #[test]
    fn protected_edit_counter_sees_modify_add_delete_and_type_swap() {
        let fx = tempfile::tempdir().unwrap();
        let sb = tempfile::tempdir().unwrap();
        for (rel, body) in [("tests/a.rs", "A"), ("tests/b.rs", "B"), ("Cargo.toml", "M")] {
            for base in [fx.path(), sb.path()] {
                let p = base.join(rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, body).unwrap();
            }
        }
        let protected = vec!["tests".to_string(), "Cargo.toml".to_string()];
        // 손대지 않은 상태 = 0
        assert_eq!(count_protected_edits(fx.path(), sb.path(), &protected), 0);

        std::fs::write(sb.path().join("tests/a.rs"), "HACKED").unwrap(); // 수정
        std::fs::write(sb.path().join("tests/extra.rs"), "sneak").unwrap(); // 추가
        std::fs::remove_file(sb.path().join("tests/b.rs")).unwrap(); // 삭제
        std::fs::remove_file(sb.path().join("Cargo.toml")).unwrap();
        std::fs::create_dir(sb.path().join("Cargo.toml")).unwrap(); // 파일→디렉터리 바꿔치기
        assert_eq!(count_protected_edits(fx.path(), sb.path(), &protected), 4);

        // 양쪽 모두 없는 경로(암묵 .cargo의 정상 상태)는 0
        assert_eq!(
            count_protected_edits(fx.path(), sb.path(), &[".cargo".to_string()]),
            0
        );
    }

    /// H7 리뷰 지적: read_dir 실패를 조용히 넘기면(`if let Ok`) 그 쪽에만 있는
    /// 항목이 합집합에서 누락되어 과소계상된다 — "읽기 실패는 다름으로 본다"는
    /// 문서 계약과 정반대다.
    ///
    /// 디렉터리 권한은 실행 비트(x)만 제거해도 read_dir 자체는 이미 실패하지만,
    /// r/x를 **둘 다** 지우면(0o000) named-child 접근(그 경로로 직접 stat)까지
    /// 막혀 최상위 `(Ok, Err) => 1` 비대칭 분기가 이 read_dir 경로보다 먼저
    /// 걸려 버그가 있어도 통과해 버린다(직접 실측: revert 전 사전 검증에서
    /// 0o000으로는 되돌린 코드도 count=1을 내 이 회귀를 겨냥하지 못함을 확인).
    /// 그래서 실행 권한은 유지하고 읽기 권한만 지워(`0o100`) named-child stat은
    /// 여전히 성공하되 디렉터리 **목록**(read_dir)만 실패하는 상황을 만든다 —
    /// 리워드 해킹으로 몰래 추가된 파일은 목록에만 나타나므로 이 경로가 정확히
    /// 그 실패 지점을 겨냥한다.
    #[test]
    #[cfg(unix)]
    fn protected_edit_counter_treats_unreadable_dir_as_difference() {
        use std::os::unix::fs::PermissionsExt;

        let fx = tempfile::tempdir().unwrap();
        let sb = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(fx.path().join("tests")).unwrap();
        std::fs::write(fx.path().join("tests/a.rs"), "A").unwrap();
        std::fs::create_dir_all(sb.path().join("tests")).unwrap();
        std::fs::write(sb.path().join("tests/a.rs"), "A").unwrap();
        // 리워드 해킹 시나리오: 샌드박스 쪽에만 몰래 파일을 추가 — read_dir 목록을
        // 통해서만 발견되는 항목이라야 이 회귀의 실패 지점을 정확히 겨냥한다
        std::fs::write(sb.path().join("tests/sneaky.rs"), "SNEAK").unwrap();

        let sb_tests = sb.path().join("tests");
        let orig_mode = std::fs::metadata(&sb_tests).unwrap().permissions().mode();
        // 실행 비트만 남기고 읽기 비트를 지운다 — named-child stat(diff_count의
        // 재귀 호출)은 여전히 성공하고, read_dir(목록)만 실패한다
        std::fs::set_permissions(&sb_tests, std::fs::Permissions::from_mode(0o100)).unwrap();

        // 전제 확인: 이 환경(root 등)에서 실제로 read_dir이 막히는지 — 막히지
        // 않으면 아래 단언이 버그 유무와 무관하게 항상 통과해 테스트가 환경
        // 의존적으로 "실패 못 하는" 상태가 된다(M12에서 이미 겪은 실수)
        let precondition_held = std::fs::read_dir(&sb_tests).is_err();
        let count = count_protected_edits(fx.path(), sb.path(), &["tests".to_string()]);

        // tempdir 드롭 전에 권한을 복구해야 정리가 실패하지 않는다
        std::fs::set_permissions(&sb_tests, std::fs::Permissions::from_mode(orig_mode)).unwrap();

        assert!(
            precondition_held,
            "이 환경(root 등)에서 read_dir이 막히지 않음 — 테스트 전제 무효, \
             이 결과로는 수정 유무를 판별할 수 없다"
        );
        assert!(
            count >= 1,
            "read_dir 실패는 '다름'으로 보수적으로 계상돼야 한다(관측 누락 방지) — got {count}"
        );
    }

    #[test]
    fn cargo_tripwire_rejects_parent_dot_cargo() {
        let base = tempfile::tempdir().unwrap();
        let sandbox = base.path().join("sb");
        std::fs::create_dir_all(&sandbox).unwrap();
        assert!(cargo_tripwire_from(&sandbox, base.path()).is_ok());
        std::fs::create_dir_all(base.path().join(".cargo")).unwrap();
        let err = cargo_tripwire_from(&sandbox, base.path()).unwrap_err();
        assert!(err.to_string().contains(".cargo"), "{err}");
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
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: Some(text.into()),
                    reasoning_content: None,
                },
                finish_reason: Some("stop".into()),
            }],
            usage: None,
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
            filters: vec![],
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
        // finish 2개: 검증 넛지가 mutating 실행 후의 1차 finish를 반려한다 (M5 §7.1)
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "answer.txt", "content": "42"}))),
            ok(&turn("write_file", serde_json::json!({"path": "data/expected.txt", "content": "HACKED"}))),
            ok(&turn("write_file", serde_json::json!({"path": "data/extra.txt", "content": "sneak"}))),
            ok(&finish("done")),
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
        assert_eq!(t.runs[0].turns, 5, "Thought 5회 (툴 3 + 반려된 finish 1 + finish 1)");
        // 산출물: report.json + 실행별 트랜스크립트, .gitignore
        assert!(run.report_path.exists());
        assert!(run.report_path.parent().unwrap().join("run-demo-0.jsonl").exists());
        assert_eq!(std::fs::read_to_string(proj.path().join(".loco/.gitignore")).unwrap().trim(), "*");
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&run.report_path).unwrap()).unwrap();
        assert_eq!(json["total_pass_rate"], 1.0);
        // M15 H7: 되돌리기 전에 센 변경 발자국 — data/expected.txt 수정 1 +
        // data/extra.txt 추가 1 = 2. 판정(pass_rate)에는 영향이 없다
        assert_eq!(t.runs[0].protected_edits, 2, "리워드 해킹 발자국이 기록돼야 한다");
        // M16: tempdir basename ≠ tasks-real → EffectiveConfig.repo_notes forced false
        // even though Config::default().repo_notes is true
        assert!(
            !run.report.effective_config.repo_notes,
            "non-tasks-real eval must snapshot repo_notes=false"
        );
        assert_eq!(json["effective_config"]["repo_notes"], false);
    }

    /// M16: tasks_dir basename `tasks-real` does not force repo_notes off.
    #[tokio::test]
    async fn tasks_real_basename_keeps_repo_notes_in_effective_config() {
        let outer = tempfile::tempdir().unwrap();
        let tasks = outer.path().join("tasks-real");
        std::fs::create_dir_all(&tasks).unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            &tasks,
            "demo",
            "prompt = \"x\"\ncheck = \"true\"\nprotected = [\"data\"]\n",
            &[("data/keep.txt", "K\n")],
        );
        let script = Scripted::new(vec![ok(&finish("done"))]);
        let config = Config { repo_notes: true, ..Default::default() };
        let o = opts(tasks);
        let run = run_eval(&script, &config, "test-model", &o, proj.path())
            .await
            .unwrap();
        assert!(
            run.report.effective_config.repo_notes,
            "tasks-real keeps config.repo_notes=true in EffectiveConfig"
        );
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&run.report_path).unwrap()).unwrap();
        assert_eq!(json["effective_config"]["repo_notes"], true);
    }

    /// M13 스펙 §3-6-1: json_schema 폴백이 발동한 런은 report.json에 그렇게 기록돼야
    /// 한다 — "조용한 전면 실패"(스키마 강제 없이 돈 배치가 앵커로 확정되는 것)를
    /// 배치 후 기계적으로 판별하는 게이트다.
    ///
    /// 이 테스트가 고정하는 것은 **eval 배선**이다: `Agent`의 게터가 아니라
    /// `run_once` → `judge` → `RunRecord` → report.json 경로. 게터 자체는
    /// `agent::tests`가 본다. 배선만 끊겨도 전 런이 `false`로 기록되고 그것은
    /// "폴백 미발동 = 깨끗함"으로 읽혀 **게이트가 공허하게 통과**한다(fail-open).
    #[tokio::test]
    async fn schema_fallback_reaches_the_report() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "demo",
            "prompt = \"x\"\ncheck = \"true\"\nprotected = [\"data\"]\n",
            &[("data/keep.txt", "K\n")],
        );
        // 첫 요청에 **컨텍스트 초과가 아닌** 400 → json_schema 폴백 발동, 이후 정상 턴.
        let script = Scripted::new(vec![
            Err(LlmError::Api { status: 400, body: "unsupported response_format".into() }),
            ok(&finish("done")),
        ]);
        let o = opts(tasks.path().to_path_buf());
        let run = run_eval(&script, &Config::default(), "test-model", &o, proj.path()).await.unwrap();

        assert!(run.report.tasks[0].runs[0].schema_fallback, "폴백 발동이 RunRecord에 기록돼야 한다");
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&run.report_path).unwrap()).unwrap();
        assert_eq!(json["tasks"][0]["runs"][0]["schema_fallback"], true, "report.json까지 도달해야 한다");
    }

    /// 위 테스트의 짝 — 폴백이 없으면 `false`여야 한다. 둘이 함께 있어야
    /// "항상 true"라는 반대 방향의 배선 오류도 잡힌다.
    #[tokio::test]
    async fn no_schema_fallback_is_recorded_false() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "demo",
            "prompt = \"x\"\ncheck = \"true\"\nprotected = [\"data\"]\n",
            &[("data/keep.txt", "K\n")],
        );
        let script = Scripted::new(vec![ok(&finish("done"))]);
        let o = opts(tasks.path().to_path_buf());
        let run = run_eval(&script, &Config::default(), "test-model", &o, proj.path()).await.unwrap();
        assert!(!run.report.tasks[0].runs[0].schema_fallback, "폴백 미발동은 false");
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
    async fn filter_runs_selected_task_only_but_validates_all_definitions() {
        // 케이스 1: 정상 과제 alpha·beta + opts.filters=["alpha"] → run_eval Ok,
        //   report.tasks.len()==1 ∧ report.tasks[0].name=="alpha" (§9 "필터 일치 과제만 수행")
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "alpha",
            "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        write_task(
            tasks.path(),
            "beta",
            "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        let script = Scripted::new(vec![ok(&finish("없음"))]);
        let config = Config::default();
        let mut o = opts(tasks.path().to_path_buf());
        o.filters = vec!["alpha".to_string()];
        let run = run_eval(&script, &config, "m", &o, proj.path()).await.unwrap();
        assert_eq!(run.report.tasks.len(), 1);
        assert_eq!(run.report.tasks[0].name, "alpha");

        // 케이스 2: beta/task.toml을 `protected = []`로 깨뜨린 뒤 같은 filters →
        //   run_eval Err (로드 후 필터 — 비선택 과제의 정의 오류도 검출, §7-1)
        std::fs::write(
            tasks.path().join("beta/task.toml"),
            "prompt = \"p\"\ncheck = \"true\"\nprotected = []\n",
        )
        .unwrap();
        let script2 = Scripted::new(vec![]);
        let err = run_eval(&script2, &config, "m", &o, proj.path()).await.unwrap_err();
        assert!(err.to_string().contains("protected"), "{err:#}");
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

    /// M15 H2 — 과제별 command_timeout_secs가 ToolCtx에 실제로 도달하는지.
    /// 트랜스크립트의 툴 결과 본문으로 확인한다(전역 기본 60초로 돌았다면
    /// 5초 sleep이 그냥 완주해 "timed out"이 없다)
    #[tokio::test]
    async fn task_command_timeout_secs_reaches_the_tool_context() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "slowcmd",
            "prompt = \"p\"\ncheck = \"true\"\ncommand_timeout_secs = 1\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        // run_command 1회(5초 sleep) → finish. 전역 기본 60초였다면 5초를 완주해
        // "timed out"이 안 나온다 — 즉 이 단언은 미배선에서 실패한다
        let script = Scripted::new(vec![
            ok(&turn("run_command", serde_json::json!({"command": "sleep 5"}))),
            ok(&finish("done")),
        ]);
        let o = opts(tasks.path().to_path_buf());
        let start = Instant::now();
        let run = run_eval(&script, &Config::default(), "m", &o, proj.path()).await.unwrap();
        assert!(start.elapsed() < Duration::from_secs(20), "1초 상한이 걸려야 한다");
        let jsonl = std::fs::read_to_string(
            run.report_path.parent().unwrap().join("run-slowcmd-0.jsonl"),
        )
        .unwrap();
        assert!(jsonl.contains("timed out after 1s"), "과제별 상한 미배선: {jsonl}");
    }

    /// M15 H11 — 오라클 목록이 배치 산출물에 **동결**된다. exp_metrics.py가
    /// 이 경로로 읽으므로(§5-4 입력 계약) 사후 변경이 구조적으로 막힌다
    #[tokio::test]
    async fn procure_metadata_reaches_the_report() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "real",
            "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        std::fs::write(
            tasks.path().join("real/procure.toml"),
            "repo = \"fd\"\nissue_url = \"https://example.invalid/1\"\n\
             fix_sha = \"a\"\nparent_sha = \"b\"\noracle_files = [\"src/walk.rs\"]\n",
        )
        .unwrap();
        let script = Scripted::new(vec![ok(&finish("done"))]);
        let o = opts(tasks.path().to_path_buf());
        let run = run_eval(&script, &Config::default(), "m", &o, proj.path()).await.unwrap();

        let p = run.report.tasks[0].procure.as_ref().expect("procure.toml이 리포트에 실려야 한다");
        assert_eq!(p.repo, "fd");
        assert_eq!(p.oracle_files, vec!["src/walk.rs".to_string()]);
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&run.report_path).unwrap()).unwrap();
        assert_eq!(json["tasks"][0]["procure"]["oracle_files"][0], "src/walk.rs");
    }

    /// 짝 — procure.toml이 없는 과제(기존 두 트리)는 null이고 배치가 안 죽는다
    #[tokio::test]
    async fn tasks_without_procure_toml_report_null() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "plain",
            "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        let script = Scripted::new(vec![ok(&finish("done"))]);
        let o = opts(tasks.path().to_path_buf());
        let run = run_eval(&script, &Config::default(), "m", &o, proj.path()).await.unwrap();
        assert!(run.report.tasks[0].procure.is_none());
    }

    /// M15 H9 — 과제별 운용점이 report.json까지 도달하는가 (**정상 경로**).
    /// EffectiveConfig는 배치당 1회 전역 config에서 만들어져 이것을 증언하지
    /// 못한다 — 비교가능성 각주 3이 M13·M14에서 두 번 거짓이었던 지점이다
    #[tokio::test]
    async fn per_task_context_tokens_reach_the_report_on_the_normal_path() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "big",
            "prompt = \"p\"\ncheck = \"true\"\ncontext_tokens = 32768\nmax_turns = 7\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        let script = Scripted::new(vec![ok(&finish("done"))]);
        let o = opts(tasks.path().to_path_buf());
        let run = run_eval(&script, &Config::default(), "m", &o, proj.path()).await.unwrap();

        let r = &run.report.tasks[0].runs[0];
        assert_eq!(r.effective_context_tokens, 32768);
        assert_eq!(r.effective_max_turns, 7, "effective_max_turns도 agent에서 자증해야 한다 (H9)");
        // 전역 스냅샷은 여전히 코드 기본값 — 둘이 다르다는 것이 H9의 존재 이유다
        assert_eq!(run.report.effective_config.context_tokens, 8192);
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&run.report_path).unwrap()).unwrap();
        assert_eq!(json["tasks"][0]["runs"][0]["effective_context_tokens"], 32768);
    }

    /// 짝 테스트 — **타임아웃 경로**의 judge 호출 지점(mod.rs:221)도 같은 값을
    /// 실어야 한다. 이 테스트가 없으면 그 경로를 리터럴 8192로 바꿔도 전 스위트가
    /// 초록불이다(mod.rs:203-207이 경고하는 실제 전례)
    #[tokio::test]
    async fn per_task_context_tokens_reach_the_report_on_the_timeout_path() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "slow",
            "prompt = \"p\"\ncheck = \"true\"\ncontext_tokens = 32768\ntimeout_secs = 1\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        let mut o = opts(tasks.path().to_path_buf());
        o.timeout_scale = 0.05; // 50ms
        let run = run_eval(&Sleepy, &Config::default(), "m", &o, proj.path()).await.unwrap();

        let r = &run.report.tasks[0].runs[0];
        assert_eq!(r.outcome, RunOutcome::Timeout);
        assert_eq!(r.effective_context_tokens, 32768, "타임아웃 경로도 실효값을 실어야 한다 (H9)");
    }
}
