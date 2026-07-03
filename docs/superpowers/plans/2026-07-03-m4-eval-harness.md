# M4 — 평가 하네스(`loco eval`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `loco eval <tasks-dir>` 서브커맨드 + 과제 세트 12개 + 선행 수정 ①(-p 취소 배선)로, 소형 모델 스캐폴딩 개선을 통과율로 측정할 수 있게 한다.

**Architecture:** eval은 인프로세스로 `Agent::run`을 호출한다(가짜 `LlmClient`로 서버 없이 하네스 자체 테스트 가능). 과제마다 fixture를 임시 샌드박스에 복사 → `--auto` 의미의 `AutoApprover`로 실행 → protected 경로를 fixture 원본과 동기화(보상 해킹 차단) → check 명령 종료코드로 판정. 취소/타임아웃은 `ToolCtx.cancel` 플래그 + 유예 대기(`run_bounded`)로 자식 프로세스 그룹까지 정리한다.

**Tech Stack:** 기존 의존성만 (clap subcommand, serde/toml, tokio). **새 크레이트 추가 금지** — tempfile은 dev-dependency로만 유지(샌드박스 임시 디렉터리는 수제 생성).

**스펙:** `docs/superpowers/specs/2026-07-03-m4-eval-design.md` (설계), `docs/superpowers/specs/2026-07-02-loco-design.md` §8 (마스터).

## Global Constraints

- Edition 2024. 의존성 목록은 스펙이 고정 — 어떤 크레이트도 추가하지 않는다 (dev-dep의 본체 승격도 금지)
- 태스크마다 `cargo test` 전체 통과 + `cargo clippy --all-targets -- -D warnings` 클린 후 커밋
- 사용자 대상 CLI 메시지는 한국어, 식별자·모델 대상 텍스트(에러 문자열 포함)는 영어
- 에러: `llm` 모듈은 `thiserror`, 앱 레벨과 eval은 `anyhow`
- 테스트는 실서버 금지 — 스크립트된 가짜 `LlmClient` 사용, 전체 스위트는 수 초 내 완료 유지
- 셸 의존 테스트는 기존 관례대로 `#[cfg(unix)]` 서브모듈로 게이트
- conventional commits, 제목 한국어 허용
- 커밋 트레일러: `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`

---

### Task 1: `ChatRequest.seed` + `Agent` 시드 주입

**Files:**
- Modify: `src/llm/types.rs` (ChatRequest에 seed 필드)
- Modify: `src/agent/mod.rs` (Agent에 seed 필드 + set_seed + build_request 배선)
- Modify: `src/ui/repl.rs` (ChatRequest 생성부에 `seed: None`)

**Interfaces:**
- Produces: `ChatRequest.seed: Option<u64>` (None이면 직렬화 생략), `Agent::set_seed(&mut self, seed: u64)` — Task 8의 러너가 반복마다 `base_seed + repeat_index`를 주입한다

- [ ] **Step 1: 실패하는 테스트 작성** — `src/llm/types.rs`의 tests 모듈에 추가:

```rust
#[test]
fn seed_is_omitted_when_none_and_serialized_when_set() {
    let mut req = ChatRequest {
        model: "m".into(),
        messages: vec![ChatMessage::user("hi")],
        temperature: 0.1,
        max_tokens: None,
        stream: false,
        response_format: None,
        seed: None,
    };
    let v: serde_json::Value = serde_json::to_value(&req).unwrap();
    assert!(v.get("seed").is_none(), "None이면 필드 생략 (기존 경로 무영향)");
    req.seed = Some(42);
    let v: serde_json::Value = serde_json::to_value(&req).unwrap();
    assert_eq!(v["seed"], 42);
}
```

- [ ] **Step 2: 컴파일 실패 확인** — Run: `cargo test seed_is_omitted` → 기대: `seed` 필드 없음 컴파일 에러

- [ ] **Step 3: 필드 추가** — `src/llm/types.rs` `ChatRequest`의 `response_format` 아래에:

```rust
    /// 평가 하네스의 재현성용 (스펙 §8). None이면 필드 자체를 보내지 않는다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
```

컴파일러가 알려주는 모든 `ChatRequest { .. }` 생성부에 `seed: None` 추가. 알려진 위치: `src/llm/types.rs` 기존 테스트 4곳, `src/ui/repl.rs::run_chat_turn` 1곳, `src/agent/mod.rs::build_request` 1곳(이곳은 Step 5에서 `self.seed`로 바꾼다). `src/llm/client.rs` 테스트에 생성부가 있으면 거기도 `seed: None`.

- [ ] **Step 4: Agent 시드 테스트 작성** — `src/agent/mod.rs` tests 모듈에:

```rust
#[tokio::test]
async fn set_seed_reaches_the_request() {
    let dir = tempfile::tempdir().unwrap();
    let script = Scripted::new(vec![ok(&finish("done"))]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    agent.set_seed(7);
    let mut session = new_session(&agent);
    run_quiet(&mut agent, &mut session, "질문").await.unwrap();
    assert_eq!(script.requests.lock().unwrap()[0].seed, Some(7));
}
```

- [ ] **Step 5: Agent 구현** — `src/agent/mod.rs`:
  - `Agent` 구조체에 필드 `seed: Option<u64>,` 추가, `new()`에서 `seed: None,` 초기화
  - `impl` 블록에:

```rust
    /// 평가 하네스가 반복마다 다른 시드를 주입한다 (스펙 §8)
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = Some(seed);
    }
```

  - `build_request()`의 ChatRequest에 `seed: self.seed,`

- [ ] **Step 6: 검증** — Run: `cargo test && cargo clippy --all-targets -- -D warnings` → 기대: 전체 통과

- [ ] **Step 7: 커밋** — `git commit -m "feat: ChatRequest에 seed 추가 + Agent 시드 주입"`

---

### Task 2: `AgentOutcome::Cancelled` — 취소 플래그 루프 체크

**Files:**
- Modify: `src/agent/mod.rs` (변형 추가 + 루프 상단 체크 + 테스트)
- Modify: `src/ui/repl.rs` (match arm 추가)
- Modify: `src/main.rs` (match arm 추가)

**Interfaces:**
- Consumes: `ToolCtx.cancel: Arc<AtomicBool>` (기존)
- Produces: `AgentOutcome::Cancelled` — cancel 플래그가 선 뒤 다음 LLM 호출 전에 반환된다. Task 3의 `run_bounded`가 유예 대기 중 이 경로로 빠르게 수렴한다

- [ ] **Step 1: 실패하는 테스트 작성** — `src/agent/mod.rs` tests 모듈에 (기존 `use std::sync::atomic::{AtomicUsize, Ordering};` 활용):

```rust
#[tokio::test]
async fn preset_cancel_flag_returns_cancelled_without_llm_calls() {
    let dir = tempfile::tempdir().unwrap();
    let script = Scripted::new(vec![]); // 호출되면 스크립트 고갈로 패닉
    let ctx = ToolCtx::new(dir.path().to_path_buf());
    ctx.cancel.store(true, Ordering::SeqCst);
    let config = Config::default();
    let mut agent = Agent::new(&script, Registry::read_only(), ctx, "test-model".into(), &config);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "질문").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Cancelled));
    assert_eq!(script.requests.lock().unwrap().len(), 0, "LLM 호출 없이 반환");
}

/// 실행되면 cancel 플래그를 세우는 가짜 툴 — 툴 실행 후 다음 LLM 호출 전에 멈추는지 검증
struct CancelTool(Arc<std::sync::atomic::AtomicBool>);
impl crate::tools::Tool for CancelTool {
    fn name(&self) -> &'static str { "cancel_tool" }
    fn doc(&self) -> &'static str { "cancel_tool(): test." }
    fn run(&self, _a: &serde_json::Value, _c: &ToolCtx) -> Result<String, crate::tools::ToolError> {
        self.0.store(true, Ordering::SeqCst);
        Ok("ok".to_string())
    }
}

#[tokio::test]
async fn cancel_during_tool_stops_before_next_llm_call() {
    let dir = tempfile::tempdir().unwrap();
    let script = Scripted::new(vec![ok(&turn("cancel_tool", serde_json::json!({})))]); // 응답 1개뿐
    let ctx = ToolCtx::new(dir.path().to_path_buf());
    let flag = ctx.cancel.clone();
    let config = Config::default();
    let mut agent = Agent::new(
        &script, Registry::new(vec![Box::new(CancelTool(flag))]), ctx, "test-model".into(), &config,
    );
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "질문").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Cancelled));
    assert_eq!(script.requests.lock().unwrap().len(), 1, "취소 후 추가 LLM 호출 금지");
}
```

- [ ] **Step 2: 컴파일 실패 확인** — Run: `cargo test cancel` → 기대: `Cancelled` 변형 없음 에러

- [ ] **Step 3: 구현** — `src/agent/mod.rs`:
  - `AgentOutcome`에 변형 추가:

```rust
    /// 취소 플래그 감지 후 자발 종료 (M4 — -p Ctrl+C·eval 타임아웃).
    /// REPL은 퓨처 드롭으로 취소하므로 보통 이 변형을 보지 않는다
    Cancelled,
```

  - `run()`의 `while turns < self.max_turns {` 바로 다음 줄에:

```rust
            // 취소 신호 후에는 새 LLM 호출을 만들지 않는다 — run_bounded의 유예가
            // 이 경로로 빠르게 끝난다 (설계 §1). 진행 중이던 run_command는 자체
            // 폴링으로 이미 프로세스 그룹을 정리했다
            if self.ctx.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                return Ok(AgentOutcome::Cancelled);
            }
```

- [ ] **Step 4: match arm 추가** (exhaustive match 컴파일 에러 해소):
  - `src/ui/repl.rs::run_agent_turn`의 match에:

```rust
        Some(Ok(AgentOutcome::Cancelled)) => {
            session.rollback(snap);
            println!("\n(중단됨 — 이번 요청은 히스토리에서 제거)");
        }
```

  - `src/main.rs::run_oneshot`의 match에:

```rust
        AgentOutcome::Cancelled => {
            eprintln!("(중단됨)");
            Ok(ExitCode::from(2))
        }
```

- [ ] **Step 5: 검증** — Run: `cargo test && cargo clippy --all-targets -- -D warnings` → 기대: 전체 통과

- [ ] **Step 6: 커밋** — `git commit -m "feat: AgentOutcome::Cancelled — 취소 플래그를 루프 상단에서 감지"`

---

### Task 3: `run_bounded` + `-p` Ctrl+C 배선 (백로그 ① 완결)

**Files:**
- Create: `src/agent/bounded.rs`
- Modify: `src/agent/mod.rs` (`pub mod bounded;` 선언 — 파일 상단 `mod approval` 근처)
- Modify: `src/main.rs` (run_oneshot 배선)

**Interfaces:**
- Produces: `pub enum Stopped { TimedOut, Interrupted }`, `pub async fn run_bounded<F: Future, I: Future>(fut: F, cancel: &AtomicBool, limit: Option<Duration>, grace: Duration, interrupt: I) -> Result<F::Output, Stopped>`, `pub async fn watch_flag(flag: &AtomicBool)` — Task 8의 러너가 과제 타임아웃·공유 인터럽트 플래그에 재사용
- interrupt를 퓨처 파라미터로 받는 이유: tokio의 `ctrl_c()`는 첫 폴링에서 프로세스 기본 SIGINT 동작을 영구 대체하고 **등록 이후의 신호만** 본다. -p처럼 실행 전체가 하나의 select! 창인 호출자는 `ctrl_c()`를 그대로 넘기면 되지만, eval처럼 select! 창 밖 구간(샌드박스 복사·check 실행)이 있는 호출자는 장수 리스너+공유 플래그(`watch_flag`)를 써야 SIGINT가 유실되지 않는다

- [ ] **Step 1: 실패하는 테스트와 함께 파일 생성** — `src/agent/bounded.rs`:

```rust
//! 에이전트 실행을 시간·Ctrl+C로 경계 짓는 러너 (M4 설계 §1, 백로그 ①).

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// run_bounded가 퓨처를 중도 포기한 이유
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    TimedOut,
    Interrupted,
}

/// 플래그가 설 때까지 폴링(50ms) — eval의 공유 인터럽트 플래그를 run_bounded의
/// interrupt 퓨처로 바꾸는 어댑터
pub async fn watch_flag(flag: &AtomicBool) {
    while !flag.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 중단 신호(interrupt 퓨처 완료)와 시간 상한(limit이 Some일 때)을 감시하며 퓨처를
/// 실행한다. interrupt는 호출자가 정한다: -p는 tokio::signal::ctrl_c(), eval은
/// watch_flag(장수 리스너가 세우는 공유 플래그) — ctrl_c()는 등록 이후의 신호만
/// 보므로 select! 창 밖 구간이 있는 호출자가 그대로 쓰면 SIGINT가 유실된다.
/// 발화 시 cancel 플래그를 세우고 유예(grace) 동안 퓨처의 자연 종료를 기다린다 —
/// 즉시 드롭하면 run_command의 자식 프로세스 그룹을 죽일 기회가 없어 고아가 남는다.
/// 유예 안에 완료돼도 결과는 버린다: 호출자에게는 중단 사실이 결과보다 중요하다.
pub async fn run_bounded<F: Future, I: Future>(
    fut: F,
    cancel: &AtomicBool,
    limit: Option<Duration>,
    grace: Duration,
    interrupt: I,
) -> Result<F::Output, Stopped> {
    tokio::pin!(fut);
    tokio::pin!(interrupt);
    let stopped = tokio::select! {
        out = &mut fut => return Ok(out),
        _ = &mut interrupt => Stopped::Interrupted,
        _ = sleep_limit(limit) => Stopped::TimedOut,
    };
    cancel.store(true, Ordering::SeqCst);
    let _ = tokio::time::timeout(grace, &mut fut).await;
    Err(stopped)
}

async fn sleep_limit(limit: Option<Duration>) {
    match limit {
        Some(d) => tokio::time::sleep(d).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// 완료되지 않는 interrupt — Ctrl+C가 없는 상황
    fn never() -> std::future::Pending<()> {
        std::future::pending()
    }

    #[tokio::test]
    async fn completed_future_passes_through() {
        let cancel = AtomicBool::new(false);
        let r = run_bounded(async { 42 }, &cancel, Some(Duration::from_secs(5)), Duration::from_millis(10), never()).await;
        assert_eq!(r.unwrap(), 42);
        assert!(!cancel.load(Ordering::SeqCst), "정상 완료는 플래그를 건드리지 않음");
    }

    #[tokio::test]
    async fn timeout_sets_cancel_and_reports_timed_out() {
        let cancel = AtomicBool::new(false);
        let r = run_bounded(
            std::future::pending::<()>(),
            &cancel,
            Some(Duration::from_millis(20)),
            Duration::from_millis(10),
            never(),
        )
        .await;
        assert_eq!(r.unwrap_err(), Stopped::TimedOut);
        assert!(cancel.load(Ordering::SeqCst), "타임아웃은 cancel 플래그를 세운다");
    }

    #[tokio::test]
    async fn interrupt_future_stops_and_sets_cancel() {
        let cancel = AtomicBool::new(false);
        let flag = AtomicBool::new(true); // 이미 선 플래그 — watch_flag가 즉시 완료
        let r = run_bounded(
            std::future::pending::<()>(),
            &cancel,
            None,
            Duration::from_millis(10),
            watch_flag(&flag),
        )
        .await;
        assert_eq!(r.unwrap_err(), Stopped::Interrupted);
        assert!(cancel.load(Ordering::SeqCst), "중단도 cancel 플래그를 세운다");
    }

    #[tokio::test]
    async fn grace_lets_the_future_finish_side_effects() {
        // limit(20ms) 발화 후에도 유예(1s) 동안 퓨처가 정리 작업을 마친다 — 부수효과로 관찰
        let cancel = AtomicBool::new(false);
        let cleaned = Arc::new(AtomicBool::new(false));
        let c2 = cleaned.clone();
        let fut = async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            c2.store(true, Ordering::SeqCst);
        };
        let r = run_bounded(fut, &cancel, Some(Duration::from_millis(20)), Duration::from_secs(1), never()).await;
        assert_eq!(r.unwrap_err(), Stopped::TimedOut, "결과는 버려진다");
        assert!(cleaned.load(Ordering::SeqCst), "유예 동안 자연 종료가 완료됨");
    }
}
```

`src/agent/mod.rs` 상단(기존 `pub mod approval;` 옆)에 `pub mod bounded;` 추가.

- [ ] **Step 2: 테스트 실행** — Run: `cargo test bounded` → 기대: 4개 통과

- [ ] **Step 3: run_oneshot 배선** — `src/main.rs`:
  - import 추가: `use loco::agent::bounded::{run_bounded, Stopped};`
  - `run_oneshot`에서 `let mut ctx = ...; ctx.command_timeout = ...;` 다음 줄에 `let cancel = ctx.cancel.clone();`
  - 기존

```rust
    let outcome = agent.run(&mut session, prompt, approver, &mut on_event).await;
    spinner.borrow_mut().stop();

    match outcome? {
```

  를 다음으로 교체:

```rust
    // Ctrl+C: cancel 플래그 → 유예 대기 (자식 프로세스 그룹 정리 후 종료 — 백로그 ①).
    // -p는 실행 전체가 하나의 select! 창이라 ctrl_c()를 그대로 interrupt로 쓴다
    let bounded = run_bounded(
        agent.run(&mut session, prompt, approver, &mut on_event),
        &cancel,
        None,
        std::time::Duration::from_secs(5),
        async {
            let _ = tokio::signal::ctrl_c().await;
        },
    )
    .await;
    spinner.borrow_mut().stop();
    let outcome = match bounded {
        Ok(r) => r?,
        Err(Stopped::Interrupted) | Err(Stopped::TimedOut) => {
            // limit이 None이므로 실제로는 Interrupted만 온다
            eprintln!("(중단됨 — 실행 중이던 명령까지 정리했습니다)");
            return Ok(ExitCode::from(2));
        }
    };

    match outcome {
```

- [ ] **Step 4: 검증** — Run: `cargo test && cargo clippy --all-targets -- -D warnings` → 기대: 전체 통과

- [ ] **Step 5 (선택 — LM Studio 필요): 라이브 스모크** — 서버가 떠 있으면: `cargo run -- --auto -p "sleep 20 명령을 실행해줘"` 실행, run_command 시작 후 Ctrl+C → 기대: "(중단됨...)" 출력, exit 2, `pgrep -f "sleep 20"` 결과 없음(고아 없음). 서버가 없으면 이 단계는 건너뛰고 Task 14의 라이브 검증에 위임.

- [ ] **Step 6: 커밋** — `git commit -m "feat: run_bounded + -p Ctrl+C 취소 배선 (백로그 ①)"`

---

### Task 4: `tools/exec.rs` — 셸 실행 공용 기반 추출

**Files:**
- Create: `src/tools/exec.rs`
- Modify: `src/tools/mod.rs` (`pub(crate) mod exec;` 선언)
- Modify: `src/tools/run_command.rs` (exec 사용으로 리팩터)

**Interfaces:**
- Produces: `pub enum ExecEnd { Done(ExitStatus), TimedOut, Cancelled }`, `pub struct Exec { pub end: ExecEnd, pub body: String }`, `pub fn exec_shell(command: &str, cwd: &Path, timeout: Duration, cancel: &AtomicBool) -> std::io::Result<Exec>` — Task 8의 check 실행이 재사용
- 동작 계약: 기존 run_command와 동일 (sh -c/cmd /C, 프로세스 그룹 킬, UTF-8→CP949 폴백, 8000바이트 중간 절삭, READER_GRACE 파이프 포기)

- [ ] **Step 1: exec.rs 생성** — `src/tools/run_command.rs`에서 다음을 **이동**(복사 후 원본 삭제): `MAX_OUTPUT_BYTES`, `POLL`, `READER_GRACE`, `decode`, `truncate_middle`, `spawn_reader`, `drain`, `shell_command`, `kill_tree`, `Ended`(이름을 `ExecEnd`로, pub). `shell_command`는 `ctx: &ToolCtx` 대신 `cwd: &Path`를 받게 변경. 파일 골격:

```rust
//! 셸 명령 실행 공용 기반 — run_command 툴과 eval check가 공유 (스펙 §10).
//! 프로세스 그룹 킬, UTF-8/CP949 디코딩, 출력 중간 절삭.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

// (이동해 온 상수·헬퍼들: MAX_OUTPUT_BYTES, POLL, READER_GRACE,
//  decode, truncate_middle, spawn_reader, drain, kill_tree — 본문 무변경)

fn shell_command(command: &str, cwd: &Path) -> Command {
    // 기존 run_command::shell_command에서 ctx.root → cwd 로만 변경
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut c = Command::new("sh");
        c.arg("-c").arg(command).current_dir(cwd);
        c.process_group(0);
        c
    }
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.args(["/C", command]).current_dir(cwd);
        c
    }
}

pub enum ExecEnd {
    Done(std::process::ExitStatus),
    TimedOut,
    Cancelled,
}

pub struct Exec {
    pub end: ExecEnd,
    /// "--- stdout ---"/"--- stderr ---" 섹션 + 절삭·파이프 점유 안내가 적용된 본문
    pub body: String,
}

pub fn exec_shell(
    command: &str,
    cwd: &Path,
    timeout: Duration,
    cancel: &AtomicBool,
) -> std::io::Result<Exec> {
    let mut child = shell_command(command, cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let out_rx = spawn_reader(child.stdout.take());
    let err_rx = spawn_reader(child.stderr.take());

    let start = Instant::now();
    let end = loop {
        if let Some(status) = child.try_wait()? {
            break ExecEnd::Done(status);
        }
        if cancel.load(Ordering::SeqCst) {
            kill_tree(&mut child);
            let _ = child.wait();
            break ExecEnd::Cancelled;
        }
        if start.elapsed() >= timeout {
            kill_tree(&mut child);
            let _ = child.wait();
            break ExecEnd::TimedOut;
        }
        std::thread::sleep(POLL);
    };

    // 기존 run_command::run의 출력 조립 로직 그대로 (stdout/stderr 섹션,
    // 파이프 점유 안내, truncate_middle)
    let (stdout, out_ok) = drain(out_rx);
    let (stderr, err_ok) = drain(err_rx);
    let mut body = String::new();
    if !stdout.trim().is_empty() {
        body.push_str("--- stdout ---\n");
        body.push_str(&stdout);
    }
    if !stderr.trim().is_empty() {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str("--- stderr ---\n");
        body.push_str(&stderr);
    }
    if !out_ok || !err_ok {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str("(some output unavailable - a background child still holds the pipe)");
    }
    Ok(Exec { end, body: truncate_middle(&body) })
}
```

decode/truncate_middle 테스트도 exec.rs의 `#[cfg(test)] mod tests`로 이동. 추가로:

```rust
    #[cfg(unix)]
    #[test]
    fn exec_shell_reports_exit_status() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let e = exec_shell("exit 7", dir.path(), Duration::from_secs(5), &cancel).unwrap();
        assert!(matches!(e.end, ExecEnd::Done(s) if s.code() == Some(7)));
    }
```

`src/tools/mod.rs`에 `pub(crate) mod exec;` 추가.

- [ ] **Step 2: run_command.rs 리팩터** — 이동한 항목을 지우고 `run()`을 다음으로 교체 (preview·name·doc·is_mutating·Args는 무변경):

```rust
use serde::Deserialize;

use super::exec::{exec_shell, ExecEnd};
use super::{Tool, ToolCtx, ToolError};
```

```rust
    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone()).map_err(|e| ToolError::BadArgs(e.to_string()))?;
        let exec = exec_shell(&args.command, &ctx.root, ctx.command_timeout, &ctx.cancel)?;
        Ok(match exec.end {
            ExecEnd::Done(status) => {
                let code = status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "(terminated by signal)".to_string());
                format!("exit code: {code}\n{}", exec.body)
            }
            ExecEnd::TimedOut => format!(
                "command timed out after {}s and was killed\n{}",
                ctx.command_timeout.as_secs(),
                exec.body
            ),
            ExecEnd::Cancelled => format!("command was cancelled by the user\n{}", exec.body),
        })
    }
```

run_command.rs의 기존 unix 서브모듈 테스트는 그대로 둔다(리팩터 후에도 통과해야 함 — 이것이 동작 보존 증명). 단, 외부 `mod tests`는 decode/truncate 테스트가 exec.rs로 떠나면 `use super::*;`와 그 위의 안내 주석이 unused가 되어 `-D warnings` 게이트에 걸린다 — 외부 tests 모듈을 `#[cfg(unix)] mod unix` 서브모듈만 남게 정리할 것.

- [ ] **Step 3: 검증** — Run: `cargo test && cargo clippy --all-targets -- -D warnings` → 기대: 전체 통과 (특히 run_command unix 테스트 6개)

- [ ] **Step 4: 커밋** — `git commit -m "refactor: run_command 실행 기반을 tools/exec로 추출"`

---

### Task 5: `eval/task.rs` — 과제 로드·검증

**Files:**
- Create: `src/eval/mod.rs` (일단 `pub mod task;`만)
- Create: `src/eval/task.rs`
- Modify: `src/lib.rs` (`pub mod eval;`)

**Interfaces:**
- Produces: `pub struct TaskSpec { prompt: String, check: String, timeout_secs: u64 (기본 300), check_timeout_secs: u64 (기본 120), max_turns: Option<usize>, protected: Vec<String> }`, `pub struct Task { name: String, fixture: PathBuf, spec: TaskSpec }`, `pub fn load_tasks(tasks_dir: &Path) -> anyhow::Result<Vec<Task>>` (이름순 정렬, 시작 전 일괄 검증)

- [ ] **Step 1: 실패하는 테스트 작성** — `src/eval/task.rs` (구현 없이 tests 먼저 못 쓰므로 스켈레톤과 함께):

```rust
//! 과제 정의 로드·검증 (설계 §3). 과제 하나 = 디렉터리 하나 (task.toml + fixture/).
//! 정의 오류는 실행 시작 전 하네스 에러로 일괄 보고한다 (스펙 §8 종료 코드 1).

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::Deserialize;

/// task.toml 스키마 (설계 §3). 미지 키는 오타로 간주해 거부 — config와 동일 정책
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpec {
    /// 에이전트에게 줄 요청 (실사용과 같은 한국어)
    pub prompt: String,
    /// 샌드박스 루트에서 실행할 판정 명령 — 종료 코드 0이면 통과
    pub check: String,
    /// 에이전트 실행 전체(LLM 호출 포함) 상한. 파싱 재시도 탓에 최악
    /// LLM 호출 = max_turns×3회 (스펙 §8) — 넉넉하게
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// check 명령 상한 (콜드 빌드 감안)
    #[serde(default = "default_check_timeout_secs")]
    pub check_timeout_secs: u64,
    /// 설정보다 우선하는 과제별 턴 상한
    pub max_turns: Option<usize>,
    /// 판정 자산 — check 전에 fixture 원본과 정확히 일치하도록 동기화 (스펙 §8)
    pub protected: Vec<String>,
}

fn default_timeout_secs() -> u64 {
    300
}

fn default_check_timeout_secs() -> u64 {
    120
}

pub struct Task {
    pub name: String,
    pub fixture: PathBuf,
    pub spec: TaskSpec,
}

pub fn load_tasks(tasks_dir: &Path) -> anyhow::Result<Vec<Task>> {
    let mut tasks = Vec::new();
    let entries = std::fs::read_dir(tasks_dir)
        .with_context(|| format!("과제 디렉터리를 열 수 없음: {}", tasks_dir.display()))?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue; // .gitattributes 등 과제 아닌 파일은 무시
        }
        let dir = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(dir.join("task.toml"))
            .with_context(|| format!("과제 {name}: task.toml 읽기 실패"))?;
        let spec: TaskSpec =
            toml::from_str(&text).with_context(|| format!("과제 {name}: task.toml 파싱 실패"))?;
        let fixture = dir.join("fixture");
        if !fixture.is_dir() {
            bail!("과제 {name}: fixture/ 디렉터리가 없음");
        }
        if spec.protected.is_empty() {
            bail!("과제 {name}: protected가 비어 있음 — 판정 자산 없이는 공정한 채점이 불가 (스펙 §8)");
        }
        for p in &spec.protected {
            if !fixture.join(p).exists() {
                bail!("과제 {name}: protected 경로가 fixture에 없음: {p}");
            }
        }
        tasks.push(Task { name, fixture, spec });
    }
    if tasks.is_empty() {
        bail!("과제가 없음: {}", tasks_dir.display());
    }
    tasks.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
prompt = "p"
check = "true"
protected = ["keep.txt"]
"#;

    fn write_task(root: &Path, name: &str, toml: &str, fixture_files: &[&str]) {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("fixture")).unwrap();
        std::fs::write(dir.join("task.toml"), toml).unwrap();
        for f in fixture_files {
            let p = dir.join("fixture").join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
    }

    #[test]
    fn loads_sorted_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "b-task", MINIMAL, &["keep.txt"]);
        write_task(dir.path(), "a-task", MINIMAL, &["keep.txt"]);
        let tasks = load_tasks(dir.path()).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "a-task", "이름순 정렬");
        assert_eq!(tasks[0].spec.timeout_secs, 300, "기본값");
        assert_eq!(tasks[0].spec.check_timeout_secs, 120);
        assert_eq!(tasks[0].spec.max_turns, None);
    }

    #[test]
    fn overrides_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let toml = r#"
prompt = "p"
check = "cargo test"
timeout_secs = 60
check_timeout_secs = 30
max_turns = 10
protected = ["keep.txt"]
"#;
        write_task(dir.path(), "t", toml, &["keep.txt"]);
        let t = &load_tasks(dir.path()).unwrap()[0];
        assert_eq!(t.spec.timeout_secs, 60);
        assert_eq!(t.spec.check_timeout_secs, 30);
        assert_eq!(t.spec.max_turns, Some(10));
    }

    #[test]
    fn unknown_key_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "t", "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\ntimout_secs = 3\n", &["keep.txt"]);
        let err = load_tasks(dir.path()).unwrap_err();
        assert!(err.to_string().contains("t"), "{err:#}");
    }

    #[test]
    fn missing_fixture_dir_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("t");
        std::fs::create_dir_all(&t).unwrap();
        std::fs::write(t.join("task.toml"), MINIMAL).unwrap();
        assert!(load_tasks(dir.path()).unwrap_err().to_string().contains("fixture"));
    }

    #[test]
    fn empty_protected_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "t", "prompt = \"p\"\ncheck = \"true\"\nprotected = []\n", &["keep.txt"]);
        assert!(load_tasks(dir.path()).unwrap_err().to_string().contains("protected"));
    }

    #[test]
    fn protected_path_must_exist_in_fixture() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "t", MINIMAL, &["other.txt"]); // keep.txt 없음
        assert!(load_tasks(dir.path()).unwrap_err().to_string().contains("keep.txt"));
    }

    #[test]
    fn empty_tasks_dir_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_tasks(dir.path()).is_err());
    }
}
```

`src/eval/mod.rs`:

```rust
//! 평가 하네스 (스펙 §8, 설계 2026-07-03) — M4에서 단계적으로 채워진다

pub mod task;
```

`src/lib.rs`에 `pub mod eval;` 추가 (알파벳 순서 유지: config 다음).

- [ ] **Step 2: 검증** — Run: `cargo test eval:: && cargo clippy --all-targets -- -D warnings` → 기대: 신규 7개 포함 전체 통과

- [ ] **Step 3: 커밋** — `git commit -m "feat: eval 과제 로드·검증 (task.toml)"`

---

### Task 6: `eval/sandbox.rs` — fixture 복사·protected 동기화

**Files:**
- Create: `src/eval/sandbox.rs`
- Modify: `src/eval/mod.rs` (`pub mod sandbox;`)

**Interfaces:**
- Produces: `pub struct Sandbox { pub root: PathBuf }`, `Sandbox::create(fixture: &Path) -> anyhow::Result<Sandbox>`, `Sandbox::sync_protected(&self, fixture: &Path, protected: &[String]) -> anyhow::Result<()>`, `Sandbox::cleanup(self)` — Task 8의 러너가 사용
- 동기화 의미론(스펙 §8): protected 경로를 샌드박스에서 통째로 지우고 fixture에서 새로 복사 → 수정 복원과 추가 파일 삭제를 한 번에 달성

- [ ] **Step 1: 구현 + 테스트 작성** — `src/eval/sandbox.rs`:

```rust
//! 과제 샌드박스 — fixture 복사, protected 동기화 (스펙 §8), 임시 디렉터리 관리.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};

/// 프로세스 내 샌드박스 일련번호 — pid와 조합해 고유 이름을 만든다
static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub struct Sandbox {
    pub root: PathBuf,
}

impl Sandbox {
    /// fixture를 새 임시 디렉터리로 복사한다. tempfile 크레이트는 dev-dependency —
    /// 의존성 고정(스펙) 때문에 본체로 승격하지 않고 pid+카운터로 고유 이름을 만든다
    pub fn create(fixture: &Path) -> anyhow::Result<Sandbox> {
        let base = std::env::temp_dir();
        loop {
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let root = base.join(format!("loco-eval-{}-{n}", std::process::id()));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    if let Err(e) = copy_tree(fixture, &root) {
                        // 부분 복사 잔재를 남기지 않는다 — 에러 경로 샌드박스 누수 방지
                        let _ = std::fs::remove_dir_all(&root);
                        return Err(e);
                    }
                    return Ok(Sandbox { root });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    return Err(e).with_context(|| format!("샌드박스 생성 실패: {}", root.display()));
                }
            }
        }
    }

    /// protected 경로를 fixture 원본과 정확히 일치시킨다 (스펙 §8):
    /// 샌드박스 쪽을 통째로 지우고 fixture에서 새로 복사 —
    /// 수정 복원 + 에이전트가 추가한 파일 삭제를 한 번에 처리
    pub fn sync_protected(&self, fixture: &Path, protected: &[String]) -> anyhow::Result<()> {
        for rel in protected {
            let src = fixture.join(rel);
            let dst = self.root.join(rel);
            if dst.symlink_metadata().is_ok() {
                remove_any(&dst).with_context(|| format!("protected 정리 실패: {}", dst.display()))?;
            }
            if src.is_dir() {
                std::fs::create_dir_all(&dst)?;
                copy_tree(&src, &dst)?;
            } else if src.exists() {
                if let Some(parent) = dst.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&src, &dst)?;
            }
        }
        Ok(())
    }

    /// 최선 노력 정리 — 실패해도 하네스를 죽이지 않는다
    pub fn cleanup(self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn copy_tree(src: &Path, dst: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&from)?;
        if meta.is_symlink() {
            bail!("fixture에 심링크가 있음 (지원 안 함): {}", from.display());
        }
        if meta.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn remove_any(p: &Path) -> std::io::Result<()> {
    // is_dir()은 심링크를 따라간다 — 모델이 protected 경로를 심링크로 바꿔치기해도
    // 하네스가 죽지 않게 symlink_metadata의 파일타입으로 분기한다 (심링크 자체는
    // remove_file 대상; remove_dir_all은 심링크 루트를 거부한다)
    let meta = p.symlink_metadata()?;
    if meta.file_type().is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (rel, content) in files {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        dir
    }

    #[test]
    fn create_copies_nested_tree() {
        let fx = fixture_with(&[("src/lib.rs", "code"), ("tests/t.rs", "test"), ("Cargo.toml", "manifest")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        assert_eq!(std::fs::read_to_string(sb.root.join("src/lib.rs")).unwrap(), "code");
        assert_eq!(std::fs::read_to_string(sb.root.join("tests/t.rs")).unwrap(), "test");
        sb.cleanup();
    }

    #[test]
    fn two_sandboxes_get_distinct_roots() {
        let fx = fixture_with(&[("a.txt", "x")]);
        let a = Sandbox::create(fx.path()).unwrap();
        let b = Sandbox::create(fx.path()).unwrap();
        assert_ne!(a.root, b.root);
        a.cleanup();
        b.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_in_fixture_is_an_error() {
        let fx = fixture_with(&[("real.txt", "x")]);
        std::os::unix::fs::symlink(fx.path().join("real.txt"), fx.path().join("link.txt")).unwrap();
        assert!(Sandbox::create(fx.path()).unwrap_err().to_string().contains("심링크"));
    }

    #[cfg(unix)]
    #[test]
    fn sync_replaces_symlinked_protected_path() {
        // 보상 해킹 변형: 모델이 run_command로 protected 디렉터리를 심링크로 바꿔치기
        let fx = fixture_with(&[("tests/t.rs", "ORIGINAL"), ("decoy.txt", "D")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::remove_dir_all(sb.root.join("tests")).unwrap();
        std::os::unix::fs::symlink(sb.root.join("decoy.txt"), sb.root.join("tests")).unwrap();
        sb.sync_protected(fx.path(), &["tests".to_string()]).unwrap();
        assert_eq!(std::fs::read_to_string(sb.root.join("tests/t.rs")).unwrap(), "ORIGINAL");
        sb.cleanup();
    }

    #[test]
    fn sync_restores_modified_and_deletes_added() {
        let fx = fixture_with(&[("tests/t.rs", "ORIGINAL"), ("src/lib.rs", "code")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        // 에이전트의 보상 해킹 시뮬레이션: protected 수정 + protected 아래 파일 추가
        std::fs::write(sb.root.join("tests/t.rs"), "HACKED").unwrap();
        std::fs::write(sb.root.join("tests/extra.rs"), "sneak").unwrap();
        // protected 밖 산출물은 보존돼야 한다
        std::fs::write(sb.root.join("answer.txt"), "42").unwrap();

        sb.sync_protected(fx.path(), &["tests".to_string()]).unwrap();

        assert_eq!(std::fs::read_to_string(sb.root.join("tests/t.rs")).unwrap(), "ORIGINAL", "수정 복원");
        assert!(!sb.root.join("tests/extra.rs").exists(), "추가 파일 삭제 (스펙 §8)");
        assert_eq!(std::fs::read_to_string(sb.root.join("answer.txt")).unwrap(), "42", "작업 산출물 보존");
        sb.cleanup();
    }

    #[test]
    fn sync_restores_single_protected_file() {
        let fx = fixture_with(&[("Cargo.toml", "ORIGINAL")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::write(sb.root.join("Cargo.toml"), "HACKED").unwrap();
        sb.sync_protected(fx.path(), &["Cargo.toml".to_string()]).unwrap();
        assert_eq!(std::fs::read_to_string(sb.root.join("Cargo.toml")).unwrap(), "ORIGINAL");
        sb.cleanup();
    }

    #[test]
    fn sync_removes_protected_dir_the_agent_deleted_and_recreated_wrong() {
        let fx = fixture_with(&[("tests/a.rs", "A"), ("tests/sub/b.rs", "B")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::remove_dir_all(sb.root.join("tests")).unwrap();
        std::fs::create_dir_all(sb.root.join("tests")).unwrap();
        std::fs::write(sb.root.join("tests/a.rs"), "TAMPERED").unwrap();
        sb.sync_protected(fx.path(), &["tests".to_string()]).unwrap();
        assert_eq!(std::fs::read_to_string(sb.root.join("tests/a.rs")).unwrap(), "A");
        assert_eq!(std::fs::read_to_string(sb.root.join("tests/sub/b.rs")).unwrap(), "B", "중첩 복원");
        sb.cleanup();
    }

    #[test]
    fn cleanup_removes_the_sandbox() {
        let fx = fixture_with(&[("a.txt", "x")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        let root = sb.root.clone();
        sb.cleanup();
        assert!(!root.exists());
    }
}
```

`src/eval/mod.rs`에 `pub mod sandbox;` 추가.

- [ ] **Step 2: 검증** — Run: `cargo test sandbox && cargo clippy --all-targets -- -D warnings` → 기대: 8개 통과

- [ ] **Step 3: 커밋** — `git commit -m "feat: eval 샌드박스 — fixture 복사·protected 동기화"`

---

### Task 7: `eval/report.rs` — 집계·한국어 표·JSON

**Files:**
- Create: `src/eval/report.rs`
- Modify: `src/eval/mod.rs` (`pub mod report;`)

**Interfaces:**
- Produces (Task 8의 러너와 main.rs가 사용):
  - `pub enum RunOutcome { Finished, MaxTurns, RepetitionStop, ParseFailed, Timeout }` (snake_case 직렬화)
  - `pub struct RunRecord { repeat: usize, seed: u64, passed: bool, outcome: RunOutcome, turns: usize, duration_secs: f64 }`
  - `pub struct TaskReport { name, pass_rate, avg_turns, avg_duration_secs, runs }` + `TaskReport::from_runs(name: String, runs: Vec<RunRecord>) -> TaskReport`
  - `pub struct Report { model, base_seed, repeats, timeout_scale, started_at, duration_secs, interrupted, tasks, total_pass_rate }` + `Report::total_of(tasks: &[TaskReport]) -> f64` + `Report::render_table(&self) -> String`

- [ ] **Step 1: 구현 + 테스트 작성** — `src/eval/report.rs`:

```rust
//! 평가 리포트 — 실행 레코드 집계, 한국어 표, report.json (스펙 §8).

use serde::Serialize;

/// 실행 1회의 결말. Timeout은 하네스 타임아웃(run_bounded)이며,
/// 어떤 결말이든 check는 실행된다 (설계 결정 — MaxTurns라도 작업이 됐으면 통과)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Finished,
    MaxTurns,
    RepetitionStop,
    ParseFailed,
    Timeout,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunRecord {
    pub repeat: usize,
    /// base_seed + repeat — 개별 실행 재현용 (스펙 §8)
    pub seed: u64,
    pub passed: bool,
    pub outcome: RunOutcome,
    pub turns: usize,
    pub duration_secs: f64,
}

#[derive(Debug, Serialize)]
pub struct TaskReport {
    pub name: String,
    pub pass_rate: f64,
    pub avg_turns: f64,
    pub avg_duration_secs: f64,
    pub runs: Vec<RunRecord>,
}

impl TaskReport {
    pub fn from_runs(name: String, runs: Vec<RunRecord>) -> TaskReport {
        let n = runs.len().max(1) as f64;
        TaskReport {
            pass_rate: runs.iter().filter(|r| r.passed).count() as f64 / n,
            avg_turns: runs.iter().map(|r| r.turns as f64).sum::<f64>() / n,
            avg_duration_secs: runs.iter().map(|r| r.duration_secs).sum::<f64>() / n,
            name,
            runs,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub model: String,
    pub base_seed: u64,
    pub repeats: usize,
    pub timeout_scale: f64,
    pub started_at: String,
    pub duration_secs: f64,
    /// Ctrl+C로 중단된 부분 결과인지 — 표와 종료 코드(1)에 반영
    pub interrupted: bool,
    pub tasks: Vec<TaskReport>,
    pub total_pass_rate: f64,
}

impl Report {
    /// 전체 통과율 = 통과 실행 수 / 전체 실행 수 (과제별 평균의 평균이 아님 —
    /// 중단으로 반복 수가 다른 과제가 있어도 왜곡되지 않는 정의)
    pub fn total_of(tasks: &[TaskReport]) -> f64 {
        let total: usize = tasks.iter().map(|t| t.runs.len()).sum();
        if total == 0 {
            return 0.0;
        }
        let passed: usize = tasks.iter().map(|t| t.runs.iter().filter(|r| r.passed).count()).sum();
        passed as f64 / total as f64
    }

    /// stdout용 한국어 표 (스펙 §8 리포트). 폭 계산이 char 수 기준이라 한글
    /// 헤더(전각)와 ASCII 행의 열이 약간 어긋난다 — 과제명이 ASCII라 수용(의도적)
    pub fn render_table(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{:<28} {:>7} {:>9} {:>10}\n", "과제", "통과", "평균 턴", "평균 시간"));
        for t in &self.tasks {
            let passed = t.runs.iter().filter(|r| r.passed).count();
            let ratio = format!("{passed}/{}", t.runs.len());
            out.push_str(&format!(
                "{:<28} {:>7} {:>9.1} {:>9.1}s\n",
                t.name, ratio, t.avg_turns, t.avg_duration_secs
            ));
        }
        let total: usize = self.tasks.iter().map(|t| t.runs.len()).sum();
        let passed: usize = self.tasks.iter().map(|t| t.runs.iter().filter(|r| r.passed).count()).sum();
        out.push_str(&format!(
            "전체 통과율 {:.1}% ({passed}/{total}, 시드 {}부터, timeout×{}){}\n",
            self.total_pass_rate * 100.0,
            self.base_seed,
            self.timeout_scale,
            if self.interrupted { " — 중단됨(부분 결과)" } else { "" }
        ));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(passed: bool, turns: usize, secs: f64) -> RunRecord {
        RunRecord { repeat: 0, seed: 0, passed, outcome: RunOutcome::Finished, turns, duration_secs: secs }
    }

    #[test]
    fn from_runs_computes_averages() {
        let t = TaskReport::from_runs("t".into(), vec![run(true, 4, 10.0), run(false, 6, 20.0)]);
        assert_eq!(t.pass_rate, 0.5);
        assert_eq!(t.avg_turns, 5.0);
        assert_eq!(t.avg_duration_secs, 15.0);
    }

    #[test]
    fn empty_runs_do_not_divide_by_zero() {
        let t = TaskReport::from_runs("t".into(), vec![]);
        assert_eq!(t.pass_rate, 0.0);
        assert_eq!(Report::total_of(&[t]), 0.0);
    }

    #[test]
    fn total_is_runs_weighted() {
        let a = TaskReport::from_runs("a".into(), vec![run(true, 1, 1.0)]); // 1/1
        let b = TaskReport::from_runs("b".into(), vec![run(false, 1, 1.0), run(false, 1, 1.0), run(false, 1, 1.0)]); // 0/3
        assert_eq!(Report::total_of(&[a, b]), 0.25, "실행 가중 — 과제 평균의 평균(0.5)이 아님");
    }

    #[test]
    fn outcome_serializes_snake_case() {
        assert_eq!(serde_json::to_value(RunOutcome::MaxTurns).unwrap(), "max_turns");
        assert_eq!(serde_json::to_value(RunOutcome::Timeout).unwrap(), "timeout");
    }

    fn sample_report() -> Report {
        let tasks = vec![TaskReport::from_runs("add-function".into(), vec![run(true, 5, 38.5)])];
        Report {
            model: "gemma-4b".into(),
            base_seed: 0,
            repeats: 1,
            timeout_scale: 1.0,
            started_at: "20260703T000000Z".into(),
            duration_secs: 40.0,
            interrupted: false,
            total_pass_rate: Report::total_of(&tasks),
            tasks,
        }
    }

    #[test]
    fn report_json_has_design_schema_fields() {
        let v = serde_json::to_value(sample_report()).unwrap();
        for key in ["model", "base_seed", "repeats", "timeout_scale", "started_at", "duration_secs", "interrupted", "tasks", "total_pass_rate"] {
            assert!(v.get(key).is_some(), "리포트에 {key} 필드가 있어야 함");
        }
        assert_eq!(v["tasks"][0]["runs"][0]["seed"], 0, "시드 기록 (스펙 §8 재현성)");
        assert_eq!(v["tasks"][0]["runs"][0]["outcome"], "finished");
    }

    #[test]
    fn table_mentions_tasks_and_total() {
        let table = sample_report().render_table();
        assert!(table.contains("add-function"));
        assert!(table.contains("1/1"));
        assert!(table.contains("전체 통과율 100.0%"));
        assert!(!table.contains("중단됨"));
        let mut interrupted = sample_report();
        interrupted.interrupted = true;
        assert!(interrupted.render_table().contains("중단됨"));
    }
}
```

`src/eval/mod.rs`에 `pub mod report;` 추가.

- [ ] **Step 2: 검증** — Run: `cargo test report && cargo clippy --all-targets -- -D warnings` → 기대: 통과

- [ ] **Step 3: 커밋** — `git commit -m "feat: eval 리포트 집계·한국어 표·JSON 스키마"`

---

### Task 8: `eval/mod.rs` 러너 + `Transcript::create_at`

**Files:**
- Modify: `src/session.rs` (`Transcript::create_at`, `now_secs` 노출)
- Modify: `src/eval/mod.rs` (러너 본체 + 통합 테스트)

**Interfaces:**
- Consumes: Task 1 `set_seed` / Task 2 `AgentOutcome::Cancelled` / Task 3 `run_bounded, watch_flag, Stopped` / Task 4 `exec_shell, ExecEnd` / Task 5 `load_tasks` / Task 6 `Sandbox` / Task 7 리포트 타입
- Produces: `pub struct EvalOptions { tasks_dir: PathBuf, repeats: usize, base_seed: u64, timeout_scale: f64, cancel_grace: Duration }`, `pub struct EvalRun { report: Report, report_path: PathBuf }`, `pub async fn run_eval<C: LlmClient>(client: &C, config: &Config, model: &str, opts: &EvalOptions, project_root: &Path) -> anyhow::Result<EvalRun>` — Task 9의 CLI가 호출
- 계약: LLM 에러는 즉시 `Err`(하네스 중단), Ctrl+C는 `interrupted: true`인 부분 리포트, 타임아웃은 `outcome: timeout`으로 기록하고 계속. check는 outcome과 무관하게 항상 실행. 리포트는 `<project_root>/.loco/eval/<stamp>/report.json` + 실행별 `run-<과제>-<반복>.jsonl`

- [ ] **Step 1: Transcript::create_at** — `src/session.rs`:
  - `fn now_secs()` → `pub(crate) fn now_secs()`
  - `Transcript` impl에 추가:

```rust
    /// 지정 경로에 트랜스크립트 생성 — eval이 리포트 디렉터리에 실행별 기록을 남긴다
    pub fn create_at(path: &Path) -> std::io::Result<Transcript> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create_new(path)?;
        Ok(Transcript { file: Some(file), path: Some(path.to_path_buf()) })
    }
```

  - tests에 추가:

```rust
    #[test]
    fn create_at_writes_to_the_given_path() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/run-x-0.jsonl");
        let mut t = Transcript::create_at(&p).unwrap();
        t.record("user", "질문");
        assert!(std::fs::read_to_string(&p).unwrap().contains("질문"));
    }
```

- [ ] **Step 2: 러너 구현** — `src/eval/mod.rs` 전체를 다음으로 교체:

```rust
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
    listener.abort();

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
    let sb = Sandbox::create(&t.fixture)?;
    let mut cfg = config.clone();
    if let Some(mt) = t.spec.max_turns {
        cfg.max_turns = mt;
    }
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
    // eval은 --auto 의미 — config의 auto_deny_patterns 적용 (스펙 §5·§8)
    let mut approver = AutoApprover::new(&cfg.auto_deny_patterns)?;
    let mut turns = 0usize;
    let mut on_event = |ev: AgentEvent<'_>| {
        // 턴 수 = 파싱된 턴(Thought) 수 — 패킹 절삭과 무관하게 정확 (설계 결정)
        if matches!(ev, AgentEvent::Thought(_)) {
            turns += 1;
        }
    };
    let limit = Duration::from_secs_f64(t.spec.timeout_secs as f64 * opts.timeout_scale);
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
    let check_timeout = Duration::from_secs_f64(t.spec.check_timeout_secs as f64 * opts.timeout_scale);
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
```

- [ ] **Step 3: 통합 테스트 작성** — `src/eval/mod.rs` 말미에 (agent 테스트의 Scripted 패턴을 eval용으로 재정의 — 셸 check를 쓰므로 unix 게이트):

```rust
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
```

참고: `LlmError` 경로가 `crate::llm::client::LlmError`인지 확인하고 임포트를 실제 위치에 맞출 것. `_keep`/`ChatMessage` 임포트가 불필요하면 제거(클리피 게이트).

- [ ] **Step 4: 검증** — Run: `cargo test && cargo clippy --all-targets -- -D warnings` → 기대: 전체 통과. 통과 후 임시 디렉터리 누수 확인: `ls /tmp/loco-eval-* 2>/dev/null | wc -l` → 0 (macOS는 `ls $TMPDIR/loco-eval-*`)

- [ ] **Step 5: 커밋** — `git commit -m "feat: eval 러너 — 과제×반복 실행·판정·리포트 기록"`

---

### Task 9: CLI 서브커맨드 배선 + 서버-다운 스모크

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `loco::eval::{run_eval, EvalOptions}`
- Produces: `loco eval <tasks-dir> [--repeats N] [--seed N] [--timeout-scale F]` — 정상 완료 exit 0(통과율 무관), 하네스 에러 exit 1, Ctrl+C 부분 리포트 exit 1

- [ ] **Step 1: Cli 구조 확장** — `src/main.rs`:

```rust
#[derive(Parser)]
#[command(name = "loco", version, about = "폐쇄망 소형모델 코딩 CLI")]
struct Cli {
    /// 단발 실행 프롬프트 (비대화형 에이전트 — 최종 답변만 stdout)
    #[arg(short, long)]
    prompt: Option<String>,
    /// 확인 게이트 전부 자동 승인 (auto_deny_patterns 차단은 유지)
    #[arg(long)]
    auto: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// 평가 하네스 — 과제 세트를 실행해 통과율 리포트 생성 (스펙 §8)
    Eval {
        /// 과제 디렉터리 (과제 하나 = 하위 디렉터리 하나)
        tasks_dir: std::path::PathBuf,
        /// 과제당 반복 횟수 (권장 3~5 — 1회 비교는 노이즈)
        #[arg(long, default_value_t = 1)]
        repeats: usize,
        /// 기본 시드 — 반복 i의 시드는 seed + i
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// 모든 과제·check 타임아웃에 곱하는 배수 (느린 머신 대응)
        #[arg(long, default_value_t = 1.0)]
        timeout_scale: f64,
    },
}
```

- [ ] **Step 2: run() 분기** — `run()`의 `match cli.prompt` 앞에:

```rust
    if let Some(Command::Eval { tasks_dir, repeats, seed, timeout_scale }) = cli.command {
        // Duration::from_secs_f64는 음수/비유한 값에 패닉 — 하네스 에러(exit 1)로 선검증
        if !(timeout_scale.is_finite() && timeout_scale > 0.0) {
            anyhow::bail!("--timeout-scale은 0보다 큰 유한한 값이어야 합니다 (받은 값: {timeout_scale})");
        }
        if repeats == 0 {
            anyhow::bail!("--repeats는 1 이상이어야 합니다");
        }
        let opts = loco::eval::EvalOptions {
            tasks_dir,
            repeats,
            base_seed: seed,
            timeout_scale,
            cancel_grace: std::time::Duration::from_secs(5),
        };
        let root = std::env::current_dir()?;
        let run = loco::eval::run_eval(&client, &config, &model, &opts, &root).await?;
        println!("{}", run.report.render_table());
        println!("리포트: {}", run.report_path.display());
        return Ok(if run.report.interrupted { ExitCode::from(1) } else { ExitCode::SUCCESS });
    }
```

- [ ] **Step 3: 헬프 확인** — Run: `cargo run -- eval --help` → 기대: 한국어 설명과 `--repeats`, `--seed`, `--timeout-scale` 플래그 표시. `cargo run -- --help` → 기대: 기존 `-p`/`--auto`와 `eval` 서브커맨드 병기

- [ ] **Step 4: 서버-다운 스모크** — Run (CLAUDE.md의 스모크 절차, port 1):

```bash
cargo build
REPO="$PWD"
SMOKE="$(mktemp -d)" && cd "$SMOKE"
mkdir -p .loco tasks/t/fixture
printf 'base_url = "http://127.0.0.1:1/v1"\n' > .loco/config.toml
printf 'prompt = "x"\ncheck = "true"\nprotected = ["f.txt"]\n' > tasks/t/task.toml
touch tasks/t/fixture/f.txt
"$REPO/target/debug/loco" eval tasks; echo "exit=$?"
cd "$REPO"
```

기대: `오류:` + 서버 확인 안내 메시지, `exit=1` (하네스 에러 — 스펙 §8)

- [ ] **Step 5: 검증** — Run: `cargo test && cargo clippy --all-targets -- -D warnings` → 기대: 전체 통과

- [ ] **Step 6: 커밋** — `git commit -m "feat: loco eval CLI 서브커맨드 (--repeats/--seed/--timeout-scale)"`

---

### Task 10: 과제 1~4 (읽기·초급)

**Files:**
- Create: `tasks/find-definition/…`, `tasks/count-usages/…`, `tasks/add-function/…`, `tasks/fix-off-by-one/…`

**Interfaces:**
- Consumes: Task 5의 task.toml 스키마. 모든 과제 공통: `check = "cargo test"`, `protected = ["tests", "Cargo.toml"]`, 무의존 크레이트(오프라인 빌드), edition 2021
- 모든 fixture의 `Cargo.toml`은 다음 템플릿에서 name만 과제 디렉터리명으로 바꾼다:

```toml
[package]
name = "<과제-디렉터리명>"
version = "0.1.0"
edition = "2021"
```

- 검증 프로토콜(모든 과제 공통): fixture를 스크래치에 복사 → ① 초기 `cargo test` 실패 확인(과제가 실제로 "풀 것"이 있음) → ② 골든 수정 적용 후 `cargo test` 통과 확인(과제가 실제로 풀림)

- [ ] **Step 1: find-definition** — 파일 생성:

`tasks/find-definition/task.toml`:

```toml
prompt = "이 프로젝트에서 `area` 함수가 정의된 파일을 찾아, 그 경로(프로젝트 루트 기준 상대 경로)를 answer.txt 파일에 한 줄로 저장해줘."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/find-definition/fixture/Cargo.toml`:

```toml
[package]
name = "find-definition"
version = "0.1.0"
edition = "2021"
```

`tasks/find-definition/fixture/src/lib.rs`:

```rust
pub mod geometry;
pub mod text;
```

`tasks/find-definition/fixture/src/geometry.rs`:

```rust
/// 직사각형 넓이
pub fn area(w: f64, h: f64) -> f64 {
    w * h
}
```

`tasks/find-definition/fixture/src/text.rs`:

```rust
/// 앞뒤 공백 제거
pub fn tidy(s: &str) -> String {
    s.trim().to_string()
}
```

`tasks/find-definition/fixture/tests/check.rs`:

```rust
#[test]
fn answer_names_the_defining_file() {
    let answer = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/answer.txt"))
        .expect("answer.txt가 없습니다");
    let normalized = answer.trim().replace('\\', "/");
    assert_eq!(normalized.trim_start_matches("./"), "src/geometry.rs");
}
```

- [ ] **Step 2: find-definition 검증**:

```bash
S=/tmp/loco-fixture-check && rm -rf $S && cp -R tasks/find-definition/fixture $S
(cd $S && cargo test) # 기대: FAIL (answer.txt 없음)
(cd $S && printf 'src/geometry.rs\n' > answer.txt && cargo test) # 기대: PASS
```

- [ ] **Step 3: count-usages** — 파일 생성:

`tasks/count-usages/task.toml`:

```toml
prompt = "src/lib.rs 안에서 `normalize` 함수가 호출되는 횟수(호출 표현식의 개수 — 한 줄에 두 번 나오면 2회)를 세어, 그 숫자만 answer.txt에 한 줄로 저장해줘. use/pub use 선언은 호출이 아니야."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/count-usages/fixture/Cargo.toml`: (name = "count-usages", 나머지는 find-definition과 동일 형식)

`tasks/count-usages/fixture/src/lib.rs`:

```rust
mod util;

pub use util::normalize;

pub fn title(s: &str) -> String {
    let n = normalize(s);
    let mut c = n.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => n,
    }
}

pub fn slug(s: &str) -> String {
    normalize(s).replace(' ', "-")
}

pub fn compare(a: &str, b: &str) -> bool {
    normalize(a) == normalize(b)
}
```

`tasks/count-usages/fixture/src/util.rs`:

```rust
pub fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}
```

`tasks/count-usages/fixture/tests/check.rs`:

```rust
#[test]
fn answer_counts_call_sites() {
    let answer = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/answer.txt"))
        .expect("answer.txt가 없습니다");
    assert_eq!(answer.trim(), "4"); // title 1 + slug 1 + compare 2
}
```

- [ ] **Step 4: count-usages 검증** — Step 2와 같은 프로토콜, 골든: `printf '4\n' > answer.txt` → PASS

- [ ] **Step 5: add-function** — 파일 생성:

`tasks/add-function/task.toml`:

```toml
prompt = "src/lib.rs의 median 함수를 doc 주석 명세대로 구현해줘. tests/median.rs의 테스트가 통과해야 해."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/add-function/fixture/Cargo.toml`: (name = "add-function")

`tasks/add-function/fixture/src/lib.rs`:

```rust
/// 정수 슬라이스의 중앙값. 짝수 길이는 가운데 두 값의 평균.
/// 입력은 비어 있지 않다고 가정한다.
pub fn median(xs: &[i64]) -> f64 {
    let _ = xs;
    todo!("구현 필요")
}
```

`tasks/add-function/fixture/tests/median.rs`:

```rust
use add_function::median;

#[test]
fn odd_length() {
    assert_eq!(median(&[3, 1, 2]), 2.0);
}

#[test]
fn even_length() {
    assert_eq!(median(&[1, 2, 3, 4]), 2.5);
}

#[test]
fn single() {
    assert_eq!(median(&[5]), 5.0);
}

#[test]
fn unsorted_negative() {
    assert_eq!(median(&[-5, 10, 0]), 0.0);
}
```

- [ ] **Step 6: add-function 검증** — 초기 FAIL(todo 패닉) 확인 후 골든 구현으로 교체해 PASS 확인:

```rust
/// 정수 슬라이스의 중앙값. 짝수 길이는 가운데 두 값의 평균.
/// 입력은 비어 있지 않다고 가정한다.
pub fn median(xs: &[i64]) -> f64 {
    let mut v = xs.to_vec();
    v.sort_unstable();
    let n = v.len();
    if n % 2 == 1 { v[n / 2] as f64 } else { (v[n / 2 - 1] + v[n / 2]) as f64 / 2.0 }
}
```

- [ ] **Step 7: fix-off-by-one** — 파일 생성:

`tasks/fix-off-by-one/task.toml`:

```toml
prompt = "`sum_upto` 함수가 잘못된 값을 반환해. 버그를 찾아서 고쳐줘."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/fix-off-by-one/fixture/Cargo.toml`: (name = "fix-off-by-one")

`tasks/fix-off-by-one/fixture/src/lib.rs`:

```rust
/// 1부터 n까지(포함) 정수의 합
pub fn sum_upto(n: u32) -> u32 {
    (1..n).sum()
}
```

`tasks/fix-off-by-one/fixture/tests/sums.rs`:

```rust
use fix_off_by_one::sum_upto;

#[test]
fn sums_inclusive() {
    assert_eq!(sum_upto(5), 15);
}

#[test]
fn one() {
    assert_eq!(sum_upto(1), 1);
}

#[test]
fn zero() {
    assert_eq!(sum_upto(0), 0);
}
```

- [ ] **Step 8: fix-off-by-one 검증** — 초기 FAIL, 골든 `(1..=n).sum()` → PASS

- [ ] **Step 9: 하네스 통과 확인** — Run: `cargo test eval:: -q && ls tasks` → 기존 하네스 테스트 통과 + 4개 과제 디렉터리. (라이브 실행은 Task 14)

- [ ] **Step 10: 커밋** — `git add tasks && git commit -m "feat: 평가 과제 1~4 — 코드 찾기·함수 구현·버그 수정"`

---

### Task 11: 과제 5~8 (중급·이스케이프/EOL 리스크)

**Files:**
- Create: `tasks/fix-failing-test/…`, `tasks/multiline-string-edit/…`, `tasks/edit-crlf-file/…`, `tasks/create-module/…`, `tasks/.gitattributes`

**Interfaces:**
- 공통 규칙: `check = "cargo test"`, `protected = ["tests", "Cargo.toml"]`, 무의존 크레이트, edition 2021. 각 fixture의 `Cargo.toml`은 다음 템플릿에서 name만 과제 디렉터리명으로:

```toml
[package]
name = "<과제-디렉터리명>"
version = "0.1.0"
edition = "2021"
```

- 검증 프로토콜: fixture를 스크래치에 복사 → 초기 `cargo test` 실패 확인 → 골든 수정 적용 후 통과 확인
- `multiline-string-edit`는 마스터 스펙 §8이 의무화한 "여러 줄·따옴표 많은 편집" 측정 과제

- [ ] **Step 1: fix-failing-test** — 파일 생성:

`tasks/fix-failing-test/task.toml`:

```toml
prompt = "cargo test가 실패해. 테스트를 실행해서 실패 원인을 찾고 src 코드를 고쳐줘. 테스트 파일은 수정하면 안 돼."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/fix-failing-test/fixture/Cargo.toml`: (name = "fix-failing-test")

`tasks/fix-failing-test/fixture/src/lib.rs`:

```rust
/// 쉼표로 구분된 정수 목록의 합계. 공백 허용, 빈 문자열은 0.
pub fn sum_csv(input: &str) -> i64 {
    input.split(',').map(|p| p.trim().parse::<i64>().unwrap_or(0)).sum()
}

/// 목록의 최댓값. 파싱 불가 항목은 무시, 빈 목록이면 None.
pub fn max_csv(input: &str) -> Option<i64> {
    let mut best: Option<i64> = None;
    for part in input.split(',') {
        let Ok(v) = part.trim().parse::<i64>() else { continue };
        if best.is_none() || v < best.unwrap() {
            best = Some(v);
        }
    }
    best
}
```

`tasks/fix-failing-test/fixture/tests/csv.rs`:

```rust
use fix_failing_test::{max_csv, sum_csv};

#[test]
fn sums() {
    assert_eq!(sum_csv("1, 2,3"), 6);
}

#[test]
fn empty_sum() {
    assert_eq!(sum_csv(""), 0);
}

#[test]
fn max_of_list() {
    assert_eq!(max_csv("3, 9, 2"), Some(9));
}

#[test]
fn max_single() {
    assert_eq!(max_csv("7"), Some(7));
}

#[test]
fn max_empty() {
    assert_eq!(max_csv(""), None);
}
```

검증: 초기 FAIL(`max_of_list`), 골든 `v > best.unwrap()` → PASS

- [ ] **Step 2: multiline-string-edit** — 파일 생성:

`tasks/multiline-string-edit/task.toml`:

```toml
prompt = "src/lib.rs의 report_template가 만드는 템플릿을 수정해줘: (1) `{user_name}` 자리표시자를 `{username}`으로 바꾸고, (2) `said:` 줄 바로 다음에 `score: {score}` 줄을 추가해. 다른 줄은 그대로 유지해야 해."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/multiline-string-edit/fixture/Cargo.toml`: (name = "multiline-string-edit")

`tasks/multiline-string-edit/fixture/src/lib.rs`:

```rust
/// 사용자 요약 리포트 템플릿 (그대로 출력됨 — 이스케이프에 주의)
pub fn report_template() -> String {
    let mut t = String::new();
    t.push_str("== \"weekly\" report ==\n");
    t.push_str("user: {user_name}\n");
    t.push_str("said: \"hello, \\\"world\\\"\"\n");
    t.push_str("path: C:\\data\\logs\n");
    t.push_str("-- end of \"weekly\" report --\n");
    t
}
```

`tasks/multiline-string-edit/fixture/tests/template.rs`:

```rust
use multiline_string_edit::report_template;

#[test]
fn template_matches_spec_exactly() {
    let expected = "== \"weekly\" report ==\nuser: {username}\nsaid: \"hello, \\\"world\\\"\"\nscore: {score}\npath: C:\\data\\logs\n-- end of \"weekly\" report --\n";
    assert_eq!(report_template(), expected);
}
```

검증: 초기 FAIL, 골든(해당 두 줄 교체+추가) → PASS:

```rust
    t.push_str("user: {username}\n");
    t.push_str("said: \"hello, \\\"world\\\"\"\n");
    t.push_str("score: {score}\n");
```

- [ ] **Step 3: edit-crlf-file** — 파일 생성:

`tasks/.gitattributes` (git의 EOL 정규화로 fixture가 오염되지 않게):

```
edit-crlf-file/fixture/data/greeting.txt -text
```

`tasks/edit-crlf-file/task.toml`:

```toml
prompt = "data/greeting.txt에서 `world`라는 단어를 `loco`로 바꿔줘. 파일의 줄바꿈 형식은 바뀌면 안 돼."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/edit-crlf-file/fixture/Cargo.toml`: (name = "edit-crlf-file")

`tasks/edit-crlf-file/fixture/src/lib.rs`:

```rust
// 이 과제의 대상은 data/greeting.txt — 크레이트는 판정 테스트를 위해서만 존재
```

CRLF 데이터 파일 생성 (에디터로 만들지 말 것 — 정확한 바이트가 중요):

```bash
mkdir -p tasks/edit-crlf-file/fixture/data
printf 'hello world\r\ngoodbye moon\r\n' > tasks/edit-crlf-file/fixture/data/greeting.txt
```

`tasks/edit-crlf-file/fixture/tests/check.rs`:

```rust
#[test]
fn greeting_is_updated_and_still_crlf() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/data/greeting.txt")).unwrap();
    let text = String::from_utf8(bytes).expect("UTF-8 유지");
    assert!(text.contains("hello loco"), "단어가 교체돼야 함: {text:?}");
    assert!(!text.contains("world"), "원래 단어가 남아있음: {text:?}");
    assert!(text.contains("\r\n"), "CRLF 줄바꿈이 보존돼야 함: {text:?}");
}
```

검증: 초기 FAIL, 골든 `printf 'hello loco\r\ngoodbye moon\r\n' > data/greeting.txt` → PASS. 추가로 `git add`후 `git diff --cached -- tasks/edit-crlf-file` 출력에 CRLF 경고가 없는지 확인(.gitattributes 효과)

- [ ] **Step 4: create-module** — 파일 생성:

`tasks/create-module/task.toml`:

```toml
prompt = "src/shapes.rs 모듈을 새로 만들어서 `pub fn perimeter(w: u32, h: u32) -> u32` (직사각형 둘레 = 2*(w+h)) 함수를 구현하고, src/lib.rs에 `pub mod shapes;`로 등록해줘."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/create-module/fixture/Cargo.toml`: (name = "create-module")

`tasks/create-module/fixture/src/lib.rs`:

```rust
// shapes 모듈은 아직 없다 — 과제에서 추가된다
```

`tasks/create-module/fixture/tests/shapes.rs`:

```rust
use create_module::shapes::perimeter;

#[test]
fn rectangle_perimeter() {
    assert_eq!(perimeter(3, 4), 14);
}

#[test]
fn square() {
    assert_eq!(perimeter(5, 5), 20);
}
```

검증: 초기 FAIL(컴파일 에러), 골든(lib.rs에 `pub mod shapes;` + src/shapes.rs 생성) → PASS:

```rust
pub fn perimeter(w: u32, h: u32) -> u32 {
    2 * (w + h)
}
```

- [ ] **Step 5: 커밋** — `git add tasks && git commit -m "feat: 평가 과제 5~8 — 진단·이스케이프·CRLF·모듈 생성"`

---

### Task 12: 과제 9~12 (다중 파일·지구력)

**Files:**
- Create: `tasks/rename-function/…`, `tasks/implement-from-doc/…`, `tasks/fix-compile-error/…`, `tasks/chain-edits/…`
- Modify: `src/eval/task.rs` (커밋된 과제 세트 검증 테스트)

**Interfaces:**
- 공통 규칙: `check = "cargo test"`, `protected = ["tests", "Cargo.toml"]`, 무의존 크레이트, edition 2021. 각 fixture의 `Cargo.toml`은 다음 템플릿에서 name만 과제 디렉터리명으로:

```toml
[package]
name = "<과제-디렉터리명>"
version = "0.1.0"
edition = "2021"
```

- 검증 프로토콜: fixture를 스크래치에 복사 → 초기 `cargo test` 실패 확인 → 골든 수정 적용 후 통과 확인

- [ ] **Step 1: rename-function** — 파일 생성:

`tasks/rename-function/task.toml`:

```toml
prompt = "`total_price` 함수의 이름을 `price_total`로 바꿔줘. 정의뿐 아니라 모든 호출부와 re-export도 같이 바꿔서 프로젝트가 컴파일되게 해야 해."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/rename-function/fixture/Cargo.toml`: (name = "rename-function")

`tasks/rename-function/fixture/src/lib.rs`:

```rust
pub mod cart;
pub mod receipt;

pub use cart::total_price;
```

`tasks/rename-function/fixture/src/cart.rs`:

```rust
/// 장바구니 합계 (수량 × 단가의 총합)
pub fn total_price(items: &[(u32, u32)]) -> u32 {
    items.iter().map(|(qty, price)| qty * price).sum()
}
```

`tasks/rename-function/fixture/src/receipt.rs`:

```rust
use crate::cart::total_price;

/// 영수증 한 줄 요약
pub fn summary(items: &[(u32, u32)]) -> String {
    format!("total: {}", total_price(items))
}

/// 배송비 포함 합계 (5000 미만이면 배송비 500)
pub fn with_shipping(items: &[(u32, u32)]) -> u32 {
    let t = total_price(items);
    if t < 5000 { t + 500 } else { t }
}
```

`tasks/rename-function/fixture/tests/rename.rs`:

```rust
use rename_function::{price_total, receipt};

#[test]
fn renamed_function_is_exported() {
    assert_eq!(price_total(&[(2, 100), (1, 50)]), 250);
}

#[test]
fn callers_still_work() {
    assert_eq!(receipt::summary(&[(1, 100)]), "total: 100");
    assert_eq!(receipt::with_shipping(&[(1, 100)]), 600);
}
```

검증: 초기 FAIL(컴파일 — `price_total` 없음), 골든(cart.rs 정의명, receipt.rs use+호출 2곳, lib.rs re-export 변경) → PASS

- [ ] **Step 2: implement-from-doc** — 파일 생성:

`tasks/implement-from-doc/task.toml`:

```toml
prompt = "src/lib.rs의 `rle` 함수를 doc 주석 명세대로 구현해줘."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/implement-from-doc/fixture/Cargo.toml`: (name = "implement-from-doc")

`tasks/implement-from-doc/fixture/src/lib.rs`:

```rust
/// 런랭스 인코딩(RLE).
/// 연속으로 반복되는 문자를 `문자 + 반복횟수`로 축약한다.
/// 반복이 1회인 문자에도 횟수 1을 붙인다.
/// 예: "aaabbc" -> "a3b2c1", "" -> "".
/// 유니코드 문자 단위(char)로 처리한다.
pub fn rle(s: &str) -> String {
    let _ = s;
    todo!()
}
```

`tasks/implement-from-doc/fixture/tests/rle.rs`:

```rust
use implement_from_doc::rle;

#[test]
fn basic() {
    assert_eq!(rle("aaabbc"), "a3b2c1");
}

#[test]
fn empty() {
    assert_eq!(rle(""), "");
}

#[test]
fn no_repeats() {
    assert_eq!(rle("abc"), "a1b1c1");
}

#[test]
fn unicode() {
    assert_eq!(rle("가가가나"), "가3나1");
}

#[test]
fn long_run() {
    assert_eq!(rle(&"z".repeat(12)), "z12");
}
```

검증: 초기 FAIL, 골든 → PASS:

```rust
pub fn rle(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        let mut n = 1u32;
        while chars.peek() == Some(&c) {
            chars.next();
            n += 1;
        }
        out.push(c);
        out.push_str(&n.to_string());
    }
    out
}
```

- [ ] **Step 3: fix-compile-error** — 파일 생성:

`tasks/fix-compile-error/task.toml`:

```toml
prompt = "이 프로젝트는 컴파일이 안 돼. cargo build나 cargo test를 실행해서 에러를 확인하고, 동작 의도는 바꾸지 않는 최소 수정으로 고쳐줘."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/fix-compile-error/fixture/Cargo.toml`: (name = "fix-compile-error")

`tasks/fix-compile-error/fixture/src/lib.rs`:

```rust
/// 단어들을 대문자로 바꿔 공백 하나로 잇는다
pub fn join_upper(words: &[&str]) -> String {
    let result = String::new();
    for w in words {
        result.push_str(&w.to_uppercase());
        result.push(' ');
    }
    result.trim_end().to_string()
}
```

`tasks/fix-compile-error/fixture/tests/join.rs`:

```rust
use fix_compile_error::join_upper;

#[test]
fn joins_and_uppercases() {
    assert_eq!(join_upper(&["ab", "cd"]), "AB CD");
}

#[test]
fn empty() {
    assert_eq!(join_upper(&[]), "");
}
```

검증: 초기 FAIL(E0596 — `result` not mutable), 골든 `let mut result = String::new();` → PASS

- [ ] **Step 4: chain-edits** — 파일 생성:

`tasks/chain-edits/task.toml`:

```toml
prompt = "src/lib.rs에 세 가지를 수정해줘: (1) MAX_RETRIES를 5로 올리고, (2) greeting()이 \"안녕하세요\"를 반환하게 바꾸고, (3) backoff_ms를 지수 백오프(100 * 2^attempt 밀리초)로 바꿔줘."
check = "cargo test"
protected = ["tests", "Cargo.toml"]
```

`tasks/chain-edits/fixture/Cargo.toml`: (name = "chain-edits")

`tasks/chain-edits/fixture/src/lib.rs`:

```rust
/// 재시도 상한
pub const MAX_RETRIES: u32 = 3;

/// 인사말
pub fn greeting() -> &'static str {
    "Hello"
}

/// 재시도 대기시간(ms)
pub fn backoff_ms(attempt: u32) -> u64 {
    (attempt as u64) * 100
}
```

`tasks/chain-edits/fixture/tests/edits.rs`:

```rust
use chain_edits::{backoff_ms, greeting, MAX_RETRIES};

#[test]
fn retries_raised() {
    assert_eq!(MAX_RETRIES, 5);
}

#[test]
fn korean_greeting() {
    assert_eq!(greeting(), "안녕하세요");
}

#[test]
fn exponential_backoff() {
    assert_eq!(backoff_ms(0), 100);
    assert_eq!(backoff_ms(1), 200);
    assert_eq!(backoff_ms(3), 800);
}
```

검증: 초기 FAIL(3개 테스트 모두), 골든 → PASS:

```rust
pub const MAX_RETRIES: u32 = 5;
```

```rust
pub fn greeting() -> &'static str {
    "안녕하세요"
}
```

```rust
pub fn backoff_ms(attempt: u32) -> u64 {
    100 * (1u64 << attempt)
}
```

- [ ] **Step 5: 과제 정의 일괄 검증** — 12개 과제가 하네스 로더를 통과하는지 (서버 불필요 — load_tasks는 서버 접속 전에 실행되지만, 여기서는 단위 테스트로 확인):

`src/eval/task.rs` tests에 추가:

```rust
    /// 리포지토리에 커밋된 실제 과제 세트가 로더 검증을 통과하는지
    #[test]
    fn shipped_task_set_is_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tasks");
        let tasks = load_tasks(&dir).unwrap();
        assert_eq!(tasks.len(), 12, "초기 과제 세트 12개");
        for t in &tasks {
            assert!(t.spec.protected.contains(&"tests".to_string()) || t.spec.protected.contains(&"Cargo.toml".to_string()),
                "{}: 판정 자산 보호 가이드 (스펙 §8)", t.name);
        }
    }
```

Run: `cargo test shipped_task_set && cargo clippy --all-targets -- -D warnings` → 기대: 통과

- [ ] **Step 6: 커밋** — `git add tasks src/eval/task.rs && git commit -m "feat: 평가 과제 9~12 — 리네임·명세 구현·컴파일 에러·연쇄 편집"`

---

### Task 13: 문서 갱신

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md` (한국어 사용자 문서 — 설계 완료 기준 5 "CLAUDE.md·사용법 문서 갱신")

- [ ] **Step 1: CLAUDE.md 갱신** (영문 유지 — 사용자 선호):
  - 헤더의 `M1-M3 done … M4 (eval harness) is next` → M4 상태로 갱신 (구현 완료, 기준선 측정은 Task 14 이후 반영)
  - Commands 섹션에 추가:
    - `cargo run -- eval tasks/ [--repeats N] [--seed N] [--timeout-scale F]` — eval harness; report table to stdout + `./.loco/eval/<stamp>/report.json` (+ per-run transcripts); harness exit 0 regardless of pass rate, 1 on harness errors (server down, bad task defs)
  - Architecture 섹션에 추가 (요지):
    - `eval`: in-process harness — per task×repeat: fixture → temp sandbox → `Agent::run` with `AutoApprover`+seed(base+repeat) → protected paths re-synced from fixture (deletes agent-added files — anti reward-hack) → `check` command exit code decides pass; check runs regardless of outcome. LLM error aborts the whole harness; Ctrl+C writes a partial report (`interrupted: true`, exit 1). Timeouts: `run_bounded` sets `ToolCtx.cancel` then waits `cancel_grace` so `run_command` kills its process group (no orphans) — same helper wires `-p` Ctrl+C (exit 2). `AgentOutcome::Cancelled` = loop-top cancel check, keeps the grace fast
    - `tools/exec.rs`: shared shell exec (process-group kill, CP949 fallback, middle-truncation) used by run_command and eval checks
    - `tasks/`: 12 zero-dependency cargo-crate tasks, `check = "cargo test"`, `protected = ["tests", "Cargo.toml"]`; `tasks/.gitattributes` keeps the CRLF fixture byte-exact
  - Notes에 추가: eval integration tests are `#[cfg(unix)]`-gated (sh-based checks); fixture crates are ignored by root cargo (not a workspace)

- [ ] **Step 2: README.md 갱신** (한국어):
  - `loco eval` 사용법 섹션 추가: `loco eval tasks/ [--repeats N] [--seed N] [--timeout-scale F]`, 판정 방식 한 줄(샌드박스 + protected 복원 + check 종료코드), 리포트 위치(`./.loco/eval/<타임스탬프>/report.json` + 실행별 기록), 종료 코드(정상 완료 0 — 통과율 무관, 하네스 에러 1, Ctrl+C 부분 리포트 1)
  - `-p` 모드 설명 갱신: Ctrl+C 시 실행 중이던 명령(자식 프로세스)까지 정리하고 종료 코드 2 — 기존 종료 코드 표와 일관되게
  - 기존 문서 톤·구성 유지 (M3에서 갱신된 형식 따름)

- [ ] **Step 3: 검증** — CLAUDE.md·README의 명령이 실제로 동작하는 형태인지 눈으로 재확인 (`cargo run -- eval tasks/ --repeats 3` 문법 등)

- [ ] **Step 4: 커밋** — `git commit -m "docs: CLAUDE.md·README에 M4 eval 하네스 반영"`

---

### Task 14: 라이브 기준선 측정 (사용자 협조 필요)

**Files:**
- Create: `docs/baselines.md`

이 태스크는 LM Studio가 필요하다 — 사용자에게 요청할 것.

- [ ] **Step 1: 스모크 (사용자)** — LM Studio에 gemma 4B급 로드 후:

```bash
cargo build --release
./target/release/loco eval tasks --repeats 1
```

기대: 12과제 실행, 표 + report.json. 하네스 버그(패닉, 잘못된 판정, 샌드박스 누수 등)가 보이면 수정 후 재실행.

- [ ] **Step 2: gemma 기준선 (사용자)** — `./target/release/loco eval tasks --repeats 3` 완주. 매우 느리면 `--timeout-scale 2`.

- [ ] **Step 3: qwen 기준선 (사용자)** — LM Studio에서 qwen 4B급으로 교체 후 동일 실행.

- [ ] **Step 4: 결과 기록** — `docs/baselines.md`에 모델별 표(전체 통과율, 과제별 통과율/평균 턴, 사용 커맨드라인과 시드, report.json의 위치) 작성. 요지를 CLAUDE.md 헤더(M4 done)에 반영.

- [ ] **Step 5: 커밋** — `git commit -m "docs: M4 기준선 측정 결과 (gemma/qwen 4B급)"`
