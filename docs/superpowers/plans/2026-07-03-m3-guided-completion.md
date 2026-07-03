# loco M3 — 가이드형 완성 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** write_file/edit_file/run_command + 확인 게이트(diff 미리보기, y/N) + `--auto` 가드레일 + 세션 기록 + 히스토리 절삭(§6) + 반복 감지 — 스펙 §12 M3, **v1 목표 달성 지점**.

**Architecture:** M2의 lib+thin bin 위에 얹는다. (1) `Approver` 트레이트(동기, `&mut dyn`)를 `Agent::run`의 파라미터로 주입 — 게이트는 finish처럼 루프가 처리하고, 미리보기는 `Tool::preview()` 드라이런으로 얻는다. (2) 툴 디스패치는 `spawn_blocking`으로 감싸고 `ToolCtx`의 취소 플래그로 run_command를 중단 가능하게 한다. 프로세스 트리 킬은 신규 크레이트 없이 Unix `process_group(0)`+`kill` 셸아웃 / Windows `taskkill /T /F`. (3) `session.rs`의 `Session`이 히스토리를 소유하고(jsonl 트랜스크립트 + §6 예산 패킹) `Agent::run`은 `&mut Session`을 받는다 — 절삭은 저장된 히스토리 자체를 변형한다(원문은 트랜스크립트에 이미 기록됨).

**Tech Stack:** M2 스택 + `similar`(diff 미리보기), `encoding_rs`(CP949 손실 디코딩) — 둘 다 스펙 §2 크레이트 목록에 있어 추가 승인 불필요. 그 외 크레이트 추가 금지(프로세스 킬도 셸아웃으로 해결).

**스펙:** `docs/superpowers/specs/2026-07-02-loco-design.md` (§3 반복 감지·역할 규칙, §4 툴·매칭 사다리, §5 게이트·가드레일, §6 컨텍스트, §7 CLI·세션 기록, §9 에러, §10 크로스플랫폼)

## Global Constraints

- Rust edition 2024. 의존성은 위 Tech Stack까지가 전부 — 그 외 크레이트 추가 필요 시 사용자 확인
- reqwest는 `default-features = false, features = ["json", "stream", "rustls-no-provider"]` + `rustls`(ring) 직접 의존 유지 — OpenSSL/aws-lc-sys 금지, `main()`의 ring 프로바이더 설치 줄 보존
- HTTP 클라이언트의 `.no_proxy()` 유지 — 네트워크는 설정된 엔드포인트로만
- 언어 규칙: 사용자 대상 CLI 메시지(확인 게이트 UI, Notice 포함)는 한국어. 식별자·시스템 프롬프트는 영어. **모델에게 반환되는 텍스트(툴 결과, 툴 에러, 거부 사유, 교정 메시지, 절삭 마커)는 영어** (스펙 §4). 스펙 §6의 `[결과 생략]` 마커도 모델에게 가는 텍스트이므로 `[tool result elided]`로 구현한다
- 에러 타입: `llm`/`tools` 모듈은 `thiserror`, 앱 레벨은 `anyhow`. 툴 실행 에러는 크래시가 아니라 모델에게 반환되는 데이터(스펙 §9)
- 각 태스크 완료 시점에 `cargo test` 전체 통과 + `cargo clippy --all-targets -- -D warnings` 클린
- 커밋 메시지는 conventional commits (제목 한국어 가능)
- 작업 브랜치: `feat/m3-guided` (Task 1 시작 전 `git checkout -b feat/m3-guided`)
- 작업 디렉터리: `/Users/sgj/develop/loco`

## M3 범위 밖 (M4+ 이연 — 구현 금지)

- `loco eval` 서브커맨드, 과제 세트, `--repeats`/`--seed` — M4
- A/B 교대 반복 감지, `finish_reason: length` 반복 감지 — 스펙 §3이 명시한 v1 사각지대 (`max_turns`가 상한)
- 펜스드 플레인텍스트 대체 편집 프로토콜 — 스펙 §4의 사전 등록 폴백, M4 평가 후 판단
- 세션 재개 기능 — 스펙 §7 "v1은 기록 전용"
- 플래너-이그제큐터, tree-sitter repo map 등 — M5+

## 승인된 설계 결정 (브레인스토밍 2026-07-03)

1. **Approver 트레이트**: 동기 트레이트, `Agent::run(…, approver: &mut dyn Approver, …)` 파라미터 주입. 구현체 `TtyApprover`(REPL)/`AutoApprover`(--auto)/`NonInteractiveApprover`(-p). TtyApprover의 y/N 프롬프트는 **의도적으로 동기 블로킹** — REPL의 `select!`가 프롬프트 중 Ctrl+C를 소비해 고아 stdin 리더를 만드는 것을 방지하고, rustyline이 Ctrl+C를 `Interrupted`(=거부)로 흡수한다
2. **미리보기**: `Tool::preview()` 드라이런. preview가 Err이면 게이트를 건너뛰고 그대로 디스패치한다(실행이 같은 에러를 내 모델에 되먹여짐 — 사용자를 성공할 수 없는 확인에 끌어들이지 않음)
3. **run_command**: 신규 크레이트 없이 — Unix `CommandExt::process_group(0)` + `kill -9 -- -PGID` 셸아웃, Windows `taskkill /T /F`. `spawn_blocking` 디스패치 + `ToolCtx` 취소 플래그
4. **Session 소유권**: `Session`이 히스토리+트랜스크립트+패킹 소유, 절삭은 저장 히스토리를 변형
5. **반복 감지가 Task 1** (M2 최종 리뷰어 권고) + 시스템 프롬프트 강화 한 줄
6. **deny 패턴은 --auto에서만 차단** (스펙 §5). 대화형은 사용자가 게이트 — 매치 시 [경고] 표시만 (CP949 콘솔 호환 위해 비ASCII 기호 지양 — 스피너 ASCII 프레임과 같은 이유)

## 파일 구조

```
src/
├── config.rs        (수정) auto_deny_patterns 기본 목록 내장
├── session.rs       (신규) Transcript(jsonl 기록) + Session(히스토리 소유, §6 패킹)
├── tools/
│   ├── mod.rs       (수정) ToolError 변형 추가, Tool::preview, ToolCtx{cancel,timeout}, Registry::get/guided, Send+Sync
│   ├── path.rs      (수정) confine_for_write — 미존재 경로 허용 변형
│   ├── eol.rs       (신규) normalize_eol/dominant_crlf/restore_eol
│   ├── diff.rs      (신규) render_diff (similar, 상한 포함)
│   ├── write_file.rs / edit_file.rs / run_command.rs (신규)
├── agent/
│   ├── mod.rs       (수정) 반복 감지, 게이트, spawn_blocking 디스패치, Session 통합, 컨텍스트 초과 재시도
│   ├── approval.rs  (신규) Approver/ApprovalRequest/Decision, Auto/NonInteractive, 패턴 컴파일
│   └── prompt.rs    (수정) 반복 금지 + edit_file 우선 규칙
├── ui/
│   ├── repl.rs      (수정) TtyApprover 배선, 취소 플래그, Session, /help
│   ├── gate.rs      (신규) TtyApprover (y/N 프롬프트, [경고] 표시)
│   └── status.rs    (수정) render_event 공용화, 스피너 TTY 게이트, format_action 확장
├── lib.rs           (수정) pub mod session
└── main.rs          (수정) --auto 플래그, approver 선택, 종료 코드
```

---

### Task 1: 반복(루프) 감지 + 프롬프트 강화 + length 빈 content 가드

스펙 §3: 동일 `(tool, args)` 3회 연속 → 교정 주입, 교정 후 2회 더 연속(=5회째) → 조기 종료. M2 리뷰어 권고 근거: gemma-4-e4b가 list_files/grep만 반복하다 max_turns 소진.

**Files:**
- Modify: `src/agent/mod.rs`, `src/agent/prompt.rs`, `src/main.rs`, `src/ui/repl.rs`

**Interfaces:**
- Consumes: 기존 `Agent::run` 루프, `AgentOutcome`
- Produces: `AgentOutcome::RepetitionStop` (repl/main이 매칭 — 종료 코드 2), `agent::REPEAT_CORRECTION: &str`

- [ ] **Step 1: 브랜치 생성**

```bash
cd /Users/sgj/develop/loco && git checkout -b feat/m3-guided
```

- [ ] **Step 2: 실패하는 테스트 작성** — `src/agent/mod.rs` tests 모듈에 추가

```rust
#[tokio::test]
async fn five_identical_calls_stop_early_with_one_correction() {
    let dir = tempfile::tempdir().unwrap();
    let same = || ok(&turn("list_files", serde_json::json!({})));
    let script = Scripted::new(vec![same(), same(), same(), same(), same()]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut history = agent.initial_history();
    let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::RepetitionStop));
    assert_eq!(script.requests.lock().unwrap().len(), 5, "5회째 응답까지 받고 종료");
    // 교정은 3회째에 정확히 1번, 툴 결과와 같은 user 메시지에 병합 (스펙 §3 다중 피드백 병합)
    let corrections: Vec<_> = history
        .iter()
        .filter(|m| m.content.contains("repeating the same tool call"))
        .collect();
    assert_eq!(corrections.len(), 1);
    assert_eq!(corrections[0].role, "user");
    assert!(corrections[0].content.contains("</tool_result>"), "툴 결과 메시지에 병합");
}

#[tokio::test]
async fn different_args_reset_the_repeat_counter() {
    let dir = tempfile::tempdir().unwrap();
    let a = || ok(&turn("list_files", serde_json::json!({})));
    let b = || ok(&turn("list_files", serde_json::json!({"depth": 1})));
    let script = Scripted::new(vec![a(), a(), b(), a(), a(), ok(&finish("ok"))]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut history = agent.initial_history();
    let outcome = run_quiet(&mut agent, &mut history, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)), "교대하면 감지 안 됨");
    assert!(!history.iter().any(|m| m.content.contains("repeating the same tool call")));
}

#[tokio::test]
async fn empty_length_response_gets_placeholder_content() {
    let dir = tempfile::tempdir().unwrap();
    let script = Scripted::new(vec![ok_with_reason("", "length"), ok(&finish("ok"))]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut history = agent.initial_history();
    run_quiet(&mut agent, &mut history, "x").await.unwrap();
    // 빈 assistant content를 거부하는 템플릿 대비 (파싱 실패 경로와 동일 정책)
    assert!(!history.iter().any(|m| m.role == "assistant" && m.content.is_empty()));
}
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test five_identical -- --nocapture`
Expected: FAIL — `RepetitionStop` variant 없음 (컴파일 에러)

- [ ] **Step 4: 구현** — `src/agent/mod.rs`

`AgentOutcome`에 변형 추가:

```rust
    /// 동일 (tool, args) 5회 연속 — 조기 종료 (스펙 §3), -p 종료 코드 2
    RepetitionStop,
```

상수 추가 (파일 상단, `PARSE_ATTEMPTS` 근처):

```rust
/// 반복 3회째에 주입하는 교정 (스펙 §3). 모델 대상 — 영어
pub const REPEAT_CORRECTION: &str = "You are repeating the same tool call with the same arguments. \
Its result will not change. Try a different action, or call `finish` with your answer.";
```

`run()` 루프에 감지 로직 (while 위에 지역 변수, finish 처리 **뒤**·Action 이벤트 **앞**에 판정):

```rust
        let mut last_action_key: Option<String> = None;
        let mut repeat_count = 0usize;
        let mut corrected = false;
        while turns < self.max_turns {
            // ... (기존 코드: chat, length, 파싱, thought, finish 처리)

            // 반복 감지 (스펙 §3). finish는 위에서 이미 return/continue 했으므로
            // 계수 대상이 아니다 — summary 없는 finish 반복은 max_turns가 상한
            // (A/B 교대·length 반복과 함께 §3이 명시한 v1 사각지대, 의도된 것)
            let key = format!("{}|{}", turn.action.tool, turn.action.args);
            if last_action_key.as_deref() == Some(key.as_str()) {
                repeat_count += 1;
            } else {
                last_action_key = Some(key);
                repeat_count = 1;
                corrected = false;
            }
            if repeat_count >= 5 {
                on_event(AgentEvent::Notice(
                    "(같은 툴 호출이 5회 반복돼 조기 종료합니다)".to_string(),
                ));
                return Ok(AgentOutcome::RepetitionStop);
            }

            on_event(AgentEvent::Action { /* 기존 그대로 */ });
            let body = /* 기존 디스패치 그대로 */;
            // 교정은 툴 결과와 하나의 user 메시지로 병합 (스펙 §3 — 연속 user 금지)
            let mut msg = tool_result_message(&turn.action.tool, &body);
            if repeat_count == 3 && !corrected {
                corrected = true;
                msg.content = format!("{}\n\n{}", msg.content, REPEAT_CORRECTION);
                on_event(AgentEvent::Notice("(반복 감지 — 교정 메시지 주입)".to_string()));
            }
            history.push(msg);
            turns += 1;
        }
```

length 경로 빈 content 가드 (기존 `history.push(ChatMessage::assistant(resp.text()))` 줄 교체):

```rust
            if resp.finish_reason() == Some("length") {
                let t = resp.text();
                history.push(ChatMessage::assistant(if t.is_empty() { "(empty)" } else { t }));
                // ... 이하 기존 그대로
```

- [ ] **Step 5: 프롬프트 강화** — `src/agent/prompt.rs`의 Rules 블록에 한 줄 추가

```text
- Never repeat a tool call that already returned a result - reuse that result. As soon as you have enough information, call `finish`.
```

(기존 `- One tool call per turn.` 다음 줄에 삽입)

- [ ] **Step 6: 출구 배선** — `src/main.rs` `run_oneshot`의 match에 arm 추가:

```rust
        AgentOutcome::RepetitionStop => {
            eprintln!("(같은 툴 호출 반복으로 조기 종료 — 요청을 바꿔 다시 시도하세요)");
            Ok(ExitCode::from(2))
        }
```

`src/ui/repl.rs` `run_agent_turn`의 match에 arm 추가 (롤백 없음 — 진행된 히스토리는 유효):

```rust
        Some(Ok(AgentOutcome::RepetitionStop)) => {
            println!("(같은 툴 호출을 반복해 조기 종료했습니다 — 요청을 바꿔보세요)");
        }
```

- [ ] **Step 7: 전체 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS (기존 max_turns 테스트 포함 — 반복 5회 미만 스크립트는 영향 없음), clippy 클린

- [ ] **Step 8: 커밋**

```bash
git add -A && git commit -m "feat: 반복 감지와 교정 주입 — 5회 연속 시 조기 종료"
```

---

### Task 2: 확인 게이트 코어 — Approver 트레이트 + Tool::preview + 루프 게이트

**Files:**
- Create: `src/agent/approval.rs`
- Modify: `src/agent/mod.rs`, `src/tools/mod.rs`, `src/ui/repl.rs`, `src/main.rs`

**Interfaces:**
- Consumes: `Tool::is_mutating()` (M2에 이미 존재, 기본 false)
- Produces:
  - `tools::Tool::preview(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError>` (기본: `Ok(args.to_string())`)
  - `tools::Registry::get(&self, name: &str) -> Option<&dyn Tool>`
  - `agent::approval::{Approver, ApprovalRequest, Decision, AutoApprover, NonInteractiveApprover}` (agent/mod.rs에서 `pub use`)
  - `Agent::run(&mut self, history, request, approver: &mut dyn Approver, on_event)` — **시그니처 변경**, 모든 호출부 갱신 필요

- [ ] **Step 1: 실패하는 테스트 작성** — `src/agent/mod.rs` tests에 추가

```rust
    use crate::agent::approval::{ApprovalRequest, Approver, Decision};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// 실행 횟수를 세는 가짜 mutating 툴
    struct MutTool(Arc<AtomicUsize>);
    impl crate::tools::Tool for MutTool {
        fn name(&self) -> &'static str { "mut_tool" }
        fn doc(&self) -> &'static str { "mut_tool(): test." }
        fn is_mutating(&self) -> bool { true }
        fn preview(&self, _a: &serde_json::Value, _c: &ToolCtx) -> Result<String, crate::tools::ToolError> {
            Ok("PREVIEW-TEXT".to_string())
        }
        fn run(&self, _a: &serde_json::Value, _c: &ToolCtx) -> Result<String, crate::tools::ToolError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("mutated".to_string())
        }
    }

    struct ScriptedApprover {
        decisions: Mutex<VecDeque<Decision>>,
        seen: Mutex<Vec<(String, String)>>, // (tool, preview)
    }
    impl Approver for &ScriptedApprover {
        fn approve(&mut self, req: &ApprovalRequest<'_>) -> Decision {
            self.seen.lock().unwrap().push((req.tool.to_string(), req.preview.to_string()));
            self.decisions.lock().unwrap().pop_front().expect("결정 스크립트 소진")
        }
    }

    fn mut_agent(script: &Scripted, hits: Arc<AtomicUsize>, root: std::path::PathBuf) -> Agent<&Scripted> {
        let config = Config::default();
        let reg = Registry::new(vec![Box::new(MutTool(hits))]);
        Agent::new(script, reg, ToolCtx { root }, "test-model".into(), &config)
    }

    #[tokio::test]
    async fn denied_action_is_not_executed_and_reason_reaches_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let script = Scripted::new(vec![ok(&turn("mut_tool", serde_json::json!({}))), ok(&finish("ok"))]);
        let mut agent = mut_agent(&script, hits.clone(), dir.path().to_path_buf());
        let approver = ScriptedApprover {
            decisions: Mutex::new(vec![Decision::Deny { reason: "nope".into() }].into()),
            seen: Mutex::new(Vec::new()),
        };
        let mut history = agent.initial_history();
        let outcome = agent.run(&mut history, "x", &mut (&approver), &mut |_| {}).await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        assert_eq!(hits.load(Ordering::SeqCst), 0, "거부된 액션은 실행 금지");
        assert!(history.iter().any(|m| m.role == "user" && m.content.contains("Denied: nope")));
        let seen = approver.seen.lock().unwrap();
        assert_eq!(seen[0], ("mut_tool".to_string(), "PREVIEW-TEXT".to_string()));
    }

    #[tokio::test]
    async fn approved_action_executes() {
        let dir = tempfile::tempdir().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let script = Scripted::new(vec![ok(&turn("mut_tool", serde_json::json!({}))), ok(&finish("ok"))]);
        let mut agent = mut_agent(&script, hits.clone(), dir.path().to_path_buf());
        let approver = ScriptedApprover {
            decisions: Mutex::new(vec![Decision::Approve].into()),
            seen: Mutex::new(Vec::new()),
        };
        let mut history = agent.initial_history();
        agent.run(&mut history, "x", &mut (&approver), &mut |_| {}).await.unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert!(history.iter().any(|m| m.content.contains("mutated")));
    }

    /// 읽기 툴은 approver를 부르지 않는다 — 불리면 패닉
    struct PanicApprover;
    impl Approver for PanicApprover {
        fn approve(&mut self, _r: &ApprovalRequest<'_>) -> Decision {
            panic!("읽기 툴에 게이트가 걸림");
        }
    }

    #[tokio::test]
    async fn read_tools_bypass_the_gate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "a.txt"}))),
            ok(&finish("ok")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut history = agent.initial_history();
        let outcome = agent.run(&mut history, "x", &mut PanicApprover, &mut |_| {}).await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
    }
```

기존 `run_quiet` 헬퍼도 approver를 받도록 수정:

```rust
    async fn run_quiet(
        agent: &mut Agent<&Scripted>,
        history: &mut Vec<ChatMessage>,
        request: &str,
    ) -> Result<AgentOutcome, LlmError> {
        agent.run(history, request, &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test denied_action -- --nocapture`
Expected: FAIL (approval 모듈 없음 — 컴파일 에러)

- [ ] **Step 3: `src/agent/approval.rs` 작성**

```rust
//! 확인 게이트 (스펙 §5). 게이트 판단은 Agent 루프가, 결정은 Approver가 담당한다.

/// mutating 액션 하나에 대한 확인 요청
pub struct ApprovalRequest<'a> {
    pub tool: &'a str,
    pub args: &'a serde_json::Value,
    /// Tool::preview() 결과 (diff, 명령어 등) — 사용자에게 보여줄 내용
    pub preview: &'a str,
}

pub enum Decision {
    Approve,
    /// reason은 tool_result로 모델에 전달된다 — 영어 (스펙 §4)
    Deny { reason: String },
}

/// 동기 트레이트로 유지한다: TtyApprover가 의도적으로 블로킹해 REPL select!의
/// Ctrl+C 소비(고아 stdin 리더)를 막는다 — 설계 결정 1 참조
pub trait Approver {
    fn approve(&mut self, req: &ApprovalRequest<'_>) -> Decision;
}

/// --auto: 전부 승인 (deny 패턴 차단은 Task 7에서 추가)
#[derive(Default)]
pub struct AutoApprover;

impl Approver for AutoApprover {
    fn approve(&mut self, _req: &ApprovalRequest<'_>) -> Decision {
        Decision::Approve
    }
}

/// -p에서 --auto 없음: 프롬프트를 띄우지 않고 거부한다 (스펙 §7)
pub struct NonInteractiveApprover;

impl Approver for NonInteractiveApprover {
    fn approve(&mut self, _req: &ApprovalRequest<'_>) -> Decision {
        Decision::Deny {
            reason: "mutating tools are unavailable in non-interactive mode; \
                     the user must re-run loco with --auto to allow them"
                .to_string(),
        }
    }
}
```

`src/agent/mod.rs` 상단에 `pub mod approval;`과 `pub use approval::{ApprovalRequest, Approver, Decision};` 추가.

- [ ] **Step 4: `tools/mod.rs` — preview 기본 구현과 Registry::get**

`Tool` 트레이트에 추가:

```rust
    /// 확인 게이트에 보여줄 미리보기 (diff/명령어). mutating 툴은 재정의할 것.
    /// Err이면 게이트를 건너뛰고 실행 경로가 같은 에러를 모델에 되먹인다.
    /// 불변식: preview가 Err인 입력에서는 run도 반드시 실패해야 한다 —
    /// run이 성공할 수 있는 입력에서 preview만 실패하면 확인 없이 변이가 실행된다.
    /// (M3 세 툴은 preview가 run과 동일한 파싱·경로 확인·매칭을 먼저 수행해 충족)
    fn preview(&self, args: &serde_json::Value, _ctx: &ToolCtx) -> Result<String, ToolError> {
        Ok(args.to_string())
    }
```

`Registry`에 추가:

```rust
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|b| b.as_ref())
    }
```

- [ ] **Step 5: 루프 게이트** — `src/agent/mod.rs` `run()`: 시그니처에 `approver: &mut dyn Approver` 추가(on_event 앞), Action 이벤트와 디스패치 사이에 삽입:

```rust
            on_event(AgentEvent::Action { tool: &turn.action.tool, args: &turn.action.args });

            // 확인 게이트 (스펙 §5): mutating이고 미리보기가 가능할 때만.
            // preview Err → 게이트 생략, 아래 디스패치가 같은 에러를 되먹인다
            let gate_preview = self
                .registry
                .get(&turn.action.tool)
                .filter(|t| t.is_mutating())
                .map(|t| t.preview(&turn.action.args, &self.ctx));
            if let Some(Ok(preview)) = gate_preview {
                let req = ApprovalRequest { tool: &turn.action.tool, args: &turn.action.args, preview: &preview };
                if let Decision::Deny { reason } = approver.approve(&req) {
                    on_event(AgentEvent::Notice("(거부됨 — 모델에 전달)".to_string()));
                    let mut msg = tool_result_message(&turn.action.tool, &format!("Denied: {reason}"));
                    if repeat_count == 3 && !corrected {
                        corrected = true;
                        msg.content = format!("{}\n\n{}", msg.content, REPEAT_CORRECTION);
                    }
                    history.push(msg);
                    turns += 1;
                    continue;
                }
            }
```

- [ ] **Step 6: 호출부 갱신** — `src/ui/repl.rs`와 `src/main.rs`는 임시로 `AutoApprover`를 만들어 전달한다 (레지스트리가 아직 read_only라 게이트는 발동 불가; Task 8에서 TtyApprover로 교체):

```rust
use loco::agent::approval::AutoApprover; // main.rs / repl.rs 각각
// run 호출부:
let mut approver = AutoApprover::default();
... agent.run(history, text, &mut approver, &mut on_event) ...
```

agent tests의 나머지 직접 `run` 호출부도 `&mut AutoApprover::default()` 추가.

- [ ] **Step 7: 전체 확인 + 커밋**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS

```bash
git add -A && git commit -m "feat: 확인 게이트 코어 — Approver 트레이트와 Tool::preview"
```

---

### Task 3: write_file — diff/EOL 헬퍼 + confine_for_write 포함

**Files:**
- Create: `src/tools/eol.rs`, `src/tools/diff.rs`, `src/tools/write_file.rs`
- Modify: `Cargo.toml`(similar), `src/tools/mod.rs`(mod 선언), `src/tools/path.rs`(confine_for_write)

**Interfaces:**
- Consumes: `confine` 패턴 (path.rs), `Tool` 트레이트 + `preview`
- Produces:
  - `tools::eol::{normalize_eol(&str) -> String, dominant_crlf(&str) -> bool, restore_eol(&str, bool) -> String}`
  - `tools::diff::render_diff(old: &str, new: &str) -> String` (최대 120줄 + `[diff truncated]`)
  - `tools::path::confine_for_write(root: &Path, raw: &str) -> Result<PathBuf, ToolError>` — 대상 미존재 허용
  - `tools::write_file::WriteFile` (`is_mutating = true`)

- [ ] **Step 1: 의존성 추가**

```bash
cargo add similar@2
```

- [ ] **Step 2: 실패하는 테스트 작성**

`src/tools/eol.rs` (테스트 먼저 — 파일 생성, mod 선언 추가):

```rust
//! 라인엔딩 정책 (스펙 §4): 매칭·비교는 \n 정규화, 쓰기 시 지배적 EOL 복원

pub fn normalize_eol(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// CRLF가 lone LF보다 많으면 true (덮어쓰기 시 CRLF 유지 판단)
pub fn dominant_crlf(s: &str) -> bool {
    let crlf = s.matches("\r\n").count();
    let lf = s.matches('\n').count() - crlf;
    crlf > lf
}

pub fn restore_eol(s: &str, crlf: bool) -> String {
    if crlf { s.replace('\n', "\r\n") } else { s.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_and_restore_roundtrip() {
        assert_eq!(normalize_eol("a\r\nb\nc"), "a\nb\nc");
        assert_eq!(restore_eol("a\nb", true), "a\r\nb");
        assert_eq!(restore_eol("a\nb", false), "a\nb");
    }

    #[test]
    fn dominant_crlf_counts_majority() {
        assert!(dominant_crlf("a\r\nb\r\nc\n"));
        assert!(!dominant_crlf("a\nb\nc\r\n"));
        assert!(!dominant_crlf("no newline"));
    }
}
```

`src/tools/diff.rs`:

```rust
//! 확인 게이트용 diff 렌더링 (스펙 §4 — edit 적용 전 similar로 diff 표시)

pub const MAX_DIFF_LINES: usize = 120;

pub fn render_diff(old: &str, new: &str) -> String {
    let text = similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(2)
        .to_string();
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() > MAX_DIFF_LINES {
        let mut s = lines[..MAX_DIFF_LINES].join("\n");
        s.push_str("\n[diff truncated]");
        return s;
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_changed_lines_with_signs() {
        let d = render_diff("a\nb\nc\n", "a\nB\nc\n");
        assert!(d.contains("-b"), "{d}");
        assert!(d.contains("+B"), "{d}");
    }

    #[test]
    fn long_diff_is_truncated() {
        let old = String::new();
        let new: String = (0..500).map(|i| format!("line{i}\n")).collect();
        let d = render_diff(&old, &new);
        assert!(d.lines().count() <= MAX_DIFF_LINES + 1);
        assert!(d.ends_with("[diff truncated]"));
    }
}
```

`src/tools/path.rs` 테스트 추가:

```rust
    #[test]
    fn confine_for_write_allows_missing_target_inside_root() {
        let dir = root();
        let p = confine_for_write(dir.path(), "src/new_file.rs").unwrap();
        assert!(p.ends_with("src/new_file.rs"));
        let p2 = confine_for_write(dir.path(), "brand/new/dir/f.txt").unwrap();
        assert!(p2.ends_with("brand/new/dir/f.txt"));
    }

    #[test]
    fn confine_for_write_still_rejects_escapes() {
        let dir = root();
        for p in ["../x.txt", "/abs.txt", "C:/x", "src/../../x"] {
            assert!(matches!(
                confine_for_write(dir.path(), p).unwrap_err(),
                ToolError::PathViolation(_)
            ), "{p}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn confine_for_write_rejects_symlinked_dir_escape() {
        let dir = root();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).unwrap();
        let err = confine_for_write(dir.path(), "out/new.txt").unwrap_err();
        assert!(matches!(err, ToolError::PathViolation(_)));
    }
```

`src/tools/write_file.rs` 테스트 (파일 하단):

```rust
#[cfg(test)]
mod tests {
    use crate::tools::{Tool, ToolCtx};
    use super::WriteFile;

    fn ctx(dir: &tempfile::TempDir) -> ToolCtx {
        ToolCtx { root: dir.path().to_path_buf() }
    }

    #[test]
    fn creates_new_file_with_lf_and_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let out = WriteFile
            .run(&serde_json::json!({"path": "a/b/new.txt", "content": "one\r\ntwo"}), &ctx(&dir))
            .unwrap();
        assert!(out.contains("a/b/new.txt"));
        let written = std::fs::read_to_string(dir.path().join("a/b/new.txt")).unwrap();
        assert_eq!(written, "one\ntwo", "새 파일은 \\n (스펙 §4)");
    }

    #[test]
    fn overwrite_keeps_dominant_crlf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), "a\r\nb\r\n").unwrap();
        WriteFile
            .run(&serde_json::json!({"path": "f.txt", "content": "x\ny\n"}), &ctx(&dir))
            .unwrap();
        let written = std::fs::read(dir.path().join("f.txt")).unwrap();
        assert_eq!(String::from_utf8(written).unwrap(), "x\r\ny\r\n");
    }

    #[test]
    fn preview_is_a_diff_for_overwrite_and_lists_new_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), "old\n").unwrap();
        let p = WriteFile
            .preview(&serde_json::json!({"path": "f.txt", "content": "new\n"}), &ctx(&dir))
            .unwrap();
        assert!(p.contains("-old") && p.contains("+new"), "{p}");
        let p2 = WriteFile
            .preview(&serde_json::json!({"path": "fresh.txt", "content": "hello\n"}), &ctx(&dir))
            .unwrap();
        assert!(p2.contains("새 파일") && p2.contains("+hello"), "{p2}");
    }

    #[test]
    fn is_mutating_and_rejects_escape() {
        let dir = tempfile::tempdir().unwrap();
        assert!(WriteFile.is_mutating());
        assert!(WriteFile
            .run(&serde_json::json!({"path": "../x", "content": ""}), &ctx(&dir))
            .is_err());
    }
}
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test tools:: -- --nocapture` (mod 선언 후)
Expected: FAIL — `confine_for_write`, `WriteFile` 미구현

- [ ] **Step 4: 구현**

`src/tools/path.rs`에 추가:

```rust
/// confine의 쓰기 변형 (스펙 §4): 대상이 아직 없어도 된다. 렉시컬 정규화는 동일하게
/// 하고, **존재하는 가장 깊은 조상**을 canonicalize해 루트 안임을 검증한 뒤
/// 나머지 미존재 꼬리를 렉시컬로 잇는다 (미존재 구간에는 심링크가 있을 수 없음)
pub fn confine_for_write(root: &Path, raw: &str) -> Result<PathBuf, ToolError> {
    let normalized = raw.replace('\\', "/");
    if normalized.starts_with('/') || has_drive_prefix(&normalized) {
        return Err(ToolError::PathViolation(format!("absolute paths are not allowed: {raw}")));
    }
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for comp in Path::new(&normalized).components() {
        match comp {
            Component::Normal(c) => parts.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(ToolError::PathViolation(format!("path escapes the project root: {raw}")));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ToolError::PathViolation(format!("absolute paths are not allowed: {raw}")));
            }
        }
    }
    if parts.is_empty() {
        return Err(ToolError::PathViolation(format!("not a writable file path: {raw}")));
    }
    let canon_root = root.canonicalize()?;
    // 존재하는 가장 깊은 조상 찾기
    let mut existing = canon_root.clone();
    let mut rest_start = 0;
    for (i, p) in parts.iter().enumerate() {
        let next = existing.join(p);
        match next.canonicalize() {
            Ok(c) => {
                existing = c;
                rest_start = i + 1;
            }
            Err(_) => break,
        }
    }
    if !existing.starts_with(&canon_root) {
        return Err(ToolError::PathViolation(format!(
            "path resolves outside the project root (symlink?): {raw}"
        )));
    }
    let mut out = existing;
    for p in &parts[rest_start..] {
        out.push(p);
    }
    Ok(out)
}
```

`src/tools/write_file.rs`:

```rust
use serde::Deserialize;

use super::diff::render_diff;
use super::eol::{dominant_crlf, normalize_eol, restore_eol};
use super::path::confine_for_write;
use super::{Tool, ToolCtx, ToolError};

pub struct WriteFile;

#[derive(Deserialize)]
struct Args {
    path: String,
    content: String,
}

fn parse(args: &serde_json::Value) -> Result<Args, ToolError> {
    serde_json::from_value(args.clone()).map_err(|e| ToolError::BadArgs(e.to_string()))
}

/// 기존 파일이 UTF-8 텍스트면 Some(내용), 없거나 비UTF-8이면 None
fn existing_text(path: &std::path::Path) -> Option<String> {
    std::fs::read(path).ok().and_then(|b| String::from_utf8(b).ok())
}

impl Tool for WriteFile {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn doc(&self) -> &'static str {
        "write_file(path, content): Create a new file or overwrite an existing one with `content`. Prefer edit_file for small changes to existing files."
    }

    fn is_mutating(&self) -> bool {
        true
    }

    fn preview(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let path = confine_for_write(&ctx.root, &args.path)?;
        let new = normalize_eol(&args.content);
        Ok(match existing_text(&path) {
            Some(old) => format!(
                "write_file {} (덮어쓰기)\n{}",
                args.path,
                render_diff(&normalize_eol(&old), &new)
            ),
            None if path.exists() => {
                format!("write_file {} — 기존 비UTF-8 파일을 덮어씁니다", args.path)
            }
            None => format!("write_file {} (새 파일)\n{}", args.path, render_diff("", &new)),
        })
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let path = confine_for_write(&ctx.root, &args.path)?;
        let normalized = normalize_eol(&args.content);
        // 덮어쓰기: 기존 지배적 EOL 유지. 새 파일: \n (스펙 §4)
        let crlf = existing_text(&path).map(|old| dominant_crlf(&old)).unwrap_or(false);
        let text = restore_eol(&normalized, crlf);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &text)?;
        Ok(format!("Wrote {} ({} lines)", args.path, normalized.lines().count()))
    }
}
```

`src/tools/mod.rs` 상단에 `pub mod eol; pub mod diff; pub mod write_file;` 추가.

- [ ] **Step 5: 전체 확인 + 커밋**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS

```bash
git add -A && git commit -m "feat: write_file 툴 — diff 미리보기와 EOL 정책"
```

---

### Task 4: edit_file — 매칭 사다리

스펙 §4: 정확 일치 → 후행 공백 무시 → 균일 들여쓰기 시프트. 각 단계 0회 → 다음 단계, 2회 이상 → 즉시 모호성 에러. 매칭 전 양쪽 EOL 정규화, 쓰기 시 원본 EOL 복원.

**Files:**
- Create: `src/tools/edit_file.rs`
- Modify: `src/tools/mod.rs` (mod 선언 + `ToolError::EditFailed`), `src/agent/prompt.rs` (edit 우선 규칙)

**Interfaces:**
- Consumes: `eol::*`, `diff::render_diff`, `path::confine`
- Produces: `tools::edit_file::EditFile`, `tools::ToolError::EditFailed(String)`, 내부 `apply_edit(text, search, replace) -> Result<(String, MatchMode), String>`

- [ ] **Step 1: `ToolError` 변형 추가** — `src/tools/mod.rs`:

```rust
    #[error("edit failed: {0}")]
    EditFailed(String),
```

- [ ] **Step 2: 실패하는 테스트 작성** — `src/tools/edit_file.rs` 하단:

```rust
#[cfg(test)]
mod tests {
    use crate::tools::{Tool, ToolCtx, ToolError};
    use super::EditFile;

    fn setup(content: &str) -> (tempfile::TempDir, ToolCtx) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), content).unwrap();
        let ctx = ToolCtx { root: dir.path().to_path_buf() };
        (dir, ctx)
    }

    fn edit(ctx: &ToolCtx, search: &str, replace: &str) -> Result<String, ToolError> {
        EditFile.run(&serde_json::json!({"path": "f.rs", "search": search, "replace": replace}), ctx)
    }

    #[test]
    fn exact_match_replaces_once_and_reports_mode() {
        let (dir, ctx) = setup("fn a() {}\nfn b() {}\n");
        let out = edit(&ctx, "fn a() {}", "fn a() { todo!() }").unwrap();
        assert!(out.contains("exact"), "{out}");
        let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
        assert_eq!(t, "fn a() { todo!() }\nfn b() {}\n");
    }

    #[test]
    fn trailing_whitespace_is_ignored_at_stage_two() {
        let (dir, ctx) = setup("let x = 1;   \nlet y = 2;\n");
        let out = edit(&ctx, "let x = 1;\nlet y = 2;", "let x = 9;\nlet y = 2;").unwrap();
        assert!(out.contains("trailing"), "적용 모드 보고: {out}");
        let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
        assert!(t.contains("let x = 9;"));
    }

    #[test]
    fn uniform_indent_shift_matches_and_reindents_replacement() {
        let (dir, ctx) = setup("fn outer() {\n    if x {\n        do_it();\n    }\n}\n");
        // search는 들여쓰기 없이 — 4칸 시프트로 매칭돼야 함
        let out = edit(&ctx, "if x {\n    do_it();\n}", "if x {\n    do_other();\n}").unwrap();
        assert!(out.contains("indent"), "{out}");
        let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
        assert!(t.contains("        do_other();"), "치환문에 시프트 재적용:\n{t}");
    }

    #[test]
    fn two_exact_matches_is_an_immediate_ambiguity_error() {
        let (_d, ctx) = setup("dup();\ndup();\n");
        let err = edit(&ctx, "dup();", "x();").unwrap_err();
        assert!(matches!(err, ToolError::EditFailed(_)));
        assert!(err.to_string().contains("2 locations"), "{err}");
    }

    #[test]
    fn crlf_file_stays_crlf_after_edit() {
        let (dir, ctx) = setup("a\r\nb\r\nc\r\n");
        edit(&ctx, "b", "B").unwrap(); // search는 \n 세계에서 옴 (스펙 §4 매칭 규칙)
        let t = std::fs::read(dir.path().join("f.rs")).unwrap();
        assert_eq!(String::from_utf8(t).unwrap(), "a\r\nB\r\nc\r\n");
    }

    #[test]
    fn not_found_reports_near_miss_line() {
        let (_d, ctx) = setup("alpha\nbeta\ngamma\n");
        let err = edit(&ctx, "beta\nDELTA", "x").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"), "{msg}");
        assert!(msg.contains("Line 2"), "첫 줄 근접 위치 안내: {msg}");
    }

    #[test]
    fn preview_is_a_dry_run_diff_without_writing() {
        let (dir, ctx) = setup("keep\nold\n");
        let p = EditFile
            .preview(&serde_json::json!({"path": "f.rs", "search": "old", "replace": "new"}), &ctx)
            .unwrap();
        assert!(p.contains("-old") && p.contains("+new"), "{p}");
        let t = std::fs::read_to_string(dir.path().join("f.rs")).unwrap();
        assert_eq!(t, "keep\nold\n", "preview는 쓰지 않는다");
    }

    #[test]
    fn empty_search_is_bad_args() {
        let (_d, ctx) = setup("x\n");
        assert!(edit(&ctx, "", "y").is_err());
    }
}
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test edit_file -- --nocapture`
Expected: FAIL — EditFile 미구현

- [ ] **Step 4: 구현** — `src/tools/edit_file.rs`

```rust
use serde::Deserialize;

use super::diff::render_diff;
use super::eol::{dominant_crlf, normalize_eol, restore_eol};
use super::path::confine;
use super::{Tool, ToolCtx, ToolError};

pub struct EditFile;

#[derive(Deserialize)]
struct Args {
    path: String,
    search: String,
    replace: String,
}

#[derive(Debug, PartialEq)]
enum MatchMode {
    Exact,
    IgnoreTrailingWs,
    IndentShift(String),
}

impl MatchMode {
    fn describe(&self) -> String {
        match self {
            MatchMode::Exact => "exact".to_string(),
            MatchMode::IgnoreTrailingWs => "ignoring trailing whitespace".to_string(),
            MatchMode::IndentShift(i) => format!("indent-shifted by {} chars", i.len()),
        }
    }
}

fn parse(args: &serde_json::Value) -> Result<Args, ToolError> {
    let a: Args = serde_json::from_value(args.clone()).map_err(|e| ToolError::BadArgs(e.to_string()))?;
    if a.search.is_empty() {
        return Err(ToolError::BadArgs("`search` must not be empty".to_string()));
    }
    Ok(a)
}

/// 매칭 사다리 (스펙 §4). text/search/replace는 이미 \n 정규화된 상태.
/// Err 문자열은 모델에게 가는 영어 메시지
fn apply_edit(text: &str, search: &str, replace: &str) -> Result<(String, MatchMode), String> {
    // 1단계: 정확 일치
    let exact = text.match_indices(search).count();
    match exact {
        1 => return Ok((text.replacen(search, replace, 1), MatchMode::Exact)),
        n if n >= 2 => {
            return Err(format!(
                "search block matches {n} locations (exact match); add surrounding lines to make it unique"
            ));
        }
        _ => {}
    }

    let t_lines: Vec<&str> = text.split('\n').collect();
    let mut s_lines: Vec<&str> = search.split('\n').collect();
    while s_lines.last() == Some(&"") {
        s_lines.pop(); // search 끝의 빈 줄은 매칭에서 제외
    }
    let window = s_lines.len();
    if window == 0 || t_lines.len() < window {
        return Err(not_found_message(text, &s_lines));
    }

    // 2단계: 후행 공백 무시
    let stage2: Vec<usize> = (0..=t_lines.len() - window)
        .filter(|&i| {
            t_lines[i..i + window]
                .iter()
                .zip(&s_lines)
                .all(|(w, s)| w.trim_end() == s.trim_end())
        })
        .collect();
    match stage2.len() {
        1 => {
            let new = splice(&t_lines, stage2[0], window, &replace_lines(replace, ""));
            return Ok((new, MatchMode::IgnoreTrailingWs));
        }
        n if n >= 2 => {
            return Err(format!(
                "search block matches {n} locations (ignoring trailing whitespace); add surrounding lines to make it unique"
            ));
        }
        _ => {}
    }

    // 3단계: 균일 들여쓰기 시프트
    let stage3: Vec<(usize, String)> = (0..=t_lines.len() - window)
        .filter_map(|i| indent_of_match(&t_lines[i..i + window], &s_lines).map(|ind| (i, ind)))
        .collect();
    match stage3.len() {
        1 => {
            let (i, indent) = &stage3[0];
            let new = splice(&t_lines, *i, window, &replace_lines(replace, indent));
            Ok((new, MatchMode::IndentShift(indent.clone())))
        }
        n if n >= 2 => Err(format!(
            "search block matches {n} locations (with indent shift); add surrounding lines to make it unique"
        )),
        _ => Err(not_found_message(text, &s_lines)),
    }
}

/// 모든 줄이 동일한 indent 접두로 매칭되면 그 indent를 반환 (후행 공백은 무시)
fn indent_of_match(window: &[&str], search: &[&str]) -> Option<String> {
    let (i, s0) = search.iter().enumerate().find(|(_, l)| !l.trim().is_empty())?;
    let w0 = window[i].trim_end();
    let s0 = s0.trim_end();
    let indent = w0.strip_suffix(s0)?;
    if !indent.chars().all(|c| c == ' ' || c == '\t') {
        return None;
    }
    let ok = window.iter().zip(search).all(|(w, s)| {
        let (w, s) = (w.trim_end(), s.trim_end());
        if s.is_empty() { w.is_empty() } else { w == format!("{indent}{s}") }
    });
    ok.then(|| indent.to_string())
}

/// replace를 줄 단위로 나누고 비어 있지 않은 줄에 indent를 접두한다
fn replace_lines(replace: &str, indent: &str) -> Vec<String> {
    let mut lines: Vec<&str> = replace.split('\n').collect();
    while lines.last() == Some(&"") {
        lines.pop();
    }
    lines
        .into_iter()
        .map(|l| if l.trim().is_empty() { String::new() } else { format!("{indent}{l}") })
        .collect()
}

fn splice(t_lines: &[&str], start: usize, window: usize, replacement: &[String]) -> String {
    let mut out: Vec<String> = t_lines[..start].iter().map(|s| s.to_string()).collect();
    out.extend(replacement.iter().cloned());
    out.extend(t_lines[start + window..].iter().map(|s| s.to_string()));
    out.join("\n")
}

fn not_found_message(text: &str, s_lines: &[&str]) -> String {
    let first = s_lines.first().map(|l| l.trim()).unwrap_or("");
    if !first.is_empty() {
        if let Some(i) = text.split('\n').position(|l| l.contains(first)) {
            return format!(
                "search block not found. Line {} contains the first line of your block - \
                 re-read the file and copy the exact text including whitespace",
                i + 1
            );
        }
    }
    "search block not found - re-read the file and copy the exact text".to_string()
}

impl EditFile {
    /// 읽기 → 정규화 → 사다리 적용. (새 본문, 원본 CRLF 여부, 모드)
    fn dry_run(&self, args: &Args, ctx: &ToolCtx) -> Result<(String, String, bool, MatchMode), ToolError> {
        let path = confine(&ctx.root, &args.path)?;
        let bytes = std::fs::read(&path)?;
        let raw = String::from_utf8(bytes).map_err(|_| ToolError::NotUtf8(args.path.clone()))?;
        let crlf = dominant_crlf(&raw);
        let text = normalize_eol(&raw);
        let search = normalize_eol(&args.search);
        let replace = normalize_eol(&args.replace);
        let (new, mode) = apply_edit(&text, &search, &replace).map_err(ToolError::EditFailed)?;
        Ok((text, new, crlf, mode))
    }
}

impl Tool for EditFile {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn doc(&self) -> &'static str {
        "edit_file(path, search, replace): Replace one occurrence of `search` with `replace` in an existing file. `search` must match exactly one location; include a few surrounding lines to make it unique."
    }

    fn is_mutating(&self) -> bool {
        true
    }

    fn preview(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let (old, new, _crlf, mode) = self.dry_run(&args, ctx)?;
        Ok(format!("edit_file {} ({})\n{}", args.path, mode.describe(), render_diff(&old, &new)))
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let (_old, new, crlf, mode) = self.dry_run(&args, ctx)?;
        let path = confine(&ctx.root, &args.path)?;
        std::fs::write(&path, restore_eol(&new, crlf))?;
        Ok(format!("Edited {} (matched {})", args.path, mode.describe()))
    }
}
```

`src/tools/mod.rs`에 `pub mod edit_file;` 추가.

- [ ] **Step 5: 프롬프트 규칙 추가** — `src/agent/prompt.rs` Rules에 한 줄 (Task 1에서 넣은 줄 다음):

```text
- To change an existing file, prefer `edit_file` with a small unique search block. Use `write_file` only for new files or full rewrites.
```

- [ ] **Step 6: 전체 확인 + 커밋**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS

```bash
git add -A && git commit -m "feat: edit_file 툴 — 3단 매칭 사다리와 CRLF 보존"
```

---

### Task 5: ToolCtx 확장 + spawn_blocking 디스패치 + Ctrl+C 취소 배선

M2 이연분 해소: 동기 툴 디스패치 때문에 Ctrl+C가 툴 실행을 중단하지 못했다. run_command(60초) 도입 전에 기반을 깐다.

**Files:**
- Modify: `src/tools/mod.rs`, `src/agent/mod.rs`, `src/ui/repl.rs`, `src/main.rs`, 기존 툴 테스트들(ToolCtx 리터럴)

**Interfaces:**
- Consumes: 기존 `Registry::dispatch`
- Produces:
  - `ToolCtx { root: PathBuf, cancel: Arc<AtomicBool>, command_timeout: Duration }` + `ToolCtx::new(root: PathBuf) -> Self` (cancel=false, timeout=60s). **주의: repl/main 배선에서 반드시 `ctx.command_timeout = Duration::from_secs(config.command_timeout_secs)`를 대입할 것** — 이 대입이 없으면 설정 키가 파싱·표시만 되고 무시된다 (스펙 §4 "설정 가능" 위반, 테스트 게이트로는 안 잡힘)
  - `Registry`/`Tool`이 `Send + Sync` (`Box<dyn Tool + Send + Sync>`)
  - `Agent` 내부 필드 `registry: Arc<Registry>`, `ctx: Arc<ToolCtx>` (생성자 시그니처는 값 그대로 받아 내부에서 Arc 래핑 — 호출부 불변)

- [ ] **Step 1: 실패하는 테스트 작성** — `src/tools/mod.rs` tests:

```rust
    #[test]
    fn registry_and_ctx_are_send_sync_for_spawn_blocking() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Registry>();
        assert_send_sync::<ToolCtx>();
    }

    #[test]
    fn tool_ctx_new_defaults() {
        let c = ToolCtx::new(std::path::PathBuf::from("."));
        assert!(!c.cancel.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(c.command_timeout, std::time::Duration::from_secs(60));
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test tool_ctx_new`
Expected: FAIL (필드/생성자 없음)

- [ ] **Step 3: 구현**

`src/tools/mod.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub struct ToolCtx {
    pub root: PathBuf,
    /// Ctrl+C 시 REPL이 true로 — 장기 실행 툴(run_command)이 폴링해 자진 중단
    pub cancel: Arc<AtomicBool>,
    /// run_command 타임아웃 (config.command_timeout_secs)
    pub command_timeout: Duration,
}

impl ToolCtx {
    pub fn new(root: PathBuf) -> Self {
        Self { root, cancel: Arc::new(AtomicBool::new(false)), command_timeout: Duration::from_secs(60) }
    }
}
```

`Tool` 사용부의 트레이트 객체를 전부 `Box<dyn Tool + Send + Sync>`로 (Registry 필드, `new`, `get` 반환은 `&(dyn Tool + Send + Sync)`... `get`은 `Option<&(dyn Tool)>` 유지 가능 — 반환에는 바운드 불필요). 기존 `ToolCtx { root: ... }` 리터럴은 `ToolCtx::new(...)`로 일괄 교체 — 각 툴 테스트, repl, main뿐 아니라 **`src/agent/mod.rs` 테스트의 `make_agent`와 Task 2에서 추가한 `mut_agent`도 포함**.

`src/agent/mod.rs` — 필드를 Arc로, 디스패치를 spawn_blocking으로:

```rust
pub struct Agent<C: LlmClient> {
    client: C,
    registry: std::sync::Arc<Registry>,
    ctx: std::sync::Arc<ToolCtx>,
    // ... 나머지 기존 그대로
}
// Agent::new 내부: registry: Arc::new(registry), ctx: Arc::new(ctx)

// run() 디스패치 교체:
            let registry = std::sync::Arc::clone(&self.registry);
            let ctx = std::sync::Arc::clone(&self.ctx);
            let tool_name = turn.action.tool.clone();
            let tool_args = turn.action.args.clone();
            let dispatched =
                tokio::task::spawn_blocking(move || registry.dispatch(&tool_name, &tool_args, &ctx)).await;
            let body = match dispatched {
                Ok(Ok(s)) if s.is_empty() => "(no output)".to_string(),
                Ok(Ok(s)) => s,
                Ok(Err(e)) => format!("Error: {e}"),
                Err(join) => format!("Error: tool execution panicked: {join}"),
            };
```

(게이트의 `preview`는 빠른 읽기라 인라인 유지.)

- [ ] **Step 4: REPL 취소 배선** — `src/ui/repl.rs`:

```rust
    // run_repl 셋업 (root는 Task 11이 Transcript::create_under(&root)로 다시 쓰므로 clone):
    let mut ctx = ToolCtx::new(root.clone());
    ctx.command_timeout = std::time::Duration::from_secs(config.command_timeout_secs); // 설정 배선 — 생략 금지
    let cancel = ctx.cancel.clone();
    let mut agent = Agent::new(client, Registry::read_only(), ctx, model.to_string(), config);
    // run_agent_turn에 cancel: &std::sync::Arc<std::sync::atomic::AtomicBool> 파라미터 추가.
    // 턴 시작 시: cancel.store(false, Ordering::SeqCst);
    // select!의 ctrl_c arm에서: cancel.store(true, Ordering::SeqCst);
```

`src/main.rs` oneshot도 동일하게 `let mut ctx = ToolCtx::new(root.clone()); ctx.command_timeout = std::time::Duration::from_secs(config.command_timeout_secs);` (취소 배선은 REPL만 — -p의 Ctrl+C는 프로세스 종료). 설정값이 실제로 툴에 도달하는 검증은 Task 6의 `timeout_kills_the_process_tree_promptly` 테스트가 `ctx.command_timeout`을 직접 바꿔 수행한다 — repl/main의 대입 줄이 빠지면 컴파일은 통과하되 설정이 무시되므로, 이 두 줄은 리뷰 포인트다.

- [ ] **Step 5: 전체 확인 + 커밋**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS (기존 에이전트/툴 테스트가 회귀 게이트)

```bash
git add -A && git commit -m "refactor: 툴 디스패치를 spawn_blocking으로 — 취소 플래그 기반 마련"
```

---### Task 6: run_command 툴

스펙 §4·§10: cwd 고정, 타임아웃(설정), 출력 절삭, UTF-8→CP949 손실 디코딩, 프로세스 트리 킬.

**Files:**
- Create: `src/tools/run_command.rs`
- Modify: `Cargo.toml`(encoding_rs), `src/tools/mod.rs`(mod 선언)

**Interfaces:**
- Consumes: `ToolCtx::{cancel, command_timeout, root}`
- Produces: `tools::run_command::RunCommand` (`is_mutating = true`), 내부 `decode(bytes) -> String`, `truncate_middle(&str) -> String`

- [ ] **Step 1: 의존성 추가**

```bash
cargo add encoding_rs@0.8
```

- [ ] **Step 2: 실패하는 테스트 작성** — `src/tools/run_command.rs` 하단 (프로세스 테스트는 unix 전용, 헬퍼 테스트는 공통):

```rust
#[cfg(test)]
mod tests {
    // 주의: 외부 mod에는 decode/truncate_middle 테스트만 있다 — Tool/ToolCtx를
    // 여기서 import하면 unused import로 -D warnings 게이트에 걸린다 (unix 서브모듈이 자체 import)
    use super::*;

    #[test]
    fn decode_falls_back_to_euc_kr() {
        assert_eq!(decode("한글".as_bytes()), "한글");
        // "한글"의 CP949 인코딩: C7 D1 B1 DB
        assert_eq!(decode(&[0xC7, 0xD1, 0xB1, 0xDB]), "한글");
    }

    #[test]
    fn truncate_middle_keeps_head_and_tail() {
        let s = "x".repeat(20_000);
        let t = truncate_middle(&s);
        assert!(t.len() < 12_000);
        assert!(t.contains("output truncated"));
        let short = "short";
        assert_eq!(truncate_middle(short), "short");
    }

    #[cfg(unix)]
    mod unix {
        use super::super::*;
        use crate::tools::{Tool, ToolCtx};
        use std::time::{Duration, Instant};

        fn ctx() -> (tempfile::TempDir, ToolCtx) {
            let dir = tempfile::tempdir().unwrap();
            let ctx = ToolCtx::new(dir.path().to_path_buf());
            (dir, ctx)
        }

        #[test]
        fn runs_in_project_root_and_reports_exit_code() {
            let (dir, ctx) = ctx();
            std::fs::write(dir.path().join("here.txt"), "").unwrap();
            let out = RunCommand.run(&serde_json::json!({"command": "ls"}), &ctx).unwrap();
            assert!(out.contains("exit code: 0"), "{out}");
            assert!(out.contains("here.txt"), "cwd는 프로젝트 루트: {out}");
            let fail = RunCommand.run(&serde_json::json!({"command": "exit 3"}), &ctx).unwrap();
            assert!(fail.contains("exit code: 3"), "{fail}");
        }

        #[test]
        fn stderr_is_captured() {
            let (_d, ctx) = ctx();
            let out = RunCommand
                .run(&serde_json::json!({"command": "echo oops 1>&2"}), &ctx)
                .unwrap();
            assert!(out.contains("oops"), "{out}");
        }

        #[test]
        fn timeout_kills_the_process_tree_promptly() {
            let (_d, mut c) = ctx();
            c.command_timeout = Duration::from_millis(300);
            let start = Instant::now();
            let out = RunCommand.run(&serde_json::json!({"command": "sleep 30"}), &c).unwrap();
            assert!(start.elapsed() < Duration::from_secs(5), "타임아웃 후 즉시 반환");
            assert!(out.contains("timed out"), "{out}");
        }

        #[test]
        fn cancel_flag_aborts_early() {
            let (_d, ctx) = ctx();
            let cancel = ctx.cancel.clone();
            let h = std::thread::spawn(move || {
                RunCommand.run(&serde_json::json!({"command": "sleep 30"}), &ctx)
            });
            std::thread::sleep(Duration::from_millis(200));
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            let start = Instant::now();
            let out = h.join().unwrap().unwrap();
            assert!(start.elapsed() < Duration::from_secs(5));
            assert!(out.contains("cancelled"), "{out}");
        }

        #[test]
        fn background_grandchild_does_not_hang_the_tool() {
            // sh는 즉시 종료하지만 sleep이 stdout 파이프를 물고 남는다 —
            // join() 방식이면 여기서 5초(또는 영원히) 매달린다
            let (_d, ctx) = ctx();
            let start = Instant::now();
            let out = RunCommand.run(&serde_json::json!({"command": "sleep 5 &"}), &ctx).unwrap();
            assert!(start.elapsed() < Duration::from_secs(3), "READER_GRACE 내 반환");
            assert!(out.contains("exit code: 0"), "{out}");
            assert!(out.contains("output unavailable"), "파이프 점유 안내: {out}");
        }

        #[test]
        fn preview_shows_command_and_timeout() {
            let (_d, ctx) = ctx();
            let p = RunCommand.preview(&serde_json::json!({"command": "cargo test"}), &ctx).unwrap();
            assert!(p.contains("cargo test") && p.contains("60"), "{p}");
        }
    }
}
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test run_command`
Expected: FAIL — 미구현

- [ ] **Step 4: 구현** — `src/tools/run_command.rs`

```rust
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::{Tool, ToolCtx, ToolError};

/// stdout+stderr 합산 상한 (바이트). 초과분은 가운데를 잘라낸다 —
/// 명령 에코는 앞에, 에러 요약은 뒤에 있는 경우가 많다
const MAX_OUTPUT_BYTES: usize = 8_000;
/// try_wait 폴링 간격
const POLL: Duration = Duration::from_millis(50);
/// 종료 판정 후 파이프 리더 대기 상한. join()은 금지 — 백그라운드 손자가
/// 파이프를 물고 있으면(`sh -c "x &"` 또는 그룹 킬 실패) EOF가 영원히 안 와서
/// 툴이 무한 대기한다. 상한 초과 시 해당 출력은 포기하고 안내를 남긴다
const READER_GRACE: Duration = Duration::from_millis(500);

pub struct RunCommand;

#[derive(Deserialize)]
struct Args {
    command: String,
}

/// UTF-8 우선, 실패 시 EUC-KR(windows-949) 손실 디코딩 (스펙 §10 — 한국어 Windows 콘솔)
fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::EUC_KR.decode(bytes).0.into_owned(),
    }
}

fn truncate_middle(s: &str) -> String {
    if s.len() <= MAX_OUTPUT_BYTES {
        return s.to_string();
    }
    let mut head = MAX_OUTPUT_BYTES / 2;
    while !s.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = s.len() - MAX_OUTPUT_BYTES / 2;
    while !s.is_char_boundary(tail) {
        tail += 1;
    }
    format!(
        "{}\n[... output truncated ({} bytes total) ...]\n{}",
        &s[..head],
        s.len(),
        &s[tail..]
    )
}

/// 파이프 리더 — 결과를 채널로 보낸다. JoinHandle::join 대신 recv_timeout을
/// 쓸 수 있게 하기 위함 (READER_GRACE 주석 참조)
fn spawn_reader(r: Option<impl Read + Send + 'static>) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut r) = r {
            let _ = r.read_to_end(&mut buf);
        }
        let _ = tx.send(buf);
    });
    rx
}

/// (디코딩된 출력, 제시간에 EOF를 받았는지)
fn drain(rx: std::sync::mpsc::Receiver<Vec<u8>>) -> (String, bool) {
    match rx.recv_timeout(READER_GRACE) {
        Ok(bytes) => (decode(&bytes), true),
        Err(_) => (String::new(), false),
    }
}

fn shell_command(command: &str, ctx: &ToolCtx) -> Command {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut c = Command::new("sh");
        c.arg("-c").arg(command).current_dir(&ctx.root);
        // 자기만의 프로세스 그룹 — 타임아웃 킬이 손자까지 잡게 (스펙 §10)
        c.process_group(0);
        c
    }
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.args(["/C", command]).current_dir(&ctx.root);
        c
    }
}

/// 프로세스 트리 킬 (스펙 §10). 신규 크레이트 없이 시스템 유틸로:
/// Unix는 프로세스 그룹에 kill -9, Windows는 taskkill /T /F
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").args(["-9", "--", &format!("-{}", child.id())]).status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()]).status();
    }
    let _ = child.kill(); // 그룹 킬 실패 대비 직접 킬
}

enum Ended {
    Done(std::process::ExitStatus),
    TimedOut,
    Cancelled,
}

impl Tool for RunCommand {
    fn name(&self) -> &'static str {
        "run_command"
    }

    fn doc(&self) -> &'static str {
        "run_command(command): Run a shell command from the project root and return its exit code and output. Long output is truncated; commands are killed after the configured timeout."
    }

    fn is_mutating(&self) -> bool {
        true
    }

    fn preview(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone()).map_err(|e| ToolError::BadArgs(e.to_string()))?;
        Ok(format!(
            "$ {}\n(cwd: 프로젝트 루트, 타임아웃: {}초)",
            args.command,
            ctx.command_timeout.as_secs()
        ))
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone()).map_err(|e| ToolError::BadArgs(e.to_string()))?;
        let mut child = shell_command(&args.command, ctx)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let out_rx = spawn_reader(child.stdout.take());
        let err_rx = spawn_reader(child.stderr.take());

        let start = Instant::now();
        let ended = loop {
            if let Some(status) = child.try_wait()? {
                break Ended::Done(status);
            }
            if ctx.cancel.load(Ordering::SeqCst) {
                kill_tree(&mut child);
                let _ = child.wait();
                break Ended::Cancelled;
            }
            if start.elapsed() >= ctx.command_timeout {
                kill_tree(&mut child);
                let _ = child.wait();
                break Ended::TimedOut;
            }
            std::thread::sleep(POLL);
        };

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
        let body = truncate_middle(&body);

        Ok(match ended {
            Ended::Done(status) => {
                let code = status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "(terminated by signal)".to_string());
                format!("exit code: {code}\n{body}")
            }
            Ended::TimedOut => format!(
                "command timed out after {}s and was killed\n{body}",
                ctx.command_timeout.as_secs()
            ),
            Ended::Cancelled => format!("command was cancelled by the user\n{body}"),
        })
    }
}
```

`src/tools/mod.rs`에 `pub mod run_command;` 추가.

- [ ] **Step 5: 전체 확인 + 커밋**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS (macOS에서 unix 테스트 실행됨; Windows 경로는 cfg 컴파일만)

```bash
git add -A && git commit -m "feat: run_command 툴 — 프로세스 그룹 킬과 CP949 폴백"
```

---

### Task 7: auto_deny_patterns 기본 목록 + AutoApprover 차단

스펙 §5: `--auto`에서만 차단. 기본 목록은 크로스플랫폼, 최선 노력(defense-in-depth).

**Files:**
- Modify: `src/config.rs`, `src/agent/approval.rs`

**Interfaces:**
- Consumes: `Decision`, `Approver`
- Produces:
  - `config::default_deny_patterns() -> Vec<String>` (Config::default에 내장)
  - `approval::compile_patterns(&[String]) -> anyhow::Result<Vec<regex::Regex>>` (잘못된 패턴은 시작 시 실패 — fail fast)
  - `approval::first_deny_match<'a>(&'a [Regex], &serde_json::Value) -> Option<&'a str>`
  - `AutoApprover { deny: Vec<Regex> }` + `AutoApprover::new(&[String]) -> anyhow::Result<Self>` + `AutoApprover::from_compiled(Vec<Regex>) -> Self` (기존 `Default` 유지 — deny 없음)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/config.rs` tests — 기존 `defaults_match_spec`의 마지막 줄 교체:

```rust
        // 기본 차단 목록 내장 (스펙 §5 — M3)
        assert!(c.auto_deny_patterns.iter().any(|p| p.contains("sudo")));
        assert!(c.auto_deny_patterns.iter().any(|p| p.contains("git")));
        assert!(c.auto_deny_patterns.len() >= 11);
```

`src/agent/approval.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn auto() -> AutoApprover {
        AutoApprover::new(&crate::config::default_deny_patterns()).unwrap()
    }

    fn req_cmd(cmd: &str) -> serde_json::Value {
        serde_json::json!({"command": cmd})
    }

    #[test]
    fn dangerous_commands_are_denied_in_auto_mode() {
        let mut a = auto();
        for cmd in ["sudo rm x", "rm -rf /", "rm -fr .", "git push origin main", "dd if=/dev/zero"] {
            let args = req_cmd(cmd);
            let d = a.approve(&ApprovalRequest { tool: "run_command", args: &args, preview: "" });
            assert!(matches!(d, Decision::Deny { .. }), "{cmd}는 차단돼야 함");
        }
    }

    #[test]
    fn normal_commands_and_file_tools_pass() {
        let mut a = auto();
        let args = req_cmd("cargo test");
        assert!(matches!(
            a.approve(&ApprovalRequest { tool: "run_command", args: &args, preview: "" }),
            Decision::Approve
        ));
        let w = serde_json::json!({"path": "a.rs", "content": "x"});
        assert!(matches!(
            a.approve(&ApprovalRequest { tool: "write_file", args: &w, preview: "" }),
            Decision::Approve
        ));
    }

    #[test]
    fn invalid_pattern_fails_fast() {
        assert!(AutoApprover::new(&["(".to_string()]).is_err());
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test approval && cargo test defaults_match_spec`
Expected: FAIL

- [ ] **Step 3: 구현**

`src/config.rs` — 함수 추가, `Default`에서 사용:

```rust
/// --auto 가드레일 기본 차단 목록 (스펙 §5 원문 그대로 — 크로스플랫폼, 최선 노력).
/// 대소문자 무시로 컴파일된다 (PowerShell/cmd 관례)
pub fn default_deny_patterns() -> Vec<String> {
    [
        // Unix
        "sudo", r"rm\s+-\w*[rf]", "mkfs", r"dd\s+if=", "shutdown",
        // Windows
        r"rd\s+/s", r"del\s+/[fsq]", r"format\s", r"Remove-Item\s+.*-Recurse", r"reg\s+delete",
        // 공통
        r"git\s+push",
    ]
    .map(String::from)
    .to_vec()
}
// impl Default 안: auto_deny_patterns: default_deny_patterns(),
```

`src/agent/approval.rs`:

```rust
use regex::Regex;

/// 설정 문자열 → 대소문자 무시 Regex. 잘못된 패턴은 시작 시 에러 (fail fast)
pub fn compile_patterns(patterns: &[String]) -> anyhow::Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|p| {
            regex::RegexBuilder::new(p)
                .case_insensitive(true)
                .build()
                .map_err(|e| anyhow::anyhow!("auto_deny_patterns의 정규식이 잘못됨 ({p}): {e}"))
        })
        .collect()
}

/// run_command 인자가 차단 패턴에 걸리면 해당 패턴 문자열 반환
pub fn first_deny_match<'a>(patterns: &'a [Regex], args: &serde_json::Value) -> Option<&'a str> {
    let cmd = args.get("command")?.as_str()?;
    patterns.iter().find(|re| re.is_match(cmd)).map(|re| re.as_str())
}

#[derive(Default)]
pub struct AutoApprover {
    deny: Vec<Regex>,
}

impl AutoApprover {
    pub fn new(patterns: &[String]) -> anyhow::Result<Self> {
        Ok(Self { deny: compile_patterns(patterns)? })
    }
    pub fn from_compiled(deny: Vec<Regex>) -> Self {
        Self { deny }
    }
}

impl Approver for AutoApprover {
    fn approve(&mut self, req: &ApprovalRequest<'_>) -> Decision {
        if req.tool == "run_command" {
            if let Some(pat) = first_deny_match(&self.deny, req.args) {
                return Decision::Deny {
                    reason: format!("command blocked by auto_deny_patterns (matched `{pat}`)"),
                };
            }
        }
        Decision::Approve
    }
}
```

- [ ] **Step 4: 전체 확인 + 커밋**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS

```bash
git add -A && git commit -m "feat: --auto 가드레일 — 기본 차단 패턴과 AutoApprover 거부"
```

---

### Task 8: CLI/REPL 배선 — --auto, TtyApprover, Registry::guided, -p 거부

**Files:**
- Create: `src/ui/gate.rs`
- Modify: `src/main.rs`, `src/ui/repl.rs`, `src/ui/mod.rs`, `src/ui/status.rs`(format_action), `src/tools/mod.rs`(Registry::guided)

**Interfaces:**
- Consumes: `Approver`/`Decision`, `compile_patterns`/`first_deny_match`, `Spinner`
- Produces:
  - `tools::Registry::guided() -> Registry` — 6툴: read_file, list_files, grep, write_file, edit_file, run_command
  - `ui::gate::TtyApprover<'a>` + `ui::gate::answer_is_yes(&str) -> bool`
  - `run_repl(client, config, model, auto: bool)` — 시그니처 변경
  - clap `--auto` 플래그

- [ ] **Step 1: 실패하는 테스트 작성**

`src/tools/mod.rs` tests:

```rust
    #[test]
    fn guided_registry_has_all_six_tools() {
        let reg = Registry::guided();
        assert_eq!(
            reg.names(),
            vec!["read_file", "list_files", "grep", "write_file", "edit_file", "run_command"]
        );
    }
```

`src/ui/gate.rs` tests (파일 하단):

```rust
#[cfg(test)]
mod tests {
    use super::answer_is_yes;

    #[test]
    fn only_y_and_yes_mean_yes() {
        assert!(answer_is_yes("y"));
        assert!(answer_is_yes(" Y "));
        assert!(answer_is_yes("yes"));
        assert!(!answer_is_yes(""), "빈 입력(엔터)은 거부 — 기본값 N");
        assert!(!answer_is_yes("n"));
        assert!(!answer_is_yes("ㅇ"));
    }
}
```

`src/ui/status.rs` tests — `action_lines_are_compact` 수정. **주의: 기존 테스트는 "모르는 툴 → 인자 원문" 사례로 `run_command`를 쓰고 있다** (`"→ run_command {\"command\":\"ls\"}"` 단언, "M3에서 툴 늘어나도 동작" 주석). Step 3에서 전용 arm을 추가하면 이 단언이 red가 되므로 **삭제하고**, 모르는 툴 폴백은 진짜 미지 이름으로 대체한 뒤 새 단언들을 추가한다:

```rust
        // 기존 run_command 원문 단언은 삭제 — 모르는 툴 폴백은 미지 이름으로 검증
        assert_eq!(
            format_action("teleport", &serde_json::json!({"to": "moon"})),
            "→ teleport {\"to\":\"moon\"}"
        );
        assert_eq!(
            format_action("write_file", &serde_json::json!({"path": "a.rs", "content": "..."})),
            "→ write_file a.rs"
        );
        assert_eq!(
            format_action("edit_file", &serde_json::json!({"path": "a.rs", "search": "x", "replace": "y"})),
            "→ edit_file a.rs"
        );
        assert_eq!(
            format_action("run_command", &serde_json::json!({"command": "cargo test"})),
            "→ run_command cargo test"
        );
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test guided_registry && cargo test only_y`
Expected: FAIL

- [ ] **Step 3: 구현**

`src/tools/mod.rs`:

```rust
    /// M3 가이드형 툴 세트 (스펙 §4의 7툴 중 finish 제외 6개 — finish는 루프 담당)
    pub fn guided() -> Self {
        Self::new(vec![
            Box::new(read_file::ReadFile),
            Box::new(list_files::ListFiles),
            Box::new(grep::Grep),
            Box::new(write_file::WriteFile),
            Box::new(edit_file::EditFile),
            Box::new(run_command::RunCommand),
        ])
    }
```

`src/ui/status.rs` `format_action` — match에 arm 추가:

```rust
        "write_file" | "edit_file" => args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string(),
        "run_command" => args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string(),
```

`src/ui/gate.rs`:

```rust
//! 대화형 확인 게이트 (스펙 §5). 미리보기 표시 후 y/N.

use std::cell::RefCell;

use regex::Regex;

use crate::agent::approval::{first_deny_match, ApprovalRequest, Approver, Decision};
use crate::ui::status::Spinner;

pub fn answer_is_yes(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

pub struct TtyApprover<'a> {
    pub spinner: &'a RefCell<Spinner>,
    /// 대화형에서는 차단하지 않고 [경고] 표시만 (스펙 §5 — 사용자가 게이트)
    pub deny: &'a [Regex],
}

impl Approver for TtyApprover<'_> {
    fn approve(&mut self, req: &ApprovalRequest<'_>) -> Decision {
        self.spinner.borrow_mut().stop();
        println!("\n── 확인 필요: {} ──", req.tool);
        println!("{}", req.preview);
        if req.tool == "run_command" {
            if let Some(pat) = first_deny_match(self.deny, req.args) {
                println!("[경고] 차단 패턴에 해당하는 명령입니다: {pat}"); // 비ASCII 기호는 CP949 레거시 콘솔에서 깨진다
            }
        }
        // 의도적 동기 블로킹: REPL select!가 이 사이 Ctrl+C를 소비해 고아 stdin
        // 리더를 만드는 것을 방지한다. rustyline은 raw mode라 Ctrl+C가 SIGINT가
        // 아니라 Interrupted(→ 거부)로 돌아온다 — 승인된 설계 결정 1
        let answer = rustyline::DefaultEditor::new()
            .and_then(|mut rl| rl.readline("적용할까요? [y/N] "))
            .unwrap_or_default();
        if answer_is_yes(&answer) {
            Decision::Approve
        } else {
            println!("(거부함)");
            Decision::Deny {
                reason: "The user declined this action. Try a different approach, or call `finish`."
                    .to_string(),
            }
        }
    }
}
```

`src/ui/mod.rs`에 `pub mod gate;` 추가.

`src/main.rs`:

```rust
struct Cli {
    /// 단발 실행 프롬프트 (비대화형 에이전트 — 최종 답변만 stdout)
    #[arg(short, long)]
    prompt: Option<String>,
    /// 확인 게이트 전부 자동 승인 (auto_deny_patterns 차단은 유지)
    #[arg(long)]
    auto: bool,
}
// run(): run_repl(&client, &config, &model, cli.auto)
// run_oneshot(client, config, model, prompt, auto: bool):
//   Registry::guided() 사용, approver 선택:
    let mut auto_approver;
    let mut non_interactive = NonInteractiveApprover;
    let approver: &mut dyn Approver = if auto {
        auto_approver = AutoApprover::new(&config.auto_deny_patterns)?;
        &mut auto_approver
    } else {
        &mut non_interactive
    };
```

`src/ui/repl.rs` — `run_repl(client, config, model, auto: bool)`:

```rust
    let deny = compile_patterns(&config.auto_deny_patterns)?; // 셋업에서 1회 컴파일
    let mut agent = Agent::new(client, Registry::guided(), ctx, model.to_string(), config);
    // run_agent_turn에 auto: bool, deny: &[Regex] 전달. 내부에서:
    let mut tty;
    let mut auto_approver;
    let approver: &mut dyn Approver = if auto {
        auto_approver = AutoApprover::from_compiled(deny.to_vec());
        &mut auto_approver
    } else {
        tty = TtyApprover { spinner: &spinner, deny };
        &mut tty
    };
```

(`Regex`는 `Clone`이라 `deny.to_vec()` 가능.) `/help` 갱신 — **첫 줄도 교체**한다 (현재 첫 줄이 "툴(read_file/list_files/grep)"로 M2의 3툴만 언급해 아래 새 줄과 모순):

```rust
    println!("입력한 내용은 에이전트가 툴로 조사하고, 확인을 거쳐 수정·실행까지 수행합니다.");
    // ...기존 명령어 목록 줄들 유지, 마지막에 추가:
    println!("파일 수정·명령 실행은 미리보기 후 y/N 확인을 거칩니다 (--auto로 자동 승인).");
    println!("사용 가능한 툴: read_file, list_files, grep, write_file, edit_file, run_command");
```

- [ ] **Step 4: 전체 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 5: 대화형 스모크 (서버 없이 게이트만은 불가 — LM Studio 필요 시 연기 가능)**

LM Studio가 떠 있으면: `cargo run` → "hello.txt 파일 만들어서 인사말 써줘" → diff 미리보기 + `적용할까요? [y/N]` 확인 → `n` 거부 시 모델이 대안 시도하는지 확인. 서버가 없으면 이 스텝은 Task 13 스모크로 미룬다.

- [ ] **Step 6: 커밋**

```bash
git add -A && git commit -m "feat: 가이드형 배선 — --auto, TtyApprover, 6툴 레지스트리"
```

---

### Task 9: Transcript — 세션 기록 파일 (스펙 §7)

**Files:**
- Create: `src/session.rs`
- Modify: `src/lib.rs` (`pub mod session;`)

**Interfaces:**
- Consumes: 없음 (독립)
- Produces:
  - `session::Transcript` — `create_under(root: &Path) -> std::io::Result<Transcript>`, `disabled() -> Transcript`, `record(kind: &str, content: &str)`, `record_tool(tool: &str, args: &serde_json::Value, content: &str)`, `path(&self) -> Option<&Path>`
  - `session::utc_stamp(unix_secs: u64) -> String` — `"20260703T063456Z"` 형식 (Windows 파일명 안전한 ISO8601 basic)

- [ ] **Step 1: 실패하는 테스트 작성** — `src/session.rs` 하단:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_stamp_known_values() {
        assert_eq!(utc_stamp(0), "19700101T000000Z");
        assert_eq!(utc_stamp(86_399), "19700101T235959Z");
        assert_eq!(utc_stamp(951_782_400), "20000229T000000Z", "윤일");
    }

    #[test]
    fn create_under_makes_sessions_dir_and_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let t = Transcript::create_under(dir.path()).unwrap();
        let p = t.path().unwrap().to_path_buf();
        assert!(p.starts_with(dir.path().join(".loco/sessions")));
        assert_eq!(p.extension().unwrap(), "jsonl");
        let gi = std::fs::read_to_string(dir.path().join(".loco/.gitignore")).unwrap();
        assert_eq!(gi.trim(), "*", "커밋 오염 방지 (스펙 §7)");
    }

    #[test]
    fn records_are_one_json_per_line() {
        let dir = tempfile::tempdir().unwrap();
        let mut t = Transcript::create_under(dir.path()).unwrap();
        t.record("user", "질문");
        t.record_tool("read_file", &serde_json::json!({"path": "a.rs"}), "내용");
        let text = std::fs::read_to_string(t.path().unwrap()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["kind"], "user");
        assert_eq!(first["content"], "질문");
        assert!(first["ts"].as_str().unwrap().ends_with('Z'));
        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["kind"], "tool_result");
        assert_eq!(second["tool"], "read_file");
        assert_eq!(second["args"]["path"], "a.rs");
    }

    #[test]
    fn disabled_transcript_swallows_records() {
        let mut t = Transcript::disabled();
        t.record("user", "x"); // 패닉/에러 없어야 함
        assert!(t.path().is_none());
    }

    #[test]
    fn same_second_sessions_get_distinct_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = Transcript::create_under(dir.path()).unwrap();
        let b = Transcript::create_under(dir.path()).unwrap();
        assert_ne!(a.path().unwrap(), b.path().unwrap());
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test session::`
Expected: FAIL (모듈 없음)

- [ ] **Step 3: 구현** — `src/session.rs`

```rust
//! 세션 기록(스펙 §7)과 대화 상태(Task 10에서 Session 추가).
//! 기록은 최선 노력이다 — 기록 실패가 에이전트를 죽여선 안 된다.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Unix epoch 초 → "YYYYMMDDTHHMMSSZ" (ISO8601 basic — Windows 파일명에 `:` 불가).
/// chrono 없이 (의존성 고정): Howard Hinnant의 civil_from_days 알고리즘
pub fn utc_stamp(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let secs = unix_secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z", secs / 3600, (secs % 3600) / 60, secs % 60)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub struct Transcript {
    file: Option<File>,
    path: Option<PathBuf>,
}

impl Transcript {
    /// `<root>/.loco/sessions/<stamp>.jsonl` 생성 + `.loco/.gitignore`(`*`) 보장.
    /// 같은 초에 두 세션이 열리면 `-1`, `-2`… 접미로 회피
    pub fn create_under(root: &Path) -> std::io::Result<Transcript> {
        let dir = root.join(".loco/sessions");
        std::fs::create_dir_all(&dir)?;
        let gitignore = root.join(".loco/.gitignore");
        if !gitignore.exists() {
            std::fs::write(&gitignore, "*\n")?;
        }
        let stamp = utc_stamp(now_secs());
        for suffix in 0..10 {
            let name = if suffix == 0 { format!("{stamp}.jsonl") } else { format!("{stamp}-{suffix}.jsonl") };
            let path = dir.join(&name);
            match File::create_new(&path) {
                Ok(file) => return Ok(Transcript { file: Some(file), path: Some(path) }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::other("세션 파일 이름 충돌이 반복됨"))
    }

    /// 기록 없이 동작 (기록 디렉터리 생성 실패 시 폴백)
    pub fn disabled() -> Transcript {
        Transcript { file: None, path: None }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn write(&mut self, value: serde_json::Value) {
        if let Some(f) = &mut self.file {
            let _ = writeln!(f, "{value}"); // 최선 노력 — 실패 무시
        }
    }

    /// kind: user | assistant | system (스펙 §7)
    pub fn record(&mut self, kind: &str, content: &str) {
        self.write(serde_json::json!({"ts": utc_stamp(now_secs()), "kind": kind, "content": content}));
    }

    pub fn record_tool(&mut self, tool: &str, args: &serde_json::Value, content: &str) {
        self.write(serde_json::json!({
            "ts": utc_stamp(now_secs()), "kind": "tool_result", "content": content,
            "tool": tool, "args": args,
        }));
    }
}
```

`src/lib.rs`에 `pub mod session;` 추가.

- [ ] **Step 4: 전체 확인 + 커밋**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS

```bash
git add -A && git commit -m "feat: 세션 기록 — jsonl 트랜스크립트와 .loco/.gitignore"
```

---

### Task 10: Session — 히스토리 소유 + 스냅샷 + §6 예산 패킹

**Files:**
- Modify: `src/session.rs`

**Interfaces:**
- Consumes: `ChatMessage`, `Transcript`
- Produces (`src/session.rs`):
  - `session::estimate_tokens(&str) -> usize` — `utf8_bytes / 4` (스펙 §6)
  - `session::ELIDED: &str = "[tool result elided]"`
  - `Session::new(initial: Vec<ChatMessage>, transcript: Transcript) -> Session` (시스템 프롬프트 kind=system 기록)
  - `Session::{messages() -> &[ChatMessage], push(ChatMessage), push_tool_result(tool, args, body, note: Option<&str>), push_user_request(&str), record_extra(kind, content)}`
  - `Session::{snapshot() -> Snapshot, rollback(Snapshot)}` — **전체 복제 스냅샷** (M2의 `{len, tail}` 방식은 pack()이 런 중 히스토리를 줄이는 M3에서 스테일 tail로 무관한 메시지를 덮어쓴다)
  - `Session::pack(input_budget_tokens: usize)` — 저장 히스토리 변형
  - `session::tool_result_message(tool: &str, body: &str) -> ChatMessage` — session.rs에 **추가** (agent 쪽 사용 전환과 기존 private fn 제거는 Task 11 — 이 태스크는 agent를 건드리지 않는다)

- [ ] **Step 1: 실패하는 테스트 작성** — `src/session.rs` tests에 추가:

```rust
    use crate::llm::types::ChatMessage;

    fn sess(msgs: Vec<ChatMessage>) -> Session {
        Session::new(msgs, Transcript::disabled())
    }

    fn tool_msg(body: &str) -> ChatMessage {
        tool_result_message("grep", body)
    }

    #[test]
    fn estimate_is_utf8_bytes_over_four() {
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("한글"), 1, "한글 1자=3바이트 (스펙 §6)");
    }

    #[test]
    fn pack_under_budget_is_a_noop() {
        let mut s = sess(vec![ChatMessage::system("sys"), ChatMessage::user("hi")]);
        s.pack(1_000);
        assert_eq!(s.messages().len(), 2);
    }

    #[test]
    fn pack_elides_oldest_tool_results_first() {
        let big = "x".repeat(4_000); // ≈1000토큰
        let mut s = sess(vec![
            ChatMessage::system("sys"),
            ChatMessage::user("q"),
            ChatMessage::assistant("t1"),
            tool_msg(&big),
            ChatMessage::assistant("t2"),
            tool_msg(&big),
            ChatMessage::assistant("t3"),
        ]);
        s.pack(1_200);
        let elided: Vec<_> = s.messages().iter().filter(|m| m.content.contains(ELIDED)).collect();
        assert!(!elided.is_empty(), "오래된 툴 결과부터 생략");
        assert!(elided[0].content.starts_with("<tool_result"), "래퍼 보존:\n{}", elided[0].content);
        assert_eq!(s.messages().len(), 7, "생략 단계에선 메시지를 제거하지 않음");
    }

    #[test]
    fn pack_then_drops_oldest_user_assistant_pairs_atomically() {
        let mut msgs = vec![ChatMessage::system("sys")];
        for i in 0..10 {
            msgs.push(ChatMessage::user(format!("질문{} {}", i, "y".repeat(2_000))));
            msgs.push(ChatMessage::assistant(format!("답{} {}", i, "y".repeat(2_000))));
        }
        let mut s = sess(msgs);
        s.pack(1_000);
        assert!(s.messages().len() < 21);
        assert_eq!(s.messages()[0].role, "system", "시스템 프롬프트 보존");
        // role 교대 보존 (스펙 §6 — 쌍 단위 제거)
        for w in s.messages().windows(2) {
            assert!(!(w[0].role == w[1].role && w[0].role != "system"), "인접 동일 role");
        }
        // 마지막(현재 요청)은 보존
        assert!(s.messages().last().unwrap().content.starts_with("답9"));
    }

    #[test]
    fn snapshot_rollback_restores_tail_merge() {
        let mut s = sess(vec![ChatMessage::system("sys"), tool_msg("결과")]);
        let snap = s.snapshot();
        s.push_user_request("이어서"); // 꼬리 user에 병합 (길이 불변)
        assert!(s.messages().last().unwrap().content.contains("이어서"));
        s.rollback(snap);
        assert!(!s.messages().last().unwrap().content.contains("이어서"), "병합 원복");
        assert_eq!(s.messages().len(), 2);
    }

    #[test]
    fn rollback_after_pack_restores_exactly() {
        let mut msgs = vec![ChatMessage::system("sys")];
        for i in 0..4 {
            msgs.push(ChatMessage::user(format!("질문{} {}", i, "y".repeat(2_000))));
            msgs.push(ChatMessage::assistant(format!("답{i}")));
        }
        let mut s = sess(msgs.clone());
        let snap = s.snapshot();
        s.pack(500); // 쌍 제거로 히스토리가 스냅샷 시점보다 짧아진다
        assert!(s.messages().len() < msgs.len(), "전제: pack이 실제로 줄였음");
        s.rollback(snap);
        assert_eq!(s.messages(), &msgs[..], "pack 뒤에도 정확히 원복 — {{len,tail}} 방식은 여기서 깨진다");
    }

    #[test]
    fn push_user_request_merges_after_trailing_user() {
        let mut s = sess(vec![ChatMessage::system("sys"), tool_msg("결과")]);
        s.push_user_request("추가 요청");
        assert_eq!(s.messages().len(), 2, "연속 user 금지 — 병합 (스펙 §3)");
        let mut s2 = sess(vec![ChatMessage::system("sys")]);
        s2.push_user_request("첫 요청");
        assert_eq!(s2.messages().len(), 2);
    }

    #[test]
    fn push_tool_result_with_note_appends_in_same_message() {
        let mut s = sess(vec![ChatMessage::system("sys")]);
        s.push_tool_result("grep", &serde_json::json!({}), "body", Some("NOTE"));
        let last = s.messages().last().unwrap();
        assert!(last.content.contains("</tool_result>") && last.content.ends_with("NOTE"));
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test session::`
Expected: FAIL

- [ ] **Step 3: 구현** — `src/session.rs`에 추가:

```rust
use crate::llm::types::ChatMessage;

/// 모델에게 가는 생략 마커 — 영어 (스펙 §6의 "[결과 생략]"의 영어 구현)
pub const ELIDED: &str = "[tool result elided]";

/// bytes/4 휴리스틱 (스펙 §6 — chars/4는 한국어에서 과소추정)
pub fn estimate_tokens(s: &str) -> usize {
    s.len() / 4
}

/// 툴 결과 user 래핑 (스펙 §3 — role:"tool" 금지). agent에서 이동
pub fn tool_result_message(tool: &str, body: &str) -> ChatMessage {
    ChatMessage::user(format!("<tool_result name=\"{tool}\">\n{body}\n</tool_result>"))
}

/// 전체 복제 스냅샷. M2의 {len, tail} 방식은 pack()이 런 중 히스토리를 줄일 수 있는
/// M3에서 위험하다: len이 스냅샷보다 작아지면 truncate가 no-op이 되고 스테일 tail이
/// 무관한 현재 메시지를 덮어쓴다. 히스토리는 예산 상한(≈5.5K토큰 ≈ 22KB)이라
/// 복제 비용은 무시 가능
pub struct Snapshot {
    messages: Vec<ChatMessage>,
}

/// 대화 상태의 소유자: 히스토리 + 트랜스크립트 + §6 예산 패킹
pub struct Session {
    messages: Vec<ChatMessage>,
    transcript: Transcript,
}

impl Session {
    pub fn new(initial: Vec<ChatMessage>, mut transcript: Transcript) -> Session {
        for m in &initial {
            transcript.record(&m.role, &m.content);
        }
        Session { messages: initial, transcript }
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn push(&mut self, msg: ChatMessage) {
        self.transcript.record(&msg.role, &msg.content);
        self.messages.push(msg);
    }

    /// 툴 결과 + 선택적 교정 노트를 **하나의** user 메시지로 (스펙 §3 병합 규칙)
    pub fn push_tool_result(&mut self, tool: &str, args: &serde_json::Value, body: &str, note: Option<&str>) {
        self.transcript.record_tool(tool, args, body);
        let mut msg = tool_result_message(tool, body);
        if let Some(n) = note {
            self.transcript.record("user", n);
            msg.content = format!("{}\n\n{}", msg.content, n);
        }
        self.messages.push(msg);
    }

    /// 사용자 요청 — 꼬리가 user면 병합 (스펙 §3 role 교대), 아니면 push
    pub fn push_user_request(&mut self, request: &str) {
        self.transcript.record("user", request);
        match self.messages.last_mut() {
            Some(m) if m.role == "user" => m.content = format!("{}\n\n{}", m.content, request),
            _ => self.messages.push(ChatMessage::user(request)),
        }
    }

    /// 히스토리에 넣지 않는 부가 기록 (/chat 경로 등)
    pub fn record_extra(&mut self, kind: &str, content: &str) {
        self.transcript.record(kind, content);
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot { messages: self.messages.clone() }
    }

    /// 실패/중단 롤백 — 요청 이전 상태로 완전 복원 (꼬리 병합·pack 절삭 모두 안전)
    pub fn rollback(&mut self, snap: Snapshot) {
        self.messages = snap.messages;
    }

    fn total_tokens(&self) -> usize {
        self.messages.iter().map(|m| estimate_tokens(&m.content)).sum()
    }

    /// §6 절삭: ① 오래된 툴 결과 본문 생략 → ② 오래된 user+assistant 쌍 원자 제거.
    /// 시스템 프롬프트(0)와 마지막 메시지(현재 요청/결과)는 보존.
    /// 저장 히스토리 자체를 변형한다 — 원문은 트랜스크립트에 이미 있음
    pub fn pack(&mut self, input_budget_tokens: usize) {
        let last = self.messages.len().saturating_sub(1);
        for i in 1..last {
            if self.total_tokens() <= input_budget_tokens {
                return;
            }
            let m = &mut self.messages[i];
            if m.role == "user" && m.content.starts_with("<tool_result") && !m.content.contains(ELIDED) {
                let first_line = m.content.lines().next().unwrap_or("<tool_result>").to_string();
                m.content = format!("{first_line}\n{ELIDED}\n</tool_result>");
            }
        }
        while self.total_tokens() > input_budget_tokens && self.messages.len() > 3 {
            if self.messages[1].role == "user" && self.messages[2].role == "assistant" {
                self.messages.drain(1..=2);
            } else {
                self.messages.remove(1); // 교대가 어긋난 히스토리 — 하나씩 걷어내고 병합으로 복구
            }
            merge_adjacent_same_role(&mut self.messages);
        }
    }
}

/// 쌍 제거 후 교대 재검증 — 인접 동일 role은 병합 (스펙 §6)
fn merge_adjacent_same_role(msgs: &mut Vec<ChatMessage>) {
    let mut i = 1;
    while i < msgs.len() {
        if msgs[i].role == msgs[i - 1].role && msgs[i].role != "system" {
            let taken = msgs.remove(i);
            msgs[i - 1].content = format!("{}\n\n{}", msgs[i - 1].content, taken.content);
        } else {
            i += 1;
        }
    }
}
```

- [ ] **Step 4: 전체 확인 + 커밋**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS

```bash
git add -A && git commit -m "feat: Session — 히스토리 소유, 스냅샷 롤백, 예산 패킹"
```

---

### Task 11: Agent×Session 통합 + 컨텍스트 초과 절삭 재시도

`Agent::run`이 `&mut Session`을 받고, 매 턴 패킹하며, 컨텍스트 초과 400에 절삭 후 재시도한다(스펙 §9). REPL/-p 배선 포함.

**Files:**
- Modify: `src/agent/mod.rs`, `src/ui/repl.rs`, `src/main.rs`

**Interfaces:**
- Consumes: `Session` API 전부 (Task 10)
- Produces:
  - `Agent::run(&mut self, session: &mut Session, request: &str, approver: &mut dyn Approver, on_event) -> Result<AgentOutcome, LlmError>` — **시그니처 변경**
  - `Agent`에 `context_tokens: usize` 필드 (config에서) + `fn input_budget(&self) -> usize` = `(context_tokens − max_output_tokens) × 9 / 10`
  - `agent::tool_result_message` 제거 → `session::tool_result_message` 사용

- [ ] **Step 1: 실패하는 테스트 작성** — `src/agent/mod.rs` tests. 헬퍼 갱신:

```rust
    use crate::session::{Session, Transcript};

    fn new_session(agent: &Agent<&Scripted>) -> Session {
        Session::new(agent.initial_history(), Transcript::disabled())
    }
    // run_quiet: history 대신 session
    async fn run_quiet(
        agent: &mut Agent<&Scripted>,
        session: &mut Session,
        request: &str,
    ) -> Result<AgentOutcome, LlmError> {
        agent.run(session, request, &mut crate::agent::approval::AutoApprover::default(), &mut |_| {}).await
    }
```

기존 테스트는 `let mut history = agent.initial_history()` → `let mut session = new_session(&agent)`, 히스토리 검증은 `session.messages()`로 기계적 치환. **M2의 `context_overflow_400_propagates_without_touching_fallback_flags` 테스트는 삭제**한다 — 초과 400은 이제 즉시 전파가 아니라 절삭 후 2회 재시도이므로, 아래 두 신규 테스트가 그 검증(폴백 플래그 불변 포함)을 대체한다.

신규 테스트:

```rust
    #[tokio::test]
    async fn context_overflow_packs_and_retries_then_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let overflow = || Err(LlmError::Api { status: 400, body: "exceeds the available context size".into() });
        let script = Scripted::new(vec![overflow(), ok(&finish("살아남"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        // 크기 산정 주의: 기본 예산 5529토큰은 "통과"하되 축소 예산(>>1 = 2764)은
        // 초과하도록 심는다 — 첫 턴의 일반 패킹이 아니라 초과-재시도 경로가 절삭해야 함.
        // "빅".repeat(5000) = 15000바이트 ≈ 3750토큰
        session.push(ChatMessage::user("빅".repeat(5000)));
        session.push(ChatMessage::assistant("이전 답"));
        let outcome = run_quiet(&mut agent, &mut session, "질문").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)), "절삭 후 재시도로 회복");
        assert_eq!(script.requests.lock().unwrap().len(), 2);
        let reqs = script.requests.lock().unwrap();
        assert!(reqs[1].messages.len() < reqs[0].messages.len(), "재시도는 절삭된 히스토리");
    }

    #[tokio::test]
    async fn context_overflow_three_times_propagates_with_schema_intact() {
        let dir = tempfile::tempdir().unwrap();
        let overflow = || Err(LlmError::Api { status: 400, body: "context overflow".into() });
        let script = Scripted::new(vec![overflow(), overflow(), overflow()]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let err = run_quiet(&mut agent, &mut session, "x").await.unwrap_err();
        assert!(matches!(err, LlmError::Api { status: 400, .. }));
        assert_eq!(script.requests.lock().unwrap().len(), 3, "절삭 재시도 2회 후 전파 (스펙 §9)");
        let reqs = script.requests.lock().unwrap();
        assert!(reqs[2].response_format.is_some(), "폴백 사다리 오분류 금지 — json_schema 유지");
    }

    #[tokio::test]
    async fn every_turn_packs_to_budget() {
        // 툴 결과 2개를 쌓으면 세 번째 턴의 패킹이 "오래된" 쪽(마지막 메시지가 아닌)을
        // 생략해야 한다. pack은 마지막 메시지(방금 받은 결과)는 건드리지 않으므로
        // 결과가 하나뿐이면 이 테스트는 성립하지 않는다 — 반드시 두 번 읽는다.
        // 수치: 결과 각 ≈1500토큰, 예산 = (2500−100)×0.9 = 2160 → 둘 다 온전히는 못 담음.
        // 주의: 실측 시스템 프롬프트(~400토큰)도 예산에 계상된다 — 여유 ~230토큰.
        // 후속 마일스톤에서 프롬프트가 크게 자라면 이 수치를 재조정해야 한다
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big.txt"), "z".repeat(6_000)).unwrap();
        let read = || ok(&turn("read_file", serde_json::json!({"path": "big.txt"})));
        let script = Scripted::new(vec![read(), read(), ok(&finish("done"))]);
        let config = Config { context_tokens: 2_500, max_output_tokens: 100, ..Default::default() };
        let mut agent = Agent::new(
            &script, Registry::read_only(), ToolCtx::new(dir.path().to_path_buf()),
            "test-model".into(), &config,
        );
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "읽어").await.unwrap();
        let reqs = script.requests.lock().unwrap();
        let third = &reqs[2].messages;
        assert!(
            third.iter().any(|m| m.content.contains(crate::session::ELIDED)),
            "오래된 툴 결과는 생략된 채 전송"
        );
        assert!(
            third.iter().filter(|m| m.content.contains("zzzz")).count() >= 1,
            "최신 툴 결과는 온전히 유지"
        );
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test agent::`
Expected: FAIL (시그니처/패킹 미구현)

- [ ] **Step 3: 구현** — `src/agent/mod.rs`

- `Agent`에 `context_tokens: usize` 필드 (`Agent::new`에서 `config.context_tokens`), 헬퍼:

```rust
    /// 스펙 §6: (context − max_output) × 0.9
    fn input_budget(&self) -> usize {
        self.context_tokens.saturating_sub(self.max_output_tokens as usize) * 9 / 10
    }
```

- `run()` 진입부의 병합-or-push를 `session.push_user_request(request)`로 교체.
- 매 턴 첫 줄에서 `session.pack(self.input_budget());`
- `chat_with_fallback`: 컨텍스트 초과 arm에서 **Notice를 빼고** 즉시 `return Err(...)` (오분류 방지 로직·주석은 유지). 시그니처는 `&[ChatMessage]` 유지 — 호출은 `session.messages()`.
- 턴의 chat 호출을 절삭-재시도 루프로 감싼다:

```rust
            let resp = loop {
                match self.chat_with_fallback(session.messages(), on_event).await {
                    Err(LlmError::Api { status: 400, body })
                        if looks_like_context_overflow(&body) && overflow_shrinks < 2 =>
                    {
                        overflow_shrinks += 1;
                        on_event(AgentEvent::Notice(format!(
                            "(컨텍스트 초과로 보임 — 히스토리 절삭 후 재시도 {overflow_shrinks}/2)"
                        )));
                        session.pack(self.input_budget() >> overflow_shrinks);
                    }
                    Err(LlmError::Api { status: 400, body }) if looks_like_context_overflow(&body) => {
                        on_event(AgentEvent::Notice(
                            "(컨텍스트 초과 — context_tokens 설정과 서버 로드 설정을 확인하세요)".to_string(),
                        ));
                        return Err(LlmError::Api { status: 400, body });
                    }
                    other => break other?,
                }
            };
```

(`overflow_shrinks: u32`는 `run()` 최상단 지역 변수 — 실행당 2회.)

- 모든 `history.push(...)`를 Session API로: assistant/파싱 피드백은 `session.push(ChatMessage::...)`, 툴 결과는 `session.push_tool_result(&turn.action.tool, &turn.action.args, &body, note)` (note = 반복 교정 or None). 게이트 거부도 `push_tool_result(tool, args, &format!("Denied: {reason}"), note)`.
- `tool_result_message`/그 테스트는 session.rs로 이동, agent에서는 `use crate::session::tool_result_message;` (스키마 검증 등 기존 사용처 갱신).

- [ ] **Step 4: REPL/main 배선**

`src/ui/repl.rs`:

```rust
    // 셋업: 트랜스크립트 실패는 경고 후 비활성 (기록이 에이전트를 못 죽인다)
    let transcript = Transcript::create_under(&root).unwrap_or_else(|e| {
        println!("(세션 기록을 열지 못했습니다: {e} — 기록 없이 진행)");
        Transcript::disabled()
    });
    let mut session = Session::new(agent.initial_history(), transcript);
    // /clear: 새 세션 파일 (스펙 §7)
    Input::Clear => {
        session = Session::new(agent.initial_history(), Transcript::create_under(&root).unwrap_or_else(|_| Transcript::disabled()));
        chat_history.truncate(1);
        println!("(히스토리 초기화 — 새 세션 파일)");
    }
    // /chat 경로 기록: run_chat_turn에 &mut Session 전달,
    //   성공 시 session.record_extra("user", &text); session.record_extra("assistant", &full);
    // run_agent_turn: snapshot_len/snapshot_tail/rollback() 지역 로직 삭제 →
    //   let snap = session.snapshot(); ... 실패/중단 arm에서 session.rollback(snap);
    //   (rollback 함수와 그 주석은 Session으로 이동됐으므로 repl의 fn rollback 삭제)
```

`src/main.rs` `run_oneshot`: `Transcript::create_under(&root)` (실패 시 stderr 경고 + disabled), `Session::new`, `agent.run(&mut session, ...)`.

- [ ] **Step 5: 전체 확인 + 커밋**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS

```bash
git add -A && git commit -m "feat: Session 통합 — 매 턴 패킹과 컨텍스트 초과 절삭 재시도"
```

---

### Task 12: M2 이연 정리 — 스피너 게이트, 렌더 중복, /chat 안내, 테스트 공백

**Files:**
- Modify: `src/ui/status.rs`, `src/ui/repl.rs`, `src/main.rs`, `src/tools/read_file.rs`, `src/tools/list_files.rs`, `src/tools/grep.rs`, `src/agent/mod.rs`(테스트만)

**Interfaces:**
- Produces: `ui::status::render_event(ev: &AgentEvent<'_>, to_stderr: bool)`

- [ ] **Step 1: 스피너 TTY 게이트 수정** — 스피너는 stderr에 그리므로 stderr도 확인해야 한다 (stderr 리다이렉트 시 프레임 유입 방지). `Spinner::start`:

```rust
        if !(std::io::stdout().is_terminal() && std::io::stderr().is_terminal()) {
            return Self { task: None };
        }
```

테스트 `spinner_activity_follows_stdout_tty`를 갱신:

```rust
        assert_eq!(
            s.is_active(),
            std::io::stdout().is_terminal() && std::io::stderr().is_terminal()
        );
```

스피너 이중 stop 안전성 테스트 추가:

```rust
    #[tokio::test]
    async fn spinner_stop_is_idempotent() {
        let mut s = Spinner::start("x");
        s.stop();
        s.stop(); // 두 번째 stop은 no-op — 패닉/잔상 없음
        assert!(!s.is_active());
    }
```

- [ ] **Step 2: 이벤트 렌더링 공용화** — `src/ui/status.rs`:

```rust
use crate::agent::AgentEvent;

/// main(-p, stderr)과 repl(stdout)이 공유하는 이벤트 한 줄 렌더링
pub fn render_event(ev: &AgentEvent<'_>, to_stderr: bool) {
    let line = match ev {
        AgentEvent::Thought(t) => format!("· {t}"),
        AgentEvent::Action { tool, args } => format_action(tool, args),
        AgentEvent::Notice(n) => n.clone(),
    };
    if to_stderr { eprintln!("{line}") } else { println!("{line}") }
}
```

repl의 on_event 클로저 본문을 `spinner stop → render_event(&ev, false) → spinner 재시작`으로, main은 `render_event(&ev, true)`로 교체 (match 블록 중복 제거).

- [ ] **Step 3: bare /chat 안내** — `src/ui/repl.rs` match에 arm 추가 (`Input::Unknown` 일반 arm **앞에**):

```rust
            Input::Unknown(cmd) if cmd == "chat" => println!("사용법: /chat <메시지>"),
```

- [ ] **Step 4: 테스트 공백 메우기**

`src/tools/read_file.rs` tests:

```rust
    #[test]
    fn empty_file_says_so() {
        let (_d, ctx) = setup("");
        assert_eq!(run(&ctx, serde_json::json!({"path": "f.txt"})).unwrap(), "(empty file)");
    }

    #[test]
    fn limit_is_clamped_to_max() {
        let content: String = (1..=250).map(|i| format!("line{i}\n")).collect();
        let (_d, ctx) = setup(&content);
        let out = run(&ctx, serde_json::json!({"path": "f.txt", "limit": 9999})).unwrap();
        assert!(out.contains("line200") && !out.contains("line201\n"), "limit도 200 상한");
    }

    #[test]
    fn offset_and_limit_combine() {
        let content: String = (1..=50).map(|i| format!("line{i}\n")).collect();
        let (_d, ctx) = setup(&content);
        let out = run(&ctx, serde_json::json!({"path": "f.txt", "offset": 10, "limit": 3})).unwrap();
        assert!(out.starts_with("line10"));
        assert!(out.contains("line12") && !out.contains("line13"));
        assert!(out.contains("offset=13"), "이어 읽기 안내: {out}");
    }
```

`src/tools/list_files.rs` tests:

```rust
    #[cfg(unix)]
    #[test]
    fn symlinked_dir_is_listed_as_symlink_not_followed() {
        let (dir, ctx) = setup();
        std::os::unix::fs::symlink(dir.path().join("src"), dir.path().join("alias")).unwrap();
        let out = ListFiles.run(&serde_json::json!({}), &ctx).unwrap();
        // ignore 워커는 심링크를 따라가지 않는다: 항목으로는 보이되 `/` 접미 없음,
        // 내부 파일은 나열되지 않음 (file_type이 symlink라 is_dir false)
        assert!(out.lines().any(|l| l == "alias"), "{out}");
        assert!(!out.contains("alias/main.rs"), "{out}");
    }
```

`src/tools/grep.rs` tests (현재 동작 문서화 — 인접 매치는 각자 컨텍스트 창을 갖고 `--` 구분):

```rust
    #[test]
    fn adjacent_matches_each_get_their_context_window() {
        let dir = tempfile::tempdir().unwrap();
        let content: String = (1..=8).map(|i| format!("line{i}\n")).collect();
        std::fs::write(dir.path().join("f.txt"), content.replace("line3", "hit3").replace("line4", "hit4")).unwrap();
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        let out = Grep.run(&serde_json::json!({"pattern": "hit"}), &ctx).unwrap();
        assert!(out.contains("f.txt:3:") && out.contains("f.txt:4:"), "{out}");
        assert!(out.contains("--"), "매치 블록 구분자: {out}");
    }
```

`src/agent/mod.rs` tests — "(no output)" 분기:

```rust
    struct EmptyTool;
    impl crate::tools::Tool for EmptyTool {
        fn name(&self) -> &'static str { "empty_tool" }
        fn doc(&self) -> &'static str { "empty_tool(): returns nothing." }
        fn run(&self, _a: &serde_json::Value, _c: &ToolCtx) -> Result<String, crate::tools::ToolError> {
            Ok(String::new())
        }
    }

    #[tokio::test]
    async fn empty_tool_output_becomes_no_output_marker() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![ok(&turn("empty_tool", serde_json::json!({}))), ok(&finish("ok"))]);
        let config = Config::default();
        let mut agent = Agent::new(
            &script, Registry::new(vec![Box::new(EmptyTool)]),
            ToolCtx::new(dir.path().to_path_buf()), "test-model".into(), &config,
        );
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(session.messages().iter().any(|m| m.content.contains("(no output)")));
    }
```

- [ ] **Step 5: 전체 확인 + 커밋**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS

```bash
git add -A && git commit -m "chore: M2 이연 정리 — 스피너 게이트, 렌더 공용화, 테스트 공백"
```

---

### Task 13: 문서 갱신 + 최종 검증

**Files:**
- Modify: `CLAUDE.md`, `README.md`(있으면 사용법 절)

- [ ] **Step 1: CLAUDE.md 갱신** (영문 유지 — 사용자 선호)

- 헤더 상태줄: `M1-M3 done (guided agent complete — v1 goal); M4 (eval harness) is next`
- Commands: `cargo run -- --auto`(자동 승인), 게이트 y/N 설명 한 줄, 세션 기록 위치 `./.loco/sessions/*.jsonl`
- Architecture 추가 요점 (간결히):
  - tools: 6-tool guided registry; `write_file`/`edit_file`(3-stage match ladder, CRLF-preserving)/`run_command`(process-group tree kill via `kill`/`taskkill`, CP949 lossy decode, middle-truncation); `confine_for_write` for not-yet-existing paths
  - agent: confirmation gate in the loop via `Approver` trait (`Tty`/`Auto`/`NonInteractive`); repetition detection (3→correction, 5→`RepetitionStop`, exit 2); dispatch via `spawn_blocking` + cancel flag
  - session: `Session` owns history + jsonl transcript; §6 budget packing mutates stored history; context-overflow 400 → pack + retry ×2
  - `--auto` deny patterns: default cross-platform list in config, blocked only in auto mode (interactive shows a warning line)
- Notes: TtyApprover는 의도적 동기 블로킹(고아 stdin 리더 방지) 명시

- [ ] **Step 2: README 사용법 갱신** (파일이 있으면) — /clear 우회책 문구를 자동 절삭 설명으로 교체, --auto·게이트·세션 기록 사용법 추가 (한국어)

- [ ] **Step 3: 최종 게이트**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

Expected: 전체 PASS, clippy 클린, 릴리스 빌드 성공

- [ ] **Step 4: 서버-다운 스모크** (CLAUDE.md 표준 절차)

```bash
cd "$(mktemp -d)" && mkdir .loco && printf 'base_url = "http://127.0.0.1:1/v1"\n' > .loco/config.toml
/Users/sgj/develop/loco/target/release/loco -p "hi"; echo "exit=$?"
# 기대: 한국어 연결 실패 안내 + exit=1
```

- [ ] **Step 5: 라이브 스모크** (LM Studio 필요 — 사용자와 함께)

1. `cargo run` → "src에 hello.txt 만들어 인사말 넣어줘" → diff 미리보기 + y 승인 → 파일 생성 확인
2. 같은 요청에서 `n` 거부 → 모델이 거부를 인지하고 대안/finish 하는지
3. "cargo --version 실행해봐" → run_command 미리보기 + 승인 → exit code 0 출력
4. `.loco/sessions/`에 jsonl 생성 확인, `/clear` 후 새 파일 확인
5. `cargo run -- -p "README 요약해줘" --auto` → 종료 코드 0, stdout에 요약만

- [ ] **Step 6: 커밋**

```bash
git add -A && git commit -m "docs: M3 사용법/아키텍처 문서 갱신"
```

- [ ] **Step 7: 완료 처리** — superpowers:finishing-a-development-branch 스킬로 main 머지 여부 결정
