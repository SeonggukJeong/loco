# M14 정직한 검증 신호 II 구현 플랜

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 하네스가 "검증했다"고 말하는 신호를 실제로 검증된 것에만 붙이고, M13 파일럿이 확정한 결함 2건과 도구·계측 부채를 정리한다.

**Architecture:** 파이프가 섞인 `run_command`는 **VERIFY_NUDGE 해제 술어에서만** 배제한다(루프 감지는 유지). 그 재분류를 전제하는 소비자 5종 — `VERIFY_NUDGE` 문구·`FINISH_NUDGE` 문구·상태선 규칙 4·`exp_metrics` 매처·`REPEAT_CORRECTION`(관측만) — 을 **함께** 갱신한다. `pack()`의 사용자 과제 삭제는 절단 blob을 히스토리에서 빼고 트랜스크립트에만 남겨 막고, 편집 diff를 모델에게 되돌려 조용한 삭제를 가시화한다.

**Tech Stack:** Rust edition 2024, `serde`/`thiserror`/`anyhow`/`similar`/`tempfile`. 스크립트는 POSIX `sh`와 stdlib-only Python 3.

**기준 커밋:** `fd4652e` (스펙 개정 5, 5R `Ready: Yes`)
**스펙:** `docs/superpowers/specs/2026-07-20-m14-honest-verification-ii-design.md` — **유일한 진실. 이 플랜과 충돌하면 스펙이 이긴다. 충돌을 발견하면 스스로 결정하지 말고 에스컬레이션할 것.**

## Global Constraints

이 절의 요구사항은 **모든 태스크에 암묵적으로 포함된다.**

- **Edition 2024. 의존성 추가 금지** — 스펙이 목록을 고정한다. 새 크레이트가 필요하면 착수 전 사용자에게 물을 것
- **모델 대면 텍스트(SYSTEM_PROMPT·교정 문구·상태선·노트)는 영문. 사용자 대면 CLI 메시지는 한국어.** 식별자는 영문
- **에러**: `llm` 모듈은 `thiserror`, 앱 레벨은 `anyhow`
- **커밋**: Conventional Commits (subject는 한국어 가능)
- **브랜치**: Task 1 시작 시 `main`(`fd4652e`)에서 `m14/honest-verification-ii` 생성. **main 병합은 Task 13 판정 후에만**
- **상태선 마커 계약** — `"[status] "` 접두 + 9칸 연속 들여쓰기. `status_note.rs`·`session.rs`·`scripts/exp_metrics.py` **3파일이 문자 그대로 공유**한다. 한 곳을 고치면 세 곳을 고칠 것
- **`exp_metrics.py`는 Rust 상수·술어를 손으로 복사한다**(`MAX_SR_CORRECTIONS`·`BADARGS_KEY_PREFIX`·`normalize`·상태선 매처). 자동 검출이 없으므로 **Rust 쪽을 고치면 수동 미러가 필수**
- **매 태스크 종료 게이트**: `cargo test` 전건 통과 + `cargo clippy --all-targets -- -D warnings` 무경고. `--all-targets`가 중요하다(테스트 코드도 린트)
- **`tasks/`·`tasks-large/` 변경 시** `cargo run -- eval tasks --verify`(12/12)와 `eval tasks-large --verify`(3/3)를 돌릴 것. **이 마일스톤은 픽스처를 건드리지 않으므로 변경이 있으면 그 자체가 이상 신호다**
- **측정 중 병행 빌드 금지** — Task 13에서 CPU 경합이 타이밍 판정을 왜곡한다
- **`docs/` 문서는 한국어, `CLAUDE.md`는 영문**

---

## 파일 구조

| 파일 | 책임 | 태스크 |
|---|---|---|
| `src/agent/mod.rs` | 턴 루프 배선. **T2·T3·T4·T5가 순차 수정** — 병렬 위임 금지 | T1~T5, T7 |
| `src/session.rs` | 히스토리 + 트랜스크립트. 회복 문구 중복 제거 추가 | T2 |
| `src/llm/types.rs` | `reasoning_content`·`usage` 파싱 | T3 |
| `src/tools/run_command.rs` | `has_unquoted_pipe` 가시성 승격 | T4 |
| `src/agent/finish_nudge.rs` | `FINISH_NUDGE` 파이프 변형 문구 | T4 |
| `src/agent/status_note.rs` | 검증 줄 규칙 4 폴백 + 한정자 | T5 |
| `src/tools/diff.rs` | 모델 채널용 diff 렌더러 신설 | T6 |
| `src/tools/edit_file.rs` · `write_file.rs` | 성공 결과에 diff 첨부 | T6 |
| `src/eval/report.rs` | `schema_fallback_count` 집계 | T7 |
| `scripts/pilot.sh` | 하드닝 4건 | T8 |
| `scripts/exp_metrics.py` | 신규 마커 + 출력 형식 + `verify_*` 미러 | T10 |
| `docs/`·`CLAUDE.md`·`README.md` | 봉투 명시·비교가능성 각주 | T11 |

**`src/agent/mod.rs`에 T2·T3·T4·T5가 순차로 들어간다.** T2→T3는 **같은 작업자가 연속 수행**할 것(같은 한 줄을 건드린다 — 스펙 §4-2-2).

---

### Task 1: B-2(c) — `input_budget` 붕괴 방지

**Files:**
- Modify: `src/agent/mod.rs:155-156` (`input_budget`), `Agent::new` 부근
- Test: `src/agent/mod.rs` 내 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: 없음 (첫 태스크)
- Produces: `Agent::input_budget(&self) -> usize` — 하한이 보장된다. 이후 태스크는 이 성질에 의존하지 않지만 T2의 테스트가 예산을 조작한다

**배경**: `input_budget()`은 `(context_tokens - max_output_tokens) * 9 / 10`이다. `max_output_tokens >= context_tokens`이면 `saturating_sub`가 0을 주고 예산이 0이 되어 `pack()`이 히스토리를 최대한 지운다. 스펙 §4-2가 "가장 싸고 (a)·(b)가 못 덮는 경우까지 덮는다"고 한 항목이다. **스펙 §11 Q4가 "하한인가 경고인가"를 열어 뒀고, 이 플랜은 둘 다 채택한다** — 하한은 병리적 config를 막고, 경고는 파일럿 형태(4096/8192, 예산 3686 — 하한에 안 걸린다)를 사용자에게 알린다.

- [ ] **Step 1: 실패하는 테스트를 쓴다**

`src/agent/mod.rs`의 `mod tests` 안에 추가:

```rust
#[test]
fn input_budget_has_a_floor_so_pathological_config_cannot_erase_history() {
    // max_output >= context면 saturating_sub가 0을 주고 예산이 0이 된다 —
    // pack()이 시스템 프롬프트와 마지막 메시지만 남기고 전부 지운다
    let mut cfg = crate::config::Config::default();
    cfg.context_tokens = 4096;
    cfg.max_output_tokens = 8192;
    let agent = Agent::new(Box::new(Scripted::new(vec![])), cfg, Registry::guided());
    assert!(
        agent.input_budget() >= MIN_INPUT_BUDGET,
        "예산이 {}로 붕괴했다 — 하한 {MIN_INPUT_BUDGET}이 없다",
        agent.input_budget()
    );
}

#[test]
fn cramped_output_budget_is_reported_to_the_user() {
    // 파일럿 형태: 4096/8192 → 예산 3686. 하한에는 안 걸리지만 좁다
    assert!(cramped_budget_warning(8192, 4096).is_some(), "파일럿 형태는 경고 대상");
    assert!(cramped_budget_warning(8192, 2048).is_none(), "기본값은 경고 없음");
}
```

`Scripted`는 기존 테스트 헬퍼다. `Agent::new`의 실제 시그니처가 다르면 같은 파일의 기존 테스트에서 호출 형태를 복사할 것.

- [ ] **Step 2: 실패를 확인한다**

Run: `cargo test --lib input_budget_has_a_floor cramped_output_budget -- --nocapture`
Expected: FAIL — `cannot find value MIN_INPUT_BUDGET` / `cannot find function cramped_budget_warning`

- [ ] **Step 3: 최소 구현**

`src/agent/mod.rs`의 상수 근처에 추가:

```rust
/// §4-2(c) 하한 — max_output_tokens가 context_tokens를 넘겨도 예산이 0이 되지
/// 않게 한다. 0이면 pack()이 시스템 프롬프트와 마지막 메시지만 남긴다
const MIN_INPUT_BUDGET: usize = 512;

/// 출력 예산이 컨텍스트의 절반 이상을 먹으면 입력 예산이 좁아진다는 경고.
/// 사용자 대면이므로 한국어. None이면 경고 없음
fn cramped_budget_warning(context_tokens: usize, max_output_tokens: usize) -> Option<String> {
    if max_output_tokens * 2 < context_tokens {
        return None;
    }
    let budget = context_tokens.saturating_sub(max_output_tokens) * 9 / 10;
    Some(format!(
        "경고: max_output_tokens={max_output_tokens}가 context_tokens={context_tokens}의 절반 이상입니다 \
         — 입력 예산이 {budget} 토큰으로 좁아져 오래된 대화가 일찍 잘립니다. \
         context_tokens를 올리거나 max_output_tokens를 낮추는 것을 검토하세요."
    ))
}
```

`input_budget`을 고친다:

```rust
    fn input_budget(&self) -> usize {
        (self.context_tokens.saturating_sub(self.max_output_tokens) * 9 / 10).max(MIN_INPUT_BUDGET)
    }
```

- [ ] **Step 4: 통과를 확인한다**

Run: `cargo test --lib input_budget_has_a_floor cramped_output_budget`
Expected: PASS 2개

- [ ] **Step 5: 경고를 실제로 배선한다**

`Agent::new`(또는 `run()` 진입부 — `on_event`를 쓸 수 있는 곳) 에서 1회 방출:

```rust
        if let Some(w) = cramped_budget_warning(self.context_tokens, self.max_output_tokens) {
            on_event(AgentEvent::Notice(w));
        }
```

**배선 위치 주의**: `run()` 안에 두면 REPL의 매 요청마다 반복된다. `Agent::new`에서 `eprintln!`으로 1회 내는 편이 단순하다 — 기존 코드가 `Agent::new`에서 이벤트를 못 내면 `eprintln!`을 쓰고 그 사실을 커밋 메시지에 적을 것.

- [ ] **Step 6: 전체 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과, 무경고

- [ ] **Step 7: 커밋**

```bash
git add src/agent/mod.rs
git commit -m "feat(agent): input_budget 하한과 좁은 예산 경고 (M14 B-2c)"
```

---

### Task 2: B-2(b) — 절단 blob을 히스토리에서 빼고 회복 문구 중복을 막는다

> **T2와 T3는 같은 작업자가 연속 수행할 것.** 둘이 `agent/mod.rs`의 length 분기 한 블록을 서로 반대 방향으로 건드린다(스펙 §4-2-2). 분리 위임하면 T3가 T2의 산출물을 지운다.

**Files:**
- Modify: `src/session.rs` (`push_recovery_notice` 신설), `src/agent/mod.rs:239-256` (length 분기)
- Test: `src/session.rs` tests, `src/agent/mod.rs` tests

**Interfaces:**
- Consumes: T1의 `input_budget` 하한 (테스트가 좁은 예산을 만든다)
- Produces:
  - `Session::push_recovery_notice(&mut self, notice: &str)` — 꼬리 user 메시지가 이미 `notice`로 끝나면 **아무것도 하지 않는다**(트랜스크립트 포함). 아니면 `push_user_request`와 같은 병합 규칙
  - `const LENGTH_RECOVERY: &str` (`agent/mod.rs`) — 회복 문구 상수. T3가 같은 분기를 수정할 때 이 이름을 쓴다

**배경**: 세션 1(Z1)은 예산 3686에 3751로 65 토큰 초과해 쌍 삭제가 발동했고 **user 과제 + assistant blob이 함께** 사라졌다. 절단 blob을 히스토리에 안 넣으면 초과 자체가 안 생긴다. 부작용 3건을 **전부** 처리해야 한다(스펙 §4-2).

- [ ] **Step 1: `push_recovery_notice`의 실패하는 테스트를 쓴다**

`src/session.rs`의 `mod tests`에 추가:

```rust
#[test]
fn recovery_notice_is_not_duplicated_when_appended_back_to_back() {
    let mut s = sess(vec![ChatMessage::system("sys"), ChatMessage::user("TASK: fix it")]);
    s.push_recovery_notice("CUT OFF");
    s.push_recovery_notice("CUT OFF");
    s.push_recovery_notice("CUT OFF");
    let joined = s.messages().iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n");
    assert_eq!(joined.matches("CUT OFF").count(), 1, "연속 주입은 1벌만: {joined}");
}

#[test]
fn recovery_notice_merges_into_a_trailing_user_message() {
    let mut s = sess(vec![ChatMessage::system("sys"), ChatMessage::user("TASK")]);
    s.push_recovery_notice("CUT OFF");
    assert_eq!(s.messages().len(), 2, "새 메시지가 아니라 병합이어야 role 교대가 유지된다");
    assert!(s.messages()[1].content.ends_with("CUT OFF"));
}

#[test]
fn recovery_notice_pushes_when_the_tail_is_an_assistant_message() {
    let mut s = sess(vec![ChatMessage::system("sys"), ChatMessage::assistant("a")]);
    s.push_recovery_notice("CUT OFF");
    assert_eq!(s.messages().len(), 3);
    assert_eq!(s.messages()[2].role, "user");
}
```

`sess`는 같은 파일의 기존 테스트 헬퍼(`:337`)다.

- [ ] **Step 2: 실패를 확인한다**

Run: `cargo test --lib recovery_notice`
Expected: FAIL — `no method named push_recovery_notice`

- [ ] **Step 3: `Session::push_recovery_notice` 구현**

`src/session.rs`의 `push_user_request` 바로 아래에 추가:

```rust
    /// §4-2-1 회복 문구 — 꼬리가 이미 같은 문구로 끝나면 **아무것도 하지 않는다**.
    /// 이 문구는 `</tool_result>` 뒤 접미에 병합되는데 `pack()`의 축약이 그 접미를
    /// 의도적으로 보존하므로(:133~136), 연속 주입분은 회수 경로가 없다.
    /// 교대 형태(사이에 다른 턴이 끼는 경우)의 사본은 서로 다른 메시지에 실려
    /// 쌍 삭제가 걷어내므로 여기서 막지 않는다
    pub fn push_recovery_notice(&mut self, notice: &str) {
        if self.messages.last().is_some_and(|m| m.role == "user" && m.content.ends_with(notice)) {
            return;
        }
        self.push_user_request(notice);
    }
```

- [ ] **Step 4: 통과를 확인한다**

Run: `cargo test --lib recovery_notice`
Expected: PASS 3개

- [ ] **Step 5: length 분기의 실패하는 통합 테스트를 쓴다**

`src/agent/mod.rs`의 `mod tests`에 추가. **트랜스크립트는 실 파일이어야 한다** — `Transcript::disabled()`로는 단언 ②가 공허하게 통과한다(스펙 §7 기준 3 유실 조건):

```rust
#[tokio::test]
async fn length_turns_keep_the_task_message_and_leave_the_blob_only_in_the_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let tpath = dir.path().join("t.jsonl");
    let transcript = Transcript::create_at(&tpath).unwrap();

    // length 5연속 → finish. 절단 blob은 매번 긴 텍스트
    let mut script = vec![ok_with_reason("X".repeat(400).as_str(), "length"); 5];
    script.push(ok(&finish("done")));

    let mut cfg = Config::default();
    cfg.context_tokens = 4096;
    cfg.max_output_tokens = 2048; // 예산 1843 — 쌍 삭제가 발동할 만큼 좁다
    let agent = Agent::new(Box::new(Scripted::new(script)), cfg, Registry::guided());

    let mut session = Session::new(vec![ChatMessage::system("sys")], transcript);
    session.push(ChatMessage::user("TASK: 한국어 과제 원문"));
    let out = agent.run_with_session(&mut session, &mut |_| {}).await.unwrap();

    // ① 과제 생존
    let hist = session.messages().iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n");
    assert!(hist.contains("TASK: 한국어 과제 원문"), "과제가 삭제됐다:\n{hist}");

    // ④ 연속 length가 5회여도 회복 문구는 1벌
    assert_eq!(hist.matches(LENGTH_RECOVERY).count(), 1, "회복 문구 중복:\n{hist}");

    // ③ role 교대 무손상
    for w in session.messages().windows(2) {
        assert_ne!(w[0].role, w[1].role, "role 연속: {:?}", session.messages());
    }

    // ② 절단 blob은 트랜스크립트에 남는다
    let jsonl = std::fs::read_to_string(&tpath).unwrap();
    assert!(jsonl.contains(&"X".repeat(400)), "절단 blob이 트랜스크립트에서 사라졌다");

    assert!(matches!(out, AgentOutcome::Finished(_)));
}
```

`ok_with_reason`은 기존 헬퍼(`:2001`에서 사용)다. `run_with_session`이 없으면 기존 테스트가 쓰는 진입점을 그대로 쓰고, 세션을 밖에서 만들 수 없으면 그 테스트 형태를 따를 것 — **단언 5개는 유지**한다.

- [ ] **Step 6: 실패를 확인한다**

Run: `cargo test --lib length_turns_keep_the_task_message`
Expected: FAIL — 과제 삭제 또는 `LENGTH_RECOVERY` 미정의

- [ ] **Step 7: length 분기를 고친다**

`src/agent/mod.rs`의 상수 근처:

```rust
/// length 턴 회복 문구 (M9~M13에서 문자열 동일 — 상수로 승격해 중복 제거가 참조한다)
pub(crate) const LENGTH_RECOVERY: &str = "Your previous response was cut off by the output token limit. \
                                          Respond again with exactly one, much shorter JSON turn.";
```

`:239-245`를 교체:

```rust
            if resp.finish_reason() == Some("length") {
                // §4-2(b): 절단 blob을 **히스토리에 push하지 않는다**. 예산 초과를
                // 만드는 것이 이 blob이고, 초과가 쌍 삭제를 부르면 사용자 과제가
                // 함께 사라진다(M13 세션 Z1). 트랜스크립트에는 남긴다 —
                // M13의 디코딩 퇴화 분석이 전부 이 blob을 읽어 만들어졌다
                let t = resp.text();
                session.record_extra("assistant", if t.is_empty() { "(empty)" } else { t });
                // push가 아니라 push_recovery_notice: ① assistant를 건너뛰었으므로
                // 꼬리가 user다 — 병합해야 role 교대가 유지되는데 교정 담당
                // merge_adjacent_same_role는 pack()의 쌍 삭제 루프 안에서만 돌고
                // (b)의 목적이 바로 pack 미발동이라 영영 교정되지 않는다
                // ② 연속 주입 중복은 pack()이 회수할 수 없는 자리에 쌓인다
                session.push_recovery_notice(LENGTH_RECOVERY);
```

나머지(`on_event`·`status.on_turn`·`turns += 1`·`continue`)는 **그대로 둔다.**

- [ ] **Step 8: 통과를 확인한다**

Run: `cargo test --lib length_turns_keep_the_task_message`
Expected: PASS

- [ ] **Step 9: 전체 게이트 — 기존 테스트 회귀를 확인한다**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과. **`agent/mod.rs:1102`의 기존 테스트가 `messages()`에서 "cut off"를 찾는데, 병합 경로에서도 히스토리에 남으므로 통과해야 한다. 깨지면 그 테스트가 무엇을 고정하는지 읽고 판단할 것 — 문구가 히스토리에 남는다는 성질은 유지되어야 한다**

- [ ] **Step 10: 커밋**

```bash
git add src/session.rs src/agent/mod.rs
git commit -m "fix(session): length 절단 blob을 히스토리에서 제외, 회복 문구 중복 제거 (M14 B-2b)"
```

---

### Task 3: B-1 — `reasoning_content`·`usage` 파싱

> **Task 2와 같은 작업자가 연속 수행.**

**Files:**
- Modify: `src/llm/types.rs:38-70`, `src/agent/mod.rs` (T2가 고친 length 분기의 `record_extra` 한 줄)
- Test: `src/llm/types.rs` tests

**Interfaces:**
- Consumes: T2의 `session.record_extra("assistant", ...)` 호출 지점
- Produces: `ChatResponse::reasoning(&self) -> &str`, `ChatResponse::completion_tokens(&self) -> Option<u32>`

**배경**: `ResponseMessage`가 `role`·`content`뿐이라 llama.cpp가 사고 토큰을 `reasoning_content`로 분리해 흘리면 serde가 버린다. 파일럿의 "빈 응답" 4건이 전부 이것이다. **교정이 아니라 진단이다** — 어느 턴도 되살리지 못하고, 산출물은 `"(empty)"` 대신 추론 꼬리를 트랜스크립트에 남기는 것과 소비 토큰 노출뿐이다(스펙 §4-1).

**수선 대상은 `Delta`가 아니라 `ResponseMessage`다.** 에이전트 루프는 `stream: false`다.

- [ ] **Step 1: 역직렬화 실패 테스트를 쓴다**

**이 층이 필수다** — `Scripted`는 `ChatResponse`를 직접 구성해 **serde 파싱을 우회**하므로 통합 테스트만으로는 파싱이 검증되지 않는다(스펙 §7 기준 2).

`src/llm/types.rs`의 `mod tests`에 추가 (없으면 신설):

```rust
#[test]
fn reasoning_content_is_parsed_from_a_non_streaming_response() {
    // llama.cpp 실측 페이로드 형태 — content는 빈 채 reasoning_content만 온다
    let raw = r#"{
      "choices": [{
        "message": {"role":"assistant","content":"","reasoning_content":"Let me think about the file layout"},
        "finish_reason": "length"
      }],
      "usage": {"completion_tokens": 40, "prompt_tokens": 23, "total_tokens": 63}
    }"#;
    let r: ChatResponse = serde_json::from_str(raw).unwrap();
    assert_eq!(r.text(), "");
    assert_eq!(r.reasoning(), "Let me think about the file layout");
    assert_eq!(r.completion_tokens(), Some(40));
    assert_eq!(r.finish_reason(), Some("length"));
}

#[test]
fn a_response_without_the_new_fields_still_parses() {
    let raw = r#"{"choices":[{"message":{"role":"assistant","content":"hi"}}]}"#;
    let r: ChatResponse = serde_json::from_str(raw).unwrap();
    assert_eq!(r.text(), "hi");
    assert_eq!(r.reasoning(), "");
    assert_eq!(r.completion_tokens(), None);
}
```

- [ ] **Step 2: 실패를 확인한다**

Run: `cargo test --lib reasoning_content_is_parsed a_response_without_the_new_fields`
Expected: FAIL — `no method named reasoning`

- [ ] **Step 3: 타입을 넓힌다**

`src/llm/types.rs`:

```rust
/// 응답의 message는 content가 null일 수 있어 요청용 ChatMessage와 분리
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    /// llama.cpp가 사고 토큰을 분리해 흘리는 필드 (M14 B-1). 이것을 안 읽으면
    /// content가 빈 턴에서 모델 출력을 통째로 버린다 — 파일럿 "빈 응답" 4건
    #[serde(default)]
    pub reasoning_content: Option<String>,
}

/// 토큰 소비량. 서버가 reasoning_tokens를 분해해 주지 않으므로
/// completion_tokens는 추론분을 **포함한** 합산값이다 (M14 B-1, 라이브 실측)
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub completion_tokens: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}
```

`impl ChatResponse`에 추가:

```rust
    /// 첫 번째 choice의 추론 꼬리. 없으면 빈 문자열
    pub fn reasoning(&self) -> &str {
        self.choices
            .first()
            .and_then(|c| c.message.reasoning_content.as_deref())
            .unwrap_or("")
    }

    /// 출력 토큰 소비량. content가 빈 턴에서는 곧 추론 소비량이고,
    /// 그 외에는 합산값이라 분리되지 않는다
    pub fn completion_tokens(&self) -> Option<u32> {
        self.usage.as_ref().and_then(|u| u.completion_tokens)
    }
```

`Usage`가 `dead_code`로 clippy를 깨면 `completion_tokens()`가 읽으므로 문제없다. 그래도 경고가 나면 필드에 `pub`이 붙어 있는지 확인할 것.

- [ ] **Step 4: 통과를 확인한다**

Run: `cargo test --lib reasoning_content_is_parsed a_response_without_the_new_fields`
Expected: PASS 2개

- [ ] **Step 5: length 분기가 추론 꼬리를 남기게 한다**

T2가 만든 `record_extra` 한 줄을 교체:

```rust
                // §4-2-2: 최종 상태는 "히스토리에 push 안 함 + 트랜스크립트에만".
                // B-1은 그 트랜스크립트 레코드의 **내용**을 "(empty)"에서 추론 꼬리로
                // 바꾼다 — content가 비어도 모델은 예산을 추론에 다 쓴 것이지
                // 아무것도 안 낸 것이 아니다
                let t = resp.text();
                let blob = if !t.is_empty() {
                    t
                } else {
                    let r = resp.reasoning();
                    if r.is_empty() { "(empty)" } else { r }
                };
                session.record_extra("assistant", blob);
```

- [ ] **Step 6: 통합 단언을 추가한다**

Task 2의 `length_turns_keep_the_task_message...` 테스트 아래에 추가:

```rust
#[tokio::test]
async fn a_length_turn_with_only_reasoning_records_the_reasoning_tail_not_empty() {
    let dir = tempfile::tempdir().unwrap();
    let tpath = dir.path().join("t.jsonl");
    let transcript = Transcript::create_at(&tpath).unwrap();

    // content 공백 + reasoning_content 있음 + finish_reason length — 파일럿 형태
    let script = vec![
        ok_with_reasoning("", "I was thinking about src/lib.rs", "length"),
        ok(&finish("done")),
    ];
    let agent = Agent::new(Box::new(Scripted::new(script)), Config::default(), Registry::guided());
    let mut session = Session::new(vec![ChatMessage::system("sys")], transcript);
    session.push(ChatMessage::user("TASK"));
    let _ = agent.run_with_session(&mut session, &mut |_| {}).await.unwrap();

    let jsonl = std::fs::read_to_string(&tpath).unwrap();
    assert!(jsonl.contains("I was thinking about src/lib.rs"), "추론 꼬리가 안 남았다");
    assert!(!jsonl.contains("\"(empty)\""), "(empty) 리터럴이 남았다:\n{jsonl}");
}
```

`ok_with_reasoning(content, reasoning, finish_reason)` 헬퍼를 기존 `ok_with_reason` 옆에 신설한다:

```rust
    fn ok_with_reasoning(content: &str, reasoning: &str, reason: &str) -> Result<ChatResponse, LlmError> {
        Ok(ChatResponse {
            choices: vec![Choice {
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: Some(content.to_string()),
                    reasoning_content: Some(reasoning.to_string()),
                },
                finish_reason: Some(reason.to_string()),
            }],
            usage: None,
        })
    }
```

**기존 `ok`/`ok_with_reason` 헬퍼가 `ResponseMessage`·`ChatResponse`를 리터럴로 구성한다면 새 필드 때문에 컴파일이 깨진다. 전부 고칠 것** — `reasoning_content: None`, `usage: None`을 더하면 된다.

- [ ] **Step 7: 전체 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과, 무경고

- [ ] **Step 8: 커밋**

```bash
git add src/llm/types.rs src/agent/mod.rs
git commit -m "feat(llm): ResponseMessage에 reasoning_content·usage 파싱 (M14 B-1)"
```

---

### Task 4: A-1 — 파이프 가드를 해제 술어에만 + 소비자 4종 갱신

> **이 태스크의 브리프에 반드시 동봉할 5항목**(스펙 §10). 하나라도 빠지면 하네스가 자기모순 상태로 커밋된다.
> 1. 구현 제약 — 별도 `is_piped`, `cmd_exit`/`cmd_summary`를 `None`으로 만들지 말 것
> 2. 파이프 전용 `VERIFY_NUDGE` 문구
> 3. 플래그 정의 — **해제를 건너뛸 때마다 덮어쓰기**(조건부 건너뛰기 금지)
> 4. `FINISH_NUDGE` 파이프 변형 문구
> 5. §3-4-2의 규칙별 한정자 분리 — **참조로만**(구현은 T5)

**Files:**
- Modify: `src/tools/run_command.rs:16` (가시성), `src/agent/finish_nudge.rs` (변형 문구), `src/agent/mod.rs` (`VERIFY_NUDGE` 문구·`is_piped`·플래그·해제 술어)
- Test: `src/agent/mod.rs` tests

**Interfaces:**
- Consumes: T3까지의 `agent/mod.rs` 상태
- Produces:
  - `pub(crate) fn has_unquoted_pipe(cmd: &str) -> bool` (`tools::run_command`)
  - `pub const VERIFY_NUDGE_PIPE: &str` (`agent/mod.rs`)
  - `finish_nudge::FINISH_NUDGE_PIPE: &'static str` + `FinishNudge::on_turn`이 반환하는 문구를 호출자가 선택할 수 있는 형태 — **아래 Step 5의 설계를 그대로 쓸 것**
  - 런 스코프 지역변수 `is_piped`(턴별), `unreleased_due_to_pipe`(런 스코프)

**배경**: M11이 이미 파이프 명령 결과에 "이 exit code는 마지막 명령의 것"이라는 노트를 붙이는데, 정작 하네스는 같은 턴에 그 exit 0을 검증으로 세고 있었다. **가드는 해제 술어에만 건다 — `VerifyOk` 매핑을 건드리면 재검증 루프 감지가 죽는다**(스펙 §3-3-3, 실측으로 기각된 대안).

- [ ] **Step 1: 실패하는 테스트 3개를 쓴다**

```rust
#[tokio::test]
async fn a_piped_verification_does_not_release_verify_nudge() {
    let script = vec![
        ok(&call("write_file", json!({"path":"a.rs","content":"fn a(){}"}))),
        ok(&call("run_command", json!({"command":"cargo test 2>&1 | tail -5"}))),
        ok(&finish("done")),   // 파이프 문구로 1회 반려
        ok(&finish("done2")),
    ];
    let (out, notes) = run_capturing_tool_results(script).await;
    assert!(matches!(out, AgentOutcome::Finished(ref s) if s == "done2"));
    assert!(notes.iter().any(|n| n.contains(VERIFY_NUDGE_PIPE)), "파이프 문구가 안 나왔다: {notes:?}");
    assert!(!notes.iter().any(|n| n.contains(VERIFY_NUDGE)), "기본 문구가 나왔다 — 거짓말이다");
}

#[tokio::test]
async fn a_pipe_still_counts_for_loop_detection() {
    // §3-3-3 가드: VerifyOk 매핑에 가드를 걸면 이 테스트가 실패한다.
    // 교대가 필수 — 동일 명령 반복은 5번째에 RepetitionStop이 !stop 가드로
    // FINISH_NUDGE 평가를 선점한다. 5회는 하한이다(#1 무장 + #2~#5가 K=4를 채움)
    let mut script = vec![ok(&call("write_file", json!({"path":"a.rs","content":"x"})))];
    for i in 0..5 {
        let cmd = if i % 2 == 0 { "cargo test 2>&1 | tail -5" } else { "cargo test 2>&1 | head -5" };
        script.push(ok(&call("run_command", json!({"command": cmd}))));
    }
    script.push(ok(&finish("done")));
    script.push(ok(&finish("done2")));
    let (_out, notes) = run_capturing_tool_results(script).await;
    assert!(
        notes.iter().any(|n| n.contains(finish_nudge::FINISH_NUDGE_PIPE)),
        "FINISH_NUDGE가 안 나왔다 — VerifyOk 매핑에 가드가 걸렸다: {notes:?}"
    );
}

#[tokio::test]
async fn the_pipe_flag_is_cleared_once_a_clean_verification_releases() {
    // 4R C2: 해제 성공 시 플래그를 안 고치면 FINISH_NUDGE가 낡은 true를 읽는다
    let mut script = vec![ok(&call("write_file", json!({"path":"a.rs","content":"x"})))];
    script.push(ok(&call("run_command", json!({"command":"cargo test 2>&1 | tail -5"}))));
    script.push(ok(&call("run_command", json!({"command":"cargo test"}))));   // 해제
    for _ in 0..4 {
        script.push(ok(&call("read_file", json!({"path":"a.rs"}))));
    }
    script.push(ok(&finish("done")));
    let (_out, notes) = run_capturing_tool_results(script).await;
    assert!(
        !notes.iter().any(|n| n.contains(finish_nudge::FINISH_NUDGE_PIPE)),
        "마지막 검증이 파이프가 아닌데 파이프 변형이 나왔다: {notes:?}"
    );
}
```

`run_capturing_tool_results`는 스크립트를 돌리고 `(AgentOutcome, Vec<String>)`(모든 tool_result 본문)을 주는 헬퍼다. 기존 테스트가 세션 메시지를 훑는 방식을 쓰고 있으면 그것을 재사용해 헬퍼로 뽑을 것.

- [ ] **Step 2: 실패를 확인한다**

Run: `cargo test --lib a_piped_verification a_pipe_still_counts the_pipe_flag_is_cleared`
Expected: FAIL — `VERIFY_NUDGE_PIPE` / `FINISH_NUDGE_PIPE` 미정의

- [ ] **Step 3: `has_unquoted_pipe` 가시성 승격**

`src/tools/run_command.rs:16`:

```rust
pub(crate) fn has_unquoted_pipe(cmd: &str) -> bool {
```

같은 파일의 기존 테스트가 `super::has_unquoted_pipe`로 부르고 있으므로 그대로 통과한다. `src/tools/mod.rs`에서 재수출이 필요하면 `pub(crate) use run_command::has_unquoted_pipe;`를 더할 것.

- [ ] **Step 4: 문구 2종을 정의한다**

`src/agent/mod.rs`, `VERIFY_NUDGE` 옆:

```rust
pub const VERIFY_NUDGE: &str = "You modified files but never ran a verification command since your last edit. Run the project's tests (e.g. cargo test) with run_command, then finish.";

/// §3-3-1 — 파이프 때문에 검증이 성립하지 않은 경우. 기본 문구는 "never ran"이라
/// 방금 파이프로 검증을 돌린 모델에게 거짓말이 된다
pub const VERIFY_NUDGE_PIPE: &str = "You ran a verification command, but it was a shell pipeline, so its exit code reflects only the last command in the pipe and does not tell whether the tests passed. Re-run it without a pipe, then finish.";
```

**기본 문구에 `since your last edit`를 추가한 것은 4R M3이다** — 상태선 규칙 1이 같은 상황을 `"verification: none since your last edit"`로 렌더하는데 두 장치가 다르게 말하고 있었다. `mutated_since_verify = true`는 코드 전체에서 뮤테이션 성공 디스패치 한 곳에서만 세워지므로 이 구절은 발동 시 항상 참이다.

`src/agent/finish_nudge.rs`:

```rust
/// 발동 시 1회 주입 (M9 §4-2). 모델 대상 — 영어
pub const FINISH_NUDGE: &str = "You already ran a successful verification. If the task is complete, \
call finish with a summary now; do not re-verify what you have already confirmed.";

/// §3-3-3-1 — 마지막 검증이 파이프여서 "successful"이 참이 아닌 경우.
/// 기본 문구를 그대로 쓰면 파이프 VERIFY_NUDGE와 같은 이벤트를 반대로 부른다
pub const FINISH_NUDGE_PIPE: &str = "You have re-verified several times. Note your last verification \
was a shell pipeline, so it did not establish that the tests passed - run it once without a pipe, then finish.";
```

- [ ] **Step 5: `FinishNudge`가 변형을 고를 수 있게 한다**

`on_turn`의 반환 타입은 그대로 두고 **호출자가 치환**한다 — `finish_nudge.rs`에 플래그를 넣으면 상태가 두 곳에 생긴다.

`src/agent/mod.rs`의 FINISH_NUDGE 주입 지점:

```rust
                    if !stop && let Some(nudge) = finish_nudge.on_turn(ev) {
                        let nudge = if unreleased_due_to_pipe && nudge == finish_nudge::FINISH_NUDGE {
                            finish_nudge::FINISH_NUDGE_PIPE
                        } else {
                            nudge
                        };
                        on_event(AgentEvent::Notice("(검증 완료 후 재확인 반복 — finish 유도 주입)".to_string()));
                        note = merge_note(note, nudge);
                    }
```

**`on_turn`을 호출하는 지점이 여러 곳이면 전부 같은 치환을 적용할 것.** 누락하면 그 경로에서 자기모순이 남는다.

- [ ] **Step 6: 해제 술어에 가드를 걸고 플래그를 무조건 대입한다**

`src/agent/mod.rs`의 `empty_verify` 계산부 근처. **`cmd_exit`/`cmd_summary`는 건드리지 않는다** — T5의 A-2가 그 값을 렌더에 쓴다:

```rust
            let empty_verify = cmd_summary.as_ref().is_some_and(|s| s.ran == 0 && s.filtered_out > 0);
            // §3-3: 파이프가 섞이면 exit code가 파이프라인 마지막 명령의 것이라
            // 검증 성립을 알 수 없다. **해제 술어에만** 건다 — VerifyOk 매핑을
            // 건드리면 재검증 루프 감지가 죽는다(§3-3-3, 실측으로 기각된 대안)
            let is_piped = turn.action.tool == "run_command"
                && turn
                    .action
                    .args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .is_some_and(crate::tools::run_command::has_unquoted_pipe);

            if dispatch_ok {
                if turn.action.tool == "run_command" {
                    // §3-3-1: 플래그는 "현재 미해제 상태를 만든 원인이 파이프인가"다.
                    // 조건부 건너뛰기 금지 — 해제 성공 시에도 갱신해야 한다.
                    // 소비자가 둘이고 결합 조건이 다르기 때문: VERIFY_NUDGE는
                    // mutated_since_verify와 묶여 낡은 값을 못 읽지만
                    // FINISH_NUDGE는 묶여 있지 않아 그대로 읽는다 (4R C2)
                    let released = !empty_verify && !is_piped;
                    if released {
                        mutated_since_verify = false;
                    }
                    unreleased_due_to_pipe = !released && is_piped;
                    status.record_command_result(cmd_exit.clone(), cmd_summary.clone());
                } else if self.registry.get(&turn.action.tool).is_some_and(|t| t.is_mutating()) {
                    mutated_since_verify = true;
                    unreleased_due_to_pipe = false; // 뮤테이션이 원인을 갈아치운다
                }
```

`run()` 진입부에 `verify_nudged` 옆으로 선언:

```rust
        let mut unreleased_due_to_pipe = false;
```

- [ ] **Step 7: VERIFY_NUDGE 주입 지점이 변형을 고르게 한다**

`agent/mod.rs:304-307`:

```rust
                        if mutated_since_verify && !verify_nudged {
                            verify_nudged = true;
                            let nudge = if unreleased_due_to_pipe { VERIFY_NUDGE_PIPE } else { VERIFY_NUDGE };
                            on_event(AgentEvent::Notice("(검증 없는 종료 — 확인 요청 주입)".to_string()));
                            session.push(tool_result_message("finish", nudge));
```

- [ ] **Step 8: 통과를 확인한다**

Run: `cargo test --lib a_piped_verification a_pipe_still_counts the_pipe_flag_is_cleared`
Expected: PASS 3개

- [ ] **Step 9: 전체 게이트 — 기존 테스트 회귀 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과. **`VERIFY_NUDGE` 문자열을 단언하는 기존 테스트가 `since your last edit` 추가로 깨질 수 있다.** 깨지면 단언을 새 문구로 갱신할 것 — 그 테스트가 고정하는 성질(무검증 finish가 1회 반려된다)은 유지된다.

- [ ] **Step 10: 커밋**

```bash
git add src/tools/run_command.rs src/agent/finish_nudge.rs src/agent/mod.rs
git commit -m "feat(agent): 파이프 검증을 VERIFY_NUDGE 해제에서 배제, 소비자 문구 4종 갱신 (M14 A-1)"
```

---

### Task 5: A-2 — 상태선이 파이프와 요약 부재를 정직하게 렌더

**Files:**
- Modify: `src/agent/status_note.rs` (`record_command_result` 시그니처·`verification_line`), `src/agent/mod.rs` (호출 지점 1곳)
- Test: `src/agent/status_note.rs` tests

**Interfaces:**
- Consumes: T4의 `is_piped`
- Produces: `StatusNote::record_command_result(&mut self, exit: Option<String>, summary: Option<TestSummary>, piped: bool)`

**⚠ 스펙과의 의도적 편차 — 태스크 리뷰어에게 알릴 것.** 스펙 §3-1/§3-4는 *"명령 문자열을 받도록 시그니처를 넓히고"*라고 쓰지만, `StatusNote`가 명령으로 하는 일은 파이프 판정 하나뿐이고 §3-3이 이미 디스패치 지점에서 `is_piped`를 계산한다. **파생 `bool`을 넘기면 술어가 두 곳에 복제되지 않는다.** 스펙의 목적(상태선이 파이프 사실을 안다)은 그대로 달성된다. 리뷰어가 부적절하다고 판단하면 명령 문자열로 되돌릴 것.

- [ ] **Step 1: 실패하는 테스트를 쓴다**

```rust
#[test]
fn rule_4_falls_back_to_rule_5_when_the_output_came_through_a_pipe() {
    // #3(tail 절단): 실패 섹션이 잘려 failed=0으로 파싱되고, exit는 tail의 0이라
    // M12의 교차검증이 구조적으로 통과한다 → "all 8 passed"라는 거짓
    let mut n = StatusNote::new();
    n.record_mutation(&json!({"path":"src/lib.rs"}));
    n.record_command_result(
        Some("0".into()),
        Some(TestSummary { ran: 8, passed: 8, failed: 0, filtered_out: 0, failed_names: vec![] }),
        true,
    );
    let note = n.on_turn(&ctx(3, false)).unwrap();
    assert!(!note.contains("all 8 passed"), "파이프인데 통과를 주장한다: {note}");
    assert!(note.contains("last command exited 0"), "규칙 5로 폴백해야 한다: {note}");
    assert!(note.contains("via pipe"), "파이프 한정자가 없다: {note}");
}

#[test]
fn rule_4_still_renders_all_passed_without_a_pipe() {
    let mut n = StatusNote::new();
    n.record_mutation(&json!({"path":"src/lib.rs"}));
    n.record_command_result(
        Some("0".into()),
        Some(TestSummary { ran: 8, passed: 8, failed: 0, filtered_out: 0, failed_names: vec![] }),
        false,
    );
    assert!(n.on_turn(&ctx(3, false)).unwrap().contains("all 8 passed"));
}

#[test]
fn a_missing_test_summary_is_stated_on_the_same_line() {
    let mut n = StatusNote::new();
    n.record_mutation(&json!({"path":"src/lib.rs"}));
    n.record_command_result(Some("101".into()), None, false);
    let note = n.on_turn(&ctx(3, false)).unwrap();
    assert!(note.contains("no test summary"), "{note}");
    // 블록 계약: 마커 줄 + 9칸 들여쓰기만
    for line in note.lines().skip(1) {
        assert!(line.starts_with(CONT_INDENT), "들여쓰기 없는 고아 줄: {line:?}");
    }
}

#[test]
fn the_no_summary_phrase_never_appears_when_a_summary_exists() {
    // 4R I2: 규칙 5는 요약이 있는 채로도 도달한다 — M12 교차검증 실패와 이번 폴백.
    // 문구를 렌더 지점에 붙이면 거짓이 된다
    let mut n = StatusNote::new();
    n.record_mutation(&json!({"path":"src/lib.rs"}));
    n.record_command_result(
        Some("101".into()),   // allpass + exit 101 → 교차검증 실패 → 규칙 5
        Some(TestSummary { ran: 8, passed: 8, failed: 0, filtered_out: 0, failed_names: vec![] }),
        false,
    );
    let note = n.on_turn(&ctx(3, false)).unwrap();
    assert!(!note.contains("no test summary"), "요약이 있는데 없다고 한다: {note}");
}
```

`ctx(turn, mutated_since_verify)`는 `TurnCtx`를 만드는 헬퍼다. 기존 테스트가 쓰는 형태를 재사용하되 **`turn`은 렌더가 실제로 일어나는 값**이어야 한다(뮤테이션이 있으면 `mutation_ok`나 케이던스). 위 테스트는 `record_mutation` 후이므로 뮤테이션 분기를 탄다.

- [ ] **Step 2: 실패를 확인한다**

Run: `cargo test --lib rule_4_falls_back a_missing_test_summary the_no_summary_phrase`
Expected: FAIL — 인자 개수 불일치

- [ ] **Step 3: 시그니처와 상태를 넓힌다**

`src/agent/status_note.rs`:

```rust
pub struct StatusNote {
    mutated_paths: Vec<String>,
    last_cmd_exit: Option<String>,
    last_test_summary: Option<TestSummary>,
    last_cmd_piped: bool,
    pending: bool,
}
```

`new()`에 `last_cmd_piped: false`를 더하고:

```rust
    pub fn record_command_result(&mut self, exit: Option<String>, summary: Option<TestSummary>, piped: bool) {
        self.last_cmd_piped = piped;
        if exit.is_none() {
            self.last_cmd_exit = None;
            self.last_test_summary = None;
            return;
        }
        self.last_cmd_exit = exit;
        self.last_test_summary = summary;
    }
```

- [ ] **Step 4: 규칙 4 폴백과 규칙 5 한정자**

`verification_line`의 규칙 4를 고친다:

```rust
            // 규칙 4: 전부 통과 — exit 0 교차 검증 필수.
            // §3-4-2: 파이프면 그 교차검증이 tail의 exit 0 때문에 구조적으로
            // 무조건 통과하므로 무효다. M12의 "교차검증 실패 시 규칙 5 폴백"을
            // 그대로 확장한다 — 새 정책이 아니다
            if s.failed == 0 && s.ran > 0 && self.last_cmd_exit.as_deref() == Some("0") && !self.last_cmd_piped {
                return format!("verification: last cargo test: all {} passed", s.passed);
            }
```

규칙 5를 고친다 — **한정자는 한 줄 안에서 조립한다**(블록 계약):

```rust
        // 규칙 5: 기존 문안 + 한정자. 여러 줄로 만들면 M11 블록 계약이 깨진다 —
        // CONT_INDENT 없는 줄은 remove_status_note()가 회수하지 못해 영구 잔존한다
        let mut quals: Vec<&str> = Vec::new();
        if self.last_cmd_piped {
            quals.push("via pipe");
        }
        // 조건은 last_test_summary.is_none()이며 "규칙 5에 도달했다"가 아니다 —
        // 규칙 5는 요약이 있는 채로도 도달한다(교차검증 실패, 파이프 폴백)
        if self.last_test_summary.is_none() {
            quals.push("no test summary in output");
        }
        let suffix = if quals.is_empty() { String::new() } else { format!(" ({})", quals.join(", ")) };
        match &self.last_cmd_exit {
            Some(code) => format!("verification: last command exited {code}{suffix}"),
            None => format!("verification: last command gave no exit code{suffix}"),
        }
```

- [ ] **Step 5: 호출 지점을 고친다**

`src/agent/mod.rs`의 T4 Step 6 블록:

```rust
                    status.record_command_result(cmd_exit.clone(), cmd_summary.clone(), is_piped);
```

- [ ] **Step 6: 블록 계약 불변식 테스트를 쓴다** (스펙 §7 기준 1)

```rust
#[test]
fn every_rendered_status_line_keeps_the_block_contract() {
    // §11 Q6: 규칙이 아니라 **입력 차원**을 열거한다. 규칙은 (summary, exit)에서
    // 파생되므로 규칙을 하나씩 고르면 규칙 4의 exit 교차검증이 누락된다
    let summaries: Vec<Option<TestSummary>> = vec![
        None,
        Some(TestSummary { ran: 8, passed: 8, failed: 0, filtered_out: 0, failed_names: vec![] }),
        Some(TestSummary { ran: 3, passed: 0, failed: 3, filtered_out: 0,
                           failed_names: vec!["a".into(), "b".into(), "c".into()] }),
        Some(TestSummary { ran: 0, passed: 0, failed: 0, filtered_out: 15, failed_names: vec![] }),
        Some(TestSummary { ran: 0, passed: 0, failed: 0, filtered_out: 0, failed_names: vec![] }),
    ];
    let exits: Vec<Option<String>> = vec![None, Some("0".into()), Some("101".into())];

    let mut rendered = 0usize;
    let mut expected = 0usize;
    for muts in [false, true] {
        for since in [false, true] {
            for s in &summaries {
                for e in &exits {
                    for piped in [false, true] {
                        expected += 1;
                        let mut n = StatusNote::new();
                        if muts {
                            n.record_mutation(&json!({"path": "src/lib.rs"}));
                        }
                        n.record_command_result(e.clone(), s.clone(), piped);
                        // turn 5 = 케이던스 — 무뮤테이션 분기도 반드시 렌더된다
                        let Some(note) = n.on_turn(&ctx(5, since)) else { continue };
                        rendered += 1;
                        assert!(note.starts_with(STATUS_MARKER), "{note:?}");
                        for line in note.lines().skip(1) {
                            assert!(line.starts_with(CONT_INDENT), "들여쓰기 없는 고아 줄: {line:?}\n전문:\n{note}");
                        }
                        // 문구의 참/거짓 — 이 마일스톤에서 유일하게 의미를 검사하는 단언
                        if note.contains("no test summary") {
                            assert!(s.is_none(), "요약이 있는데 없다고 렌더: {note}");
                        }
                    }
                }
            }
        }
    }
    // 렌더 건수 == 열거한 상태 수. 하나도 건너뛰지 않았음을 고정한다 —
    // 렌더가 안 일어나는 turn을 고르면 절반이 조용히 사라진다
    assert_eq!(rendered, expected, "렌더가 {rendered}/{expected}건만 일어났다 — turn 선택을 확인할 것");
}
```

- [ ] **Step 7: 통과를 확인한다**

Run: `cargo test --lib rule_4 a_missing_test_summary the_no_summary_phrase every_rendered_status_line`
Expected: PASS 5개

- [ ] **Step 8: 전체 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과. **기존 상태선 테스트가 `record_command_result` 인자 2개로 부르고 있으면 전부 `false`를 더할 것**

- [ ] **Step 9: 커밋**

```bash
git add src/agent/status_note.rs src/agent/mod.rs
git commit -m "feat(agent): 상태선 규칙 4 파이프 폴백과 한정자 (M14 A-2)"
```

---

### Task 6: A-3 — 편집 결과 diff를 모델에게

**Files:**
- Modify: `src/tools/diff.rs` (모델 채널 렌더러 신설), `src/tools/edit_file.rs:381` 부근, `src/tools/write_file.rs:66`
- Test: `src/tools/diff.rs` tests, `src/tools/edit_file.rs` tests

**Interfaces:**
- Consumes: `render_diff(old, new) -> String` (기존)
- Produces: `pub fn render_diff_for_model(old: &str, new: &str) -> String` — 상한 15줄, `-N lines, +M lines` 헤더 필수

**배경**: `render_diff`는 이미 두 도구 안에서 호출되지만 `preview()`를 통해 **승인자에게만** 간다. 모델이 받는 것은 `edit_file` 성공 시 ±3줄 컨텍스트뿐이라 조용한 삭제·중복·무효 편집이 안 보인다.

**예산은 중립이 아니라 유계다**(스펙 §3-5-1) — `render_context`는 편집 크기와 무관하게 ~9줄 상수인데 `render_diff`는 헝크 수에 비례한다(21줄 삭제 27줄, `replace_all`×8 56줄). 상한 15줄이면 ~1.7배다.

- [ ] **Step 1: 실패하는 테스트를 쓴다**

`src/tools/diff.rs`:

```rust
#[test]
fn model_diff_is_capped_and_always_carries_a_count_header() {
    let old: String = (0..100).map(|i| format!("line {i}\n")).collect();
    let new: String = (0..100).filter(|i| *i < 40 || *i >= 61).map(|i| format!("line {i}\n")).collect();
    let d = render_diff_for_model(&old, &new);
    assert!(d.lines().count() <= MODEL_DIFF_MAX_LINES + 1, "상한 초과:\n{d}");
    assert!(d.starts_with("-21 lines, +0 lines"), "헤더가 없다:\n{d}");
}

#[test]
fn model_diff_keeps_deleted_lines_when_truncating() {
    // 조용한 삭제가 주 표적 — 잘릴 때 삭제 줄이 먼저 남아야 한다
    let old: String = (0..60).map(|i| format!("keep {i}\n")).collect();
    let mut new = String::new();
    for i in 0..60 {
        if i == 30 { continue; }            // 삭제 1줄
        new.push_str(&format!("keep {i}\n"));
        if i % 5 == 0 { new.push_str("ADDED\n"); }   // 추가 다수
    }
    let d = render_diff_for_model(&old, &new);
    assert!(d.contains("-keep 30"), "삭제 줄이 절단으로 사라졌다:\n{d}");
}

#[test]
fn model_diff_of_a_tiny_edit_is_smaller_than_the_cap() {
    let d = render_diff_for_model("a\nb\nc\n", "a\nB\nc\n");
    assert!(d.contains("-b") && d.contains("+B"), "{d}");
    assert!(d.lines().count() < MODEL_DIFF_MAX_LINES, "{d}");
}
```

- [ ] **Step 2: 실패를 확인한다**

Run: `cargo test --lib model_diff`
Expected: FAIL — `cannot find function render_diff_for_model`

- [ ] **Step 3: 구현**

`src/tools/diff.rs`에 추가:

```rust
/// 모델 채널 전용 상한. MAX_DIFF_LINES(120)는 승인 게이트용이며 그 값을 매 편집
/// 턴에 붙이면 B-2가 고치려는 컨텍스트 문제를 스스로 악화시킨다 (스펙 §3-5-2)
pub const MODEL_DIFF_MAX_LINES: usize = 15;

/// 모델에게 되돌릴 편집 diff. 헤더는 **필수**다 — EDIT_STRATEGY_CORRECTION과
/// SR_CORRECTION이 막힌 모델을 write_file 전면 재작성으로 유도하는데, 헤더가
/// 없으면 상한에 걸린 큰 diff가 "몇 줄이 사라졌는지"조차 전하지 못한다
pub fn render_diff_for_model(old: &str, new: &str) -> String {
    let text = similar::TextDiff::from_lines(old, new).unified_diff().context_radius(1).to_string();
    let body: Vec<&str> = text
        .lines()
        .filter(|l| !l.starts_with("---") && !l.starts_with("+++") && !l.starts_with("@@"))
        .collect();
    let removed = body.iter().filter(|l| l.starts_with('-')).count();
    let added = body.iter().filter(|l| l.starts_with('+')).count();
    let header = format!("-{removed} lines, +{added} lines");

    if body.len() <= MODEL_DIFF_MAX_LINES {
        return format!("{header}\n{}", body.join("\n"));
    }
    // 절단: 삭제 줄 우선. 조용한 삭제가 A-3의 주 표적이고, 추가 줄은 모델이
    // 방금 자기가 쓴 내용이라 신호 가치가 낮다
    let mut kept: Vec<&str> = body.iter().filter(|l| l.starts_with('-')).copied().take(MODEL_DIFF_MAX_LINES).collect();
    for l in body.iter().filter(|l| !l.starts_with('-')) {
        if kept.len() >= MODEL_DIFF_MAX_LINES {
            break;
        }
        kept.push(l);
    }
    format!("{header}\n{}\n[diff truncated]", kept.join("\n"))
}
```

- [ ] **Step 4: 통과를 확인한다**

Run: `cargo test --lib model_diff`
Expected: PASS 3개

- [ ] **Step 5: `edit_file`이 diff로 컨텍스트를 대체하게 한다**

`src/tools/edit_file.rs`의 `run()` 마지막 줄을 교체. **±3줄 컨텍스트를 대체한다 — 추가가 아니다**:

```rust
        Ok(format!("{head}\n{}", render_diff_for_model(&_old, &outcome.new_text)))
```

`_old` 바인딩 이름을 `old`로 바꾸고 `let (old, outcome, crlf) = self.dry_run(...)`로 고칠 것. `render_context`가 더 이상 안 쓰이면 `dead_code`로 clippy가 깨진다 — **`#[cfg(test)]`로 남기거나 제거하되, 제거하면 그 함수의 기존 테스트도 함께 지울 것.**

- [ ] **Step 6: `write_file`이 diff를 첨부하게 한다**

`src/tools/write_file.rs`의 `run()` 마지막:

```rust
        // 신규 파일과 비UTF-8은 existing_text()가 둘 다 None을 준다(주석 참조).
        // 신규는 전 줄이 추가라 신호가 0이고 비UTF-8은 diff를 낼 원문이 없다 —
        // 둘 다 현행 요약 줄을 유지한다 (스펙 §3-5-2)
        Ok(match old_text {
            Some(old) => format!(
                "Wrote {} ({} lines)\n{}",
                args.path,
                normalized.lines().count(),
                render_diff_for_model(&normalize_eol(&old), &normalized)
            ),
            None => format!("Wrote {} ({} lines)", args.path, normalized.lines().count()),
        })
```

`old_text`는 기존 `existing_text(&path)` 호출을 **쓰기 전에** 한 번 받아 두는 바인딩이다 — 현재 `crlf` 계산이 이미 호출하므로 그것을 재사용할 것:

```rust
        let old_text = existing_text(&path);
        let crlf = old_text.as_deref().map(dominant_crlf).unwrap_or(false);
```

- [ ] **Step 7: 통합 단언 — 조용한 삭제가 모델에게 보인다**

`src/tools/edit_file.rs` tests:

```rust
#[test]
fn a_deletion_is_visible_in_the_result_the_model_receives() {
    let (dir, ctx) = setup("pub const A: u8 = 1;\npub const B: u8 = 2;\npub const C: u8 = 3;\nfn f() {}\n");
    let out = EditFile
        .run(
            &serde_json::json!({
                "path": "f.rs",
                "search": "pub const A: u8 = 1;\npub const B: u8 = 2;\npub const C: u8 = 3;\n",
                "replace": ""
            }),
            &ctx,
        )
        .unwrap();
    assert!(out.contains("-3 lines"), "삭제 개수가 안 보인다:\n{out}");
    assert!(out.contains("-pub const A"), "삭제된 줄이 안 보인다:\n{out}");
    drop(dir);
}
```

- [ ] **Step 8: 전체 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과. **`edit_file`의 성공 결과 문자열을 단언하는 기존 테스트가 다수 깨진다** — `render_context`의 출력(`line 12:` 같은 형태)을 기대하는 것들이다. 각각 새 diff 형태로 갱신하되, **그 테스트가 고정하던 성질(편집이 적용됐다·모드가 맞다)은 유지**할 것

- [ ] **Step 9: 커밋**

```bash
git add src/tools/diff.rs src/tools/edit_file.rs src/tools/write_file.rs
git commit -m "feat(tools): 편집 diff를 모델 결과에 첨부 (M14 A-3)"
```

---

### Task 7: B-3 `schema_fallback_count` + B-4 거부 경로 `!stop` 가드 테스트

**Files:**
- Modify: `src/eval/report.rs` (집계 2곳)
- Test: `src/eval/report.rs` tests, `src/agent/mod.rs` tests

**Interfaces:**
- Consumes: 기존 `RunRecord::schema_fallback: bool`
- Produces: `TaskReport::schema_fallback_count: usize`, `Report::schema_fallback_count: usize`

**배경(B-3)**: 앵커 게이트가 `tasks[].runs[].schema_fallback`을 전수 순회해야 하는데, 그 스크립트를 잘못 짜면(첫 런만 본다든지) **fail-open 위험이 Rust에서 스크립트로 이동할 뿐**이다. M6이 `_count` 접미 집계를 도입한 이유가 정확히 이것이다. 기존 키 불변의 가산 변경.

**배경(B-4)**: 거부 경로에는 `!stop` 가드가 **둘** 있다(finish_nudge 억제 / status 억제). 각각 독립적으로 제거해도 전체 스위트가 초록불이다(1R 실측). **B-4는 동작을 고치지 않는다 — 핀을 추가한다.**

- [ ] **Step 1: B-3의 실패하는 테스트를 쓴다**

`src/eval/report.rs` tests:

```rust
#[test]
fn schema_fallback_count_aggregates_like_the_other_count_fields() {
    let runs = vec![
        run_with(true, "finished", true),
        run_with(true, "finished", false),
        run_with(false, "repetition_stop", true),
    ];
    let t = TaskReport::from_runs("t".into(), runs);
    assert_eq!(t.schema_fallback_count, 2);
}

#[test]
fn report_json_carries_schema_fallback_count_at_both_levels() {
    let v = sample_report_json();
    assert!(v.get("schema_fallback_count").is_some(), "최상위 집계가 없다");
    assert!(v["tasks"][0].get("schema_fallback_count").is_some(), "과제별 집계가 없다");
    // 기존 키 불변
    assert!(v["tasks"][0]["runs"][0].get("schema_fallback").is_some());
}
```

`run_with`/`sample_report_json`은 같은 파일의 기존 헬퍼 형태를 따를 것(`:170`·`:214` 참조). 세 번째 인자가 `schema_fallback`이 되도록 헬퍼를 넓힌다.

- [ ] **Step 2: 실패를 확인한다**

Run: `cargo test --lib schema_fallback_count`
Expected: FAIL — `no field schema_fallback_count`

- [ ] **Step 3: 집계를 더한다**

`TaskReport`에 필드와 계산:

```rust
    pub schema_fallback_count: usize,
```
```rust
            schema_fallback_count: runs.iter().filter(|r| r.schema_fallback).count(),
```

`Report`에도 같은 이름의 필드를 더하고, 최상위 계산은 기존 `passed_count` 합산과 같은 자리에서:

```rust
            schema_fallback_count: tasks.iter().map(|t| t.schema_fallback_count).sum(),
```

**표 출력(`:139` 부근)은 건드리지 않는다** — 열 추가는 범위 밖이다.

- [ ] **Step 4: 통과를 확인한다**

Run: `cargo test --lib schema_fallback_count`
Expected: PASS 2개

- [ ] **Step 5: B-4 — 두 가드를 각각 핀하는 테스트를 쓴다**

`src/agent/mod.rs` tests. **줄번호가 아니라 성질로 지목한다** — 이 태스크 앞에서 T2~T5가 같은 파일을 고쳐 줄이 밀린다:

```rust
#[tokio::test]
async fn a_rejected_mutation_on_a_repetition_stop_turn_emits_no_status_note() {
    // 거부 경로의 status 억제 `!stop` 가드를 핀한다. 이 가드를 제거하면
    // RepetitionStop 턴에 상태선이 붙어 M11의 불변식이 깨진다
    let deny = json!({"path":"a.rs","content":"x"});
    let mut script = Vec::new();
    for _ in 0..5 {
        script.push(ok(&call("write_file", deny.clone())));
    }
    let (out, notes) = run_with_approver(script, Box::new(PanicApprover::denying())).await;
    assert!(matches!(out, AgentOutcome::RepetitionStop));
    let last = notes.last().expect("tool_result가 있어야 한다");
    assert!(!last.contains(status_note::STATUS_MARKER), "정지 턴에 상태선이 붙었다: {last}");
}

#[tokio::test]
async fn a_rejected_action_on_a_repetition_stop_turn_emits_no_finish_nudge() {
    // 거부 경로의 finish_nudge 억제 `!stop` 가드를 핀한다.
    // **이 가드는 도달 불가일 가능성이 높다**(스펙 §4-4·§11 Q3): 거부 경로의
    // 이벤트는 MutationAttempt(→disarm)와 Other(→상태 불변)뿐이라 armed가
    // 참이 될 수 없다. 그렇다면 이 테스트는 **가드 유무와 무관하게 통과**하며
    // 핀으로서 무력하다 — 그때는 아래 대체 단언을 쓸 것
    let mut n = finish_nudge::FinishNudge::new();
    n.on_turn(finish_nudge::TurnEvent::MutationOk);
    n.on_turn(finish_nudge::TurnEvent::VerifyOk { repeat: false });
    for _ in 0..4 {
        n.on_turn(finish_nudge::TurnEvent::ReadOnly { repeat: true });
    }
    // 여기서 무장·카운트가 찼다. 거부 경로 이벤트 2종이 발동시키지 못함을 단언
    let mut a = n.clone_for_test();
    assert!(a.on_turn(finish_nudge::TurnEvent::MutationAttempt).is_none(), "MutationAttempt가 발동시켰다");
    let mut b = n.clone_for_test();
    assert!(b.on_turn(finish_nudge::TurnEvent::Other).is_none(), "Other가 발동시켰다");
}
```

**Step 5-a (필수 판정)**: 첫 테스트를 쓴 뒤 **거부 경로의 status `!stop` 가드를 임시로 제거하고 돌려 실패하는지 확인**할 것. 실패하지 않으면 그 테스트는 핀이 아니다 — 시나리오를 고쳐야 한다.

**Step 5-b**: 두 번째 테스트에 `clone_for_test`가 필요하면 `#[cfg(test)] impl FinishNudge { fn clone_for_test(&self) -> Self }`를 더하거나, `FinishNudge`에 `#[derive(Clone)]`을 붙일 것(상태가 전부 `Clone`이다). **도달 불가가 증명되면 그 사실을 `agent/mod.rs`의 가드 옆 주석에 적고 이 테스트를 산출물로 남긴다 — 주석만으로는 수용 기준이 안 된다**(스펙 §7 기준 4).

- [ ] **Step 6: 전체 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과

- [ ] **Step 7: 커밋**

```bash
git add src/eval/report.rs src/agent/mod.rs src/agent/finish_nudge.rs
git commit -m "feat(eval): schema_fallback_count 집계 + 거부 경로 !stop 가드 회귀 테스트 (M14 B-3·B-4)"
```

---

### Task 8: C — `pilot.sh` 하드닝 4건

**Files:**
- Modify: `scripts/pilot.sh`

**Interfaces:**
- Consumes: 없음
- Produces: 없음 (스크립트)

**⚠ 검증 규율**: `pilot.sh`는 `/bin/sh`로 돈다. **대화형 셸의 함수·별칭·PATH가 검증을 무효화한다** — M13에서 BSD `find` 회귀가 "실측 확인"을 통과해 커밋된 전례가 있다. **모든 확인은 `/bin/sh scripts/pilot.sh` 형태로 직접 돌릴 것.** 외부 명령 동작은 `/usr/bin/`의 실제 바이너리로 확인한다. macOS에는 `/bin/true`가 없다 — `/usr/bin/true`를 쓸 것.

- [ ] **Step 1: 타임아웃 검증을 더한다**

**후보 문서와 스펙 초판이 인용한 "(실측)"은 거짓이었다** — macOS `/bin/sleep`은 단위 접미사를 받는다(`600s`·`10m`·`1e3` 전부 수용). 실제 실패 모드는 `10m`이 **조용히 600초로 해석**되는 것이다(변수 이름은 `_SECS`인데). 따라서 **숫자만 허용**한다.

`scripts/pilot.sh:17-18` 아래에 추가:

```sh
# 비숫자 값은 감시자의 `sleep`을 즉시 죽여 상한을 사라지게 하고, `10m` 같은
# 단위 접미사는 조용히 600초로 해석돼 변수 이름(_SECS)과 어긋난다.
# 선행 `-`는 sleep의 getopt 단계에서 다른 오류가 되므로 case가 먼저 거른다
for _v in BUILD TEST; do
  eval "_val=\$PILOT_${_v}_TIMEOUT_SECS"
  case "$_val" in
    ''|*[!0-9]*)
      echo "PILOT_${_v}_TIMEOUT_SECS는 초 단위 정수여야 합니다 (받은 값: '$_val')" >&2
      exit 1
      ;;
  esac
done
unset _v _val
```

- [ ] **Step 2: 확인한다**

```bash
/bin/sh -c 'PILOT_BUILD_TIMEOUT_SECS=10m /bin/sh scripts/pilot.sh </dev/null; echo "rc=$?"'
```
Expected: 거부 메시지 + `rc=1`

```bash
/usr/bin/env PILOT_BUILD_TIMEOUT_SECS=300 /bin/sh -n scripts/pilot.sh; echo "syntax rc=$?"
```
Expected: `syntax rc=0`

- [ ] **Step 3: cwd 전제를 고친다**

`scripts/pilot.sh:19`:

```sh
# git diff는 레포 전체를 보므로 REPO도 레포 루트여야 한다 — 서브디렉터리에서
# 실행하면 어긋난다
REPO="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "git 레포 안에서 실행해야 합니다"; exit 1
}
[ -n "$REPO" ] || { echo "git 레포 안에서 실행해야 합니다"; exit 1; }
```

- [ ] **Step 4: 확인한다**

임시 레포에서 커밋 1개 + 클린 트리 + 하위 디렉터리를 만들고 그 안에서 실행한다. **워킹트리가 더러우면 `y/N` 프롬프트에서 멈추므로 전제조건이다**:

```bash
T=$(mktemp -d) && cd "$T" && git init -q && mkdir -p sub && echo x > f.txt \
  && git add -A && git -c user.email=a@b -c user.name=c commit -qm init \
  && cd sub && LOCO_BIN=/usr/bin/true PILOT_LEDGER=/tmp/l.jsonl \
     /bin/sh "$OLDPWD/../../scripts/pilot.sh" </dev/null 2>&1 | head -5
```
Expected: `REPO`가 레포 루트로 잡혀 진행(경로 오류 없음). 경로는 실제 위치에 맞게 조정할 것

- [ ] **Step 5: 세션 전 `read` 4개에 EOF 안내를 더한다**

`:96`·`:102`·`:104`·`:106`의 각 `read -r X`를 다음 형태로:

```sh
read -r TASK_TYPE || { echo "입력이 필요합니다 (세션 전 수집은 의도적으로 비대화형 실행을 지원하지 않습니다)" >&2; exit 1; }
```

**주석으로 "의도적 제외"를 명시한다** — 세션 시작 전이라 하드닝 비용이 0이고, 종료 코드 1이 실제 오류와 구별되지 않던 것만 고치는 것이다.

- [ ] **Step 6: INT 트랩을 문서화한다** (동작 변경 없음)

`:61`의 `trap 'exit 130' INT` 위에 주석:

```sh
# 실측(bash 3.2 = macOS /bin/sh): 포그라운드 자식 대기 중 프로세스 그룹 SIGINT는
# 이 트랩을 실행시키지 않는다. 따라서 안전망이 실제로 발동하는 것은 판정 프롬프트
# 대기 중뿐이다. 결과적 동작은 바람직하므로(loco만 죽고 세션은 기록됨) 그대로 둔다.
# 세션 중 INT까지 잡으려면 wait 기반 구조가 필요하다 — 다음으로 미룸
```

- [ ] **Step 7: 세 전제조건 확인을 한 번에 돌린다**

```bash
/bin/sh -n scripts/pilot.sh && echo "syntax ok"
```
그리고 Step 2·4의 확인을 재실행한다. 그리고 EOF 안내:
```bash
LOCO_BIN=/usr/bin/true /bin/sh scripts/pilot.sh </dev/null 2>&1 | tail -3
```
Expected: 안내 한 줄이 보인다(더러운 트리 프롬프트에 먼저 걸리면 클린 레포에서 다시 돌릴 것)

- [ ] **Step 8: 커밋**

```bash
git add scripts/pilot.sh
git commit -m "fix(scripts): pilot.sh 하드닝 4건 — 타임아웃 검증·cwd·EOF·INT 문서화 (M14 C-1)"
```

---

### Task 9: C — 트랜스크립트 영속화 2건

**Files:**
- Modify: `src/agent/mod.rs` (`finish_reason` 기록, `AgentEvent::Notice` 기록)
- Test: `src/agent/mod.rs` tests

**Interfaces:**
- Consumes: `Session::record_extra(kind, content)`
- Produces: 트랜스크립트에 `kind = "finish_reason"`, `kind = "notice"` 행

**배경**: M13이 남긴 계측 불가 항목 2건이다. `finish_reason`은 `length` 턴 수를 직접 셀 수 없게 했고(빈-content 턴을 대리 지표로 씀), 오버플로 `Notice`는 파일럿 표에서 그 행을 **0이 아니라 미측정**으로 만들었다. M13 스펙 리뷰의 C1이 정확히 이 공백 때문에 "배치는 정상 종료했는데 흔적 0"인 시나리오를 만들어냈다.

- [ ] **Step 1: 실패하는 테스트를 쓴다**

```rust
#[tokio::test]
async fn finish_reason_and_overflow_notices_reach_the_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let tpath = dir.path().join("t.jsonl");
    let transcript = Transcript::create_at(&tpath).unwrap();
    let script = vec![ok_with_reason("cut", "length"), ok(&finish("done"))];
    let agent = Agent::new(Box::new(Scripted::new(script)), Config::default(), Registry::guided());
    let mut session = Session::new(vec![ChatMessage::system("sys")], transcript);
    session.push(ChatMessage::user("TASK"));
    let _ = agent.run_with_session(&mut session, &mut |_| {}).await.unwrap();

    let jsonl = std::fs::read_to_string(&tpath).unwrap();
    assert!(jsonl.contains("\"finish_reason\""), "finish_reason이 안 남았다");
    assert!(jsonl.contains("length"), "length 값이 안 남았다");
}
```

- [ ] **Step 2: 실패를 확인한다**

Run: `cargo test --lib finish_reason_and_overflow_notices`
Expected: FAIL

- [ ] **Step 3: `finish_reason`을 기록한다**

`agent/mod.rs`의 응답 수신 직후(length 판정 **전**):

```rust
            if let Some(fr) = resp.finish_reason() {
                session.record_extra("finish_reason", fr);
            }
```

- [ ] **Step 4: `Notice`를 기록한다**

`on_event(AgentEvent::Notice(...))` 호출은 19곳이다. **전부 고치지 말고** 헬퍼를 하나 만들어 오버플로·파싱 실패 등 진단 가치가 있는 지점에서만 쓴다:

```rust
/// 진단 가치가 있는 Notice는 트랜스크립트에도 남긴다 (M14 C-2).
/// M13에서 오버플로가 "0이 아니라 미측정"이 된 원인
macro_rules! notice_recorded {
    ($session:expr, $on_event:expr, $msg:expr) => {{
        let m: String = $msg;
        $session.record_extra("notice", &m);
        $on_event(AgentEvent::Notice(m));
    }};
}
```

**최소 적용 지점 2곳**: 컨텍스트 오버플로 400(`:227-229`), length 절단(`:246`). 다른 지점은 건드리지 않는다 — 범위를 늘리면 트랜스크립트가 비대해진다.

- [ ] **Step 5: 통과를 확인한다**

Run: `cargo test --lib finish_reason_and_overflow_notices`
Expected: PASS

- [ ] **Step 6: 전체 게이트 + 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
git add src/agent/mod.rs
git commit -m "feat(agent): finish_reason·진단 Notice를 트랜스크립트에 영속화 (M14 C-2)"
```

---

### Task 10: C — `exp_metrics.py` 확장

**Files:**
- Modify: `scripts/exp_metrics.py`

**Interfaces:**
- Consumes: T4·T5·T6이 만든 마커 문자열
- Produces: 신규 컬럼 `pipe_unreleased` · `verify_nudge_pipe` · `finish_nudge_total` · `model_diff` · `status_pipe_qual` · `status_no_summary`

**⚠ 이 태스크는 Rust↔Python 손복사의 미러 작업이다.** 자동 검출이 없다.

- [ ] **Step 1: `parse_fail_first` 출력 형식을 고친다**

`scripts/exp_metrics.py:350` 부근. 다른 전 필드가 `key=value`인데 이것만 한글 키 + 서술문이라 grep 레시피가 깨지고, CLAUDE.md의 "identifiers는 영문" 관례와도 어긋난다. `key=value` 한 항목으로 바꿀 것.

- [ ] **Step 2: 신규 마커 카운트를 더한다**

`MARKERS` 딕셔너리(`:33` 부근)에 추가. **문자열은 Rust 상수와 문자 그대로 일치해야 한다** — T4·T5·T6의 실제 값을 복사할 것:

```python
    "verify_nudge_pipe": "but it was a shell pipeline",
    "finish_nudge_pipe": "was a shell pipeline, so it did not establish",
    "status_pipe_qual": "(via pipe",
    "status_no_summary": "no test summary in output",
    "model_diff": " lines, +",
```

- [ ] **Step 3: `verify_*` 비교가능성 주석을 더한다**

**A-2의 규칙 4 → 규칙 5 폴백이 `verify_allpass`·`verify_total`의 모집단을 줄인다.** `:33-35`의 매처 정의 위에 주석:

```python
# ⚠ M14 비교가능성: §3-4-2의 규칙 4 → 규칙 5 폴백이 파이프 실행의 allpass 렌더를
# 규칙 5 문자열로 옮긴다. 모델이 파이프를 쓰는 만큼 verify_allpass·verify_total이
# **내려간다** — 하락은 회귀가 아니라 폴백이 작동한 증거일 수 있다.
# 파생 verify_failed(= total - allpass)는 규칙 2가 불변이고 두 원지표가 같은 양만큼
# 줄어 보존된다. M14 전후 배치의 이 두 지표를 나란히 인용하지 말 것.
# 선례: M12 sr_error(검사 순서), M13 T7 verify_*(무뮤테이션 렌더로 상향)
```

- [ ] **Step 4: `--selftest`를 확장한다**

내장 픽스처 트랜스크립트에 신규 마커를 포함시키고 기대값을 단언한다. `process()`를 합성 임시 스탬프 디렉터리로 돌리는 기존 경로도 유지할 것.

- [ ] **Step 5: 확인한다**

```bash
python3 scripts/exp_metrics.py --selftest
```
Expected: `selftest ok`

**과거 배치로 회귀 확인** — 신규 컬럼이 0으로 나오고 기존 컬럼이 안 바뀌어야 한다:
```bash
ls -d .loco/eval/*/ | tail -1 | xargs python3 scripts/exp_metrics.py
```
Expected: 정상 출력, 신규 컬럼 0

- [ ] **Step 6: 커밋**

```bash
git add scripts/exp_metrics.py
git commit -m "feat(scripts): exp_metrics M14 마커·출력 형식·verify_* 비교가능성 주석 (M14 C-3)"
```

---

### Task 11: D — 봉투 명시와 문서 갱신

**Files:**
- Modify: `README.md`, `CLAUDE.md`, `docs/baselines.md`
- Create: 없음 (봉투 서술은 README 절로)

**Interfaces:** 없음 (문서)

**⚠ 인용 규율 — 스펙 §6이 금지 서술과 분모 규칙을 못박는다. 그대로 옮길 것.**

- [ ] **Step 1: README에 "loco가 서비스하는 과제 봉투" 절을 쓴다**

내용:
- **경로 지정 과제가 성공 봉투다** — 파일럿 13세션 중 성공 7(감사 전 최대치), 경로 미지정 7세션은 0/7
- **성공 봉투의 크기** — 무경합 성공 4건의 diff 추가줄은 +0~23, 추가줄 29 이상인 4건 중 무경합 성공은 0건
- **분모 규칙 병기 의무**: 성공 7은 무효 Z3와 감사 강등 F5·R5를 포함한 감사 전 최대치이고, 같은 규칙을 적용하면 **무경합 기준 4/12**다. 0/7 쪽은 어떤 완화도 안 받으므로 두 수치를 나란히 쓸 때 반드시 병기
- **증거 강도 상한**: 사후 분해이고 사전등록되지 않았으며 20과제 중 19개를 컨트롤러가 설계했다. **"이 유형이 실패한다"와 "내가 설계한 이 유형의 과제 7개가 실패한다"를 이 데이터로 구분할 수단이 없다.** n=7 vs 13, 사례로 읽을 것
- **금지 서술**: *"합성 세트에서 이미 이 축만 흔들리고 있었다"*고 쓰지 말 것. 축 안 29/30 vs 축 밖 41/42이고 엄격 기준으로는 축 밖이 더 나쁘다(29/30 vs 40/42). 대비를 만드는 것은 실레포 쪽 0/7이다. `29/30` 단독 인용 시 `17/18`(심볼만 주는 3과제)이 더 엄밀한 대응임을 병기

**거절/경고 코드는 넣지 않는다.**

- [ ] **Step 2: `docs/baselines.md`에 M14 절 골격을 만든다**

Task 13이 측정 결과를 채운다. 지금은 **비교가능성 각주**만 먼저 적는다(스펙 §8-3 전문을 옮길 것) — 측정 후에 쓰면 데이터를 본 뒤 각주를 쓰는 것이 된다.

- [ ] **Step 3: `CLAUDE.md`를 갱신한다**

- 헤더의 `M1-M13 done`을 `M1-M14 done`으로
- Architecture 절에 M14 문단 추가 (영문): 파이프 가드가 해제 술어에만 걸린다는 것, 소비자 5종, `render_diff_for_model` 상한, `push_recovery_notice`, `reasoning_content`
- Commands 절에 `exp_metrics.py`의 신규 컬럼 언급
- **`verify_allpass`/`verify_total`의 M14 비교가능성 경고를 반드시 포함** — M12 `sr_error` 경고와 같은 자리에

- [ ] **Step 4: 확인 + 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify
```
Expected: 12/12, 3/3 (**픽스처를 안 건드렸으므로 변화가 있으면 이상 신호**)

```bash
git add README.md CLAUDE.md docs/baselines.md
git commit -m "docs(m14): 과제 봉투 명시·비교가능성 각주·아키텍처 갱신 (M14 D)"
```

---

### Task 12: 사전등록 — **사용자 승인 게이트에서 정지**

**Files:**
- Create: `docs/experiments/2026-07-20-honest-verification-ii/preregistration.md`

**Interfaces:** 없음

**⚠ 이 태스크는 문서를 쓰고 멈춘다. 사용자 승인 없이 Task 13(GPU 배치)에 진행하지 말 것.**
**승인은 문서의 상태 행 커밋으로만 성립한다 — 전언 승인 불가(M11·M12·M13 전례).**

- [ ] **Step 1: `docs/experiments/PROTOCOL.md`와 `TEMPLATE`을 읽는다**

- [ ] **Step 2: 사전등록을 쓴다**

반드시 포함할 것:

| 항목 | 값 |
|---|---|
| 세트 | `tasks/` 스포트 (12과제 × 3반복 = 36런) |
| 대조 | `20260719T093254Z` — 35/36, 엄격 35 (**M13 게이트 배치**. 앵커 `20260719T082030Z`가 아니다 — 이쪽이 M14 직전 상태다) |
| 임계값 | **≥33/36** — M12 방식. M13의 "앵커−4"(=31)를 채택하지 않는 근거: M13은 *서빙 스택 전환의 동등성* 판정이었고 M14는 동일 스택의 회귀 게이트라 더 조이는 쪽이 맞다 |
| 재측정 | **1회 사전 공약.** 재측정도 임계 미달이면 **비병합·정지·사용자 보고**(추가 재측정 없음) |
| 배치 사망 정의 | 하네스 에러/서버 다운/Ctrl+C로 `report.json`이 안 나온 경우만. **정상 종료한 낮은 통과 수는 사망이 아니다**(M12 교훈) |

**관측 항목(게이트 아님, 판정에 쓰지 않음)을 사전에 선언할 것**:
- A-1 파이프 해제 차단 발동 횟수 · 오발동 여부(전수 확인) · 새 VERIFY_NUDGE 문구 발동
- **FINISH_NUDGE 발동 횟수** — §3-3-3의 회귀를 배치에서도 볼 수 있게
- A-3 diff 첨부 횟수 · 절단 횟수
- 풍선효과: finish 누락 스트릭 · 거짓 finish · `stop_cause` 분포 · 동일 파이프 명령 재실행 반복 정지 · **`REPEAT_CORRECTION` 직후 finish가 파이프 VERIFY_NUDGE로 거부된 횟수**

**비교가능성 경고를 사전등록에 적을 것**: `verify_allpass`·`verify_total`은 M14 전후 직접 비교 불가(스펙 §8-3).

- [ ] **Step 3: 배치 전 점검 목록을 적는다**

- `.loco/config.toml`을 대조 배치의 `effective_config`에 정합 (**M12 배치 2 조건 `command_timeout_secs=240`이 남아 있을 수 있다 — 반드시 확인**)
- **`--release` 금지** (대조가 디버그 빌드. 빌드 프로파일은 `report.json`에 안 남아 나중에 발견 불가능)
- `ls ${TMPDIR}/.cargo` — 존재 시 수동 제거
- `setsid` 데몬화 (백그라운드 60분 수명 상한)
- 측정 중 빌드 병행 금지

- [ ] **Step 4: 커밋하고 정지한다**

```bash
git add docs/experiments/2026-07-20-honest-verification-ii/preregistration.md
git commit -m "docs(m14): 회귀 게이트 사전등록 — 사용자 승인 대기"
```

**여기서 멈춘다.** 사용자에게 사전등록 경로와 임계값을 보고하고 승인을 요청한다.

---

### Task 13: 측정 · 판정 · 병합

**⚠ Task 12의 승인이 문서 상태 행 커밋으로 성립한 뒤에만 착수한다.**

**Files:**
- Create: `docs/experiments/2026-07-20-honest-verification-ii/report.md`
- Modify: `docs/baselines.md` (M14 절 채우기)

- [ ] **Step 1: 배치 전 점검을 실행한다**

```bash
cat .loco/config.toml
ls ${TMPDIR}/.cargo 2>/dev/null && echo "⚠ 트립와이어 — 수동 제거 필요"
git rev-parse HEAD
```
`effective_config`가 대조 배치와 맞는지 확인할 것.

- [ ] **Step 2: 게이트 배치를 돌린다**

```bash
setsid nohup cargo run -- eval tasks --repeats 3 --seed 0 > /tmp/m14-gate.log 2>&1 &
```
**통지에 의존하지 말 것** — 종료는 exit code와 스탬프 디렉터리로 직접 확인한다(M10 운영 교훈).

- [ ] **Step 3: 결과를 `report.json`으로 직접 대조한다**

```bash
python3 -c "
import json,sys
r=json.load(open(sys.argv[1]))
print('passed', r['passed_count'], 'strict', r['passed_strict_count'], 'false_finish', r['false_finish_count'])
print('schema_fallback', r.get('schema_fallback_count'))
" .loco/eval/<stamp>/report.json
```

**러너 보고를 그대로 옮기지 말 것** — `report.json` 직접 대조가 규율이다(M12에서 서사가 뒤집힌 전례).

- [ ] **Step 4: 지표를 추출한다**

```bash
python3 scripts/exp_metrics.py .loco/eval/<stamp>
```

관측 항목을 사전등록 목록과 대조한다. **신규 장치가 0건이면 그 사실 자체를 기록할 것** — "발동하지 않았다"와 "불필요하다"는 다르다(M13의 `ARGS_TOOL_SWITCH_NOTE` 전례).

- [ ] **Step 5: 판정한다**

- **≥33/36**: 통과 → Step 6
- **<33/36**: 재측정 1회(사전 공약). 재측정도 미달이면 **비병합·정지·사용자 보고**

**법의학 규율**: 실패 런이 있으면 신규 장치 귀속 여부를 전수 확인할 것. M12에서 이상 징후 3건이 전부 신규 장치 밖 원인으로 밝혀진 전례가 있다. **"개입이 궤적을 바꿨다"는 주장은 원장 판정과 diff 실물로 교차 확인한 뒤에만 쓴다.**

- [ ] **Step 6: 리포트와 baselines를 쓴다**

`report.md`에 스탬프·커밋·수치·관측 항목·법의학을 적는다. `docs/baselines.md`의 M14 절을 채운다 — **Task 11에서 미리 적은 비교가능성 각주를 지우지 말 것.**

- [ ] **Step 7: 최종 게이트**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify
python3 scripts/exp_metrics.py --selftest
/bin/sh -n scripts/pilot.sh && echo "pilot syntax ok"
```
Expected: 전건 통과 · 12/12 · 3/3 · `selftest ok` · `pilot syntax ok`

- [ ] **Step 8: 브랜치 리뷰를 요청한다**

`superpowers:requesting-code-review`. **리뷰어에게 요구할 것**: 코드 실측 대조, **변이 테스트 증거**(M12 교훈 — 공허 테스트·생존 변이를 이 방식으로 적발했다), **귀인 축**, 그리고 **두 층 소비자 감사**(모델 대면 + 계측 — 이 마일스톤의 스펙 리뷰가 5라운드에 걸쳐 배운 것).

- [ ] **Step 9: 병합**

```bash
git checkout main
git merge --no-ff m14/honest-verification-ii
cargo test && cargo clippy --all-targets -- -D warnings
```
**푸시는 사용자가 지시할 때만.**

---

## Self-Review 결과

**스펙 커버리지**: A-1 → T4 / A-2 → T5 / A-3 → T6 / B-1 → T3 / B-2(c) → T1 / B-2(b)+§4-2-1 → T2 / B-3·B-4 → T7 / C-1 → T8 / C-2 → T9 / C-3 → T10 / D → T11 / §8 측정 → T12·T13. **§2-1 제외 항목은 태스크 없음이 정답이다.**

**스펙 §7 수용 기준 6종의 위치**: 기준 1 → T5 Step 6 / 기준 2 → T3 Step 1(역직렬화 층) + Step 6(트랜스크립트 층) / 기준 3 → T2 Step 5(단언 ①②③④, 실 파일 트랜스크립트) / 기준 4 → T7 Step 5 / 기준 5 → T8 Step 2·4·7 / 기준 6 → T4 Step 1의 `a_pipe_still_counts_for_loop_detection`.

**타입 일관성**: `render_diff_for_model`(T6)·`push_recovery_notice`(T2)·`LENGTH_RECOVERY`(T2)·`VERIFY_NUDGE_PIPE`(T4)·`FINISH_NUDGE_PIPE`(T4)·`record_command_result(exit, summary, piped)`(T5)·`schema_fallback_count`(T7)가 정의 태스크와 사용 태스크에서 같은 이름이다.

**알려진 편차 1건**: T5가 `record_command_result`에 명령 문자열이 아니라 파생 `bool`을 넘긴다 — 근거는 T5 본문에 적었고 태스크 리뷰어에게 판단을 요청한다.

**열린 질문의 처리**: 스펙 §11 Q1(문구 4종)은 T4·T5가 구체 문자열을 확정한다. Q2(diff 상한·헤더)는 T6이 15줄과 `-N lines, +M lines`로 확정한다. Q3(`:411` 도달 가능성)는 T7 Step 5-b가 판정한다. Q4(B-2c 형태)는 T1이 하한+경고 둘 다로 확정한다. Q5(임계값)는 T12가 `≥33`으로 확정한다. Q6(행렬 형태)은 T5 Step 6이 확정한다.
