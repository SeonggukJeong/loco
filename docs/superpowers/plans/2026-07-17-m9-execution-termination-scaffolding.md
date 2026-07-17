# M9 실행·종료 스캐폴딩 구현 플랜

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M8 실패 데이터가 지목한 두 루프(`edit_file` search==replace 자기-버그, finish 종료 실패)를 스캐폴딩으로 끊고, 리워드된 픽스처에서 2단 측정(재베이스라인 → 스캐폴딩 후)으로 효과를 행동 지표 중심으로 판정한다.

**Architecture:** 도구 층(edit_file 오류문 처방 추가) + 에이전트 층(repetition.rs에 S/R 전용 2연속 교정, agent/mod.rs에 finish 인자누락 2연속 교정, 신규 `agent/finish_nudge.rs` 상태기계로 검증완료 후 반복 재확인 감지). 측정은 코드 계측 없이 트랜스크립트 grep/python 추출.

**Tech Stack:** Rust edition 2024 (신규 크레이트 없음), LM Studio(`lms` CLI + `curl localhost:1234/api/v0/models`), jq/python3(지표 추출).

**스펙:** `docs/superpowers/specs/2026-07-17-m9-execution-termination-scaffolding-design.md` (전문 리뷰 2R Ready=Yes). 이하 "§n"은 이 스펙의 절 번호.

## Global Constraints

- Edition 2024. **신규 의존성 추가 금지** (스펙 하드 제약 — 크레이트 추가는 사용자 승인 필요)
- 매 태스크 게이트: `cargo test` + `cargo clippy --all-targets -- -D warnings` (테스트 코드도 린트)
- 모델-대면 텍스트(오류문·교정문)는 **영문**, 사용자 CLI 메시지·Notice는 한국어, 문서는 한국어
- `tasks/`·`tasks-large/` 픽스처 변경 일절 금지. report.json 스키마 변경 금지 (§5)
- 측정 배치 중 cargo build/test 병행 금지 (CPU 경합이 타이밍 판정을 오염)
- 커밋은 conventional commits (제목 한국어 가능)
- 측정 배치는 20~60분 소요 — Bash 기본 타임아웃(10분)을 넘으므로 **반드시 백그라운드로 실행**하고 완료를 기다린 뒤 다음 단계 진행. 같은 이유로 배치와 코드 작업을 한 태스크에 섞지 않는다
- `./.loco/config.toml`은 git-ignored 로컬 파일 — 이 플랜의 측정 조건 변경은 커밋 대상이 아니다

---

### Task 1: 사전 게이트 + 측정 준비 (1단은 스캐폴딩 **없는** 현재 코드로)

**Files:** 변경 없음 (검증만)

**Interfaces:**
- Consumes: main HEAD (M9 스펙 커밋 포함, 스캐폴딩 코드 없음)
- Produces: 게이트 통과 확인 + 빌드 캐시 (측정 중 재빌드 방지)

- [ ] **Step 1: 워킹트리 클린 확인**

Run: `git status --porcelain`
Expected: 출력 없음 (클린). 더러우면 중단하고 사용자에게 보고.

- [ ] **Step 2: 전체 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 테스트 전부 PASS, clippy 경고 0.

- [ ] **Step 3: 두 tasks 트리 verify 게이트 (§5)**

Run: `cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify`
Expected: 12/12, 3/3 통과, 각 exit 0.

- [ ] **Step 4: 측정 config 확인**

Run: `cat .loco/config.toml`
Expected: `context_tokens = 8192`, `max_output_tokens = 4096`, `command_timeout_secs = 240` (M8 측정 조건 그대로 — §5 공통 조건). 다르면 위 3키만 이 값으로 맞춘다 (git-ignored 파일, 커밋 금지).

---

### Task 2: 1단 배치 A — gemma-4-e4b @ 8K (재베이스라인)

**Files:** 변경 없음 (측정만; 결과는 `.loco/eval/<stamp>/` — git-ignored)

**Interfaces:**
- Consumes: Task 1의 게이트 통과 상태
- Produces: gemma@8K report.json 스탬프 (Task 4의 baselines.md 기록에 사용)

- [ ] **Step 1: 모델 전환 — gemma 로드 (§5: 이전 모델 언로드 필수)**

Run: `lms unload --all && lms load google/gemma-4-e4b --context-length 12288 -y`
Expected: 로드 성공 메시지.

- [ ] **Step 2: 로드 상태 검증**

Run: `curl -s localhost:1234/api/v0/models | python3 -c "import json,sys; ms=[m for m in json.load(sys.stdin)['data'] if m.get('state')=='loaded']; print([(m['id'], m['loaded_context_length']) for m in ms])"`
Expected: `[('google/gemma-4-e4b', 12288)]` — 다른 모델이 남아 있으면 Step 1 재수행.

- [ ] **Step 3: 배치 실행 (백그라운드, ~15-30분)**

Run: `cargo run -- eval tasks-large --repeats 3 --seed 0` (백그라운드 실행, 완료 대기. 병행 빌드 금지)
Expected: exit 0, 표 출력 + `./.loco/eval/<stamp>/report.json` 생성. 스탬프를 기록해 둔다.

- [ ] **Step 4: 결과 요약 추출**

Run: `python3 -c "import json,glob; r=json.load(open(sorted(glob.glob('.loco/eval/*/report.json'))[-1])); print(r['total_pass_rate'], r['passed_count'], r['passed_strict_count'], r['false_finish_count'], r['avg_duration_secs'])"`
Expected: 수치 5개 출력 — Task 4에서 표로 기록.

---

### Task 3: 1단 배치 B — ornith-1.0-9b @ 8K (재베이스라인)

**Files:** 변경 없음

**Interfaces:**
- Consumes: Task 2 완료 (동일 config)
- Produces: ornith@8K report.json 스탬프

- [ ] **Step 1: 모델 전환**

Run: `lms unload --all && lms load ornith-1.0-9b --context-length 12288 -y`
Expected: 로드 성공.

- [ ] **Step 2: 로드 상태 검증**

Task 2 Step 2와 동일한 curl 검증.
Expected: `[('ornith-1.0-9b', 12288)]`

- [ ] **Step 3: 배치 실행 (백그라운드, ~25-50분)**

Run: `cargo run -- eval tasks-large --repeats 3 --seed 0`
Expected: exit 0, report.json 스탬프 기록.

- [ ] **Step 4: 결과 요약 추출** — Task 2 Step 4와 동일 명령.

---

### Task 4: 1단 배치 C — ornith @ 32K + baselines.md 재베이스라인 절 기록

**Files:**
- Modify: `.loco/config.toml` (context_tokens 32768로 임시 변경 → 원복, 커밋 금지)
- Modify: `docs/baselines.md` (M9 재베이스라인 절 추가)

**Interfaces:**
- Consumes: Task 2·3의 스탬프
- Produces: baselines.md "M9 1단 재베이스라인" 절 — Task 10·11의 비교 기준선

- [ ] **Step 1: config 32K 전환**

`.loco/config.toml`의 `context_tokens = 8192`를 `context_tokens = 32768`로 변경 (다른 키 불변).

- [ ] **Step 2: 모델 재로드 (32K 운용 = 로드 49152, §5)**

Run: `lms unload --all && lms load ornith-1.0-9b --context-length 49152 -y`
Expected: 로드 성공. curl 검증: `[('ornith-1.0-9b', 49152)]`

- [ ] **Step 3: 배치 실행 (백그라운드, ~40-70분)**

Run: `cargo run -- eval tasks-large --repeats 3 --seed 0`
Expected: exit 0, 스탬프 기록.

- [ ] **Step 4: config 원복**

`context_tokens = 32768` → `context_tokens = 8192`. `git status --porcelain`으로 추적 파일 변경 없음 확인.

- [ ] **Step 5: baselines.md에 재베이스라인 절 추가**

`docs/baselines.md`의 "M8 8K 베이스라인" 절 뒤에 다음 형식으로 추가 (수치는 Task 2~4 실측):

```markdown
## M9 1단 재베이스라인 (리워드 픽스처, 스캐폴딩 전, 2026-07-17)

리워드된 픽스처(58aab75 이후)로 M8과 동일 조건 재측정 — M8 수치와의 차이 =
리워드(누출 제거) 효과. **이후 M9 2단(스캐폴딩 후) 비교의 기준선.**
하네스 커밋: <git rev-parse --short HEAD 결과>.

| 모델 | 통과 | 엄격 | 거짓 finish | 평균 s/런 | report |
|---|---|---|---|---|---|
| gemma-4-e4b @8K | n/9 | n/9 | n | n.ns | `<stamp A>` |
| ornith-1.0-9b @8K | n/9 | n/9 | n | n.ns | `<stamp B>` |
| ornith-1.0-9b @32K | n/9 | n/9 | n | n.ns | `<stamp C>` |

(M8 대비 관찰 1-2줄: 리워드가 수치를 움직였는지)
```

- [ ] **Step 6: 커밋**

```bash
git add docs/baselines.md
git commit -m "docs: M9 1단 재베이스라인 — 리워드 픽스처 tasks-large 3배치 (스캐폴딩 전)"
```

---

### Task 5: edit_file S/R 오류문 처방 추가 (§3-1)

**Files:**
- Modify: `src/tools/edit_file.rs:313-317` (dry_run의 S/R 거부), 테스트는 같은 파일 `#[cfg(test)]`

**Interfaces:**
- Consumes: 기존 `ToolError::EditFailed` (표시 형식 `edit failed: {0}`, `tools/mod.rs:29-30`)
- Produces: 오류 첫 문장(첫 `.`까지) 불변 + 처방 문장 추가 — Task 6의 `SR_KEY`가 이 첫 문장에 의존

- [ ] **Step 1: 실패하는 테스트 작성**

`src/tools/edit_file.rs` 테스트 모듈(기존 `identical_search_and_replace_is_an_error` 근처)에 추가:

```rust
#[test]
fn identical_error_has_prescription_and_stable_first_sentence() {
    let (_d, ctx) = setup("fn a() {}\n");
    let err = edit(&ctx, "fn a() {}", "fn a() {}").unwrap_err();
    let msg = format!("Error: {err}");
    assert_eq!(
        msg.split('.').next().unwrap(),
        "Error: edit failed: search and replace are identical - no change would be made",
        "스트릭 키(첫 문장)는 불변이어야 한다 (M9 §3-1): {msg}"
    );
    assert!(msg.contains("AFTER your change"), "처방 문장 누락: {msg}");
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test identical_error_has_prescription -- --nocapture`
Expected: FAIL (`AFTER your change` 미포함).

- [ ] **Step 3: 구현**

`src/tools/edit_file.rs:313-317`을 다음으로 교체:

```rust
        if search == replace {
            return Err(ToolError::EditFailed(
                "search and replace are identical - no change would be made. \
                 Put the code as it is NOW in `search`, and the code AFTER your change in `replace`."
                    .to_string(),
            ));
        }
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test --lib tools::edit_file && cargo clippy --all-targets -- -D warnings`
Expected: 전부 PASS (기존 `identical_search_and_replace_is_an_error`는 `contains("identical")`이라 그대로 통과), clippy 0.

- [ ] **Step 5: 커밋**

```bash
git add src/tools/edit_file.rs
git commit -m "feat(tools): edit_file S/R 오류문에 처방 추가 — M9 §3-1"
```

---

### Task 6: repetition.rs — S/R 전용 2연속 교정 + `seen_key` (§3-2, §4-2 신호)

**Files:**
- Modify: `src/agent/repetition.rs`

**Interfaces:**
- Consumes: Task 5의 오류 첫 문장 (`SR_KEY`로 고정)
- Produces:
  - `pub const SR_CORRECTION: &'static str`, `pub const SR_KEY: &'static str`
  - `RepetitionTracker::error_correction(&mut self, tool: &str, body: &str) -> Option<&'static str>` — 기존 시그니처 불변, S/R 분기 추가
  - `RepetitionTracker::seen_key(&self, key: &str) -> bool` — Task 8이 반복-호출 신호로 사용

- [ ] **Step 1: 실패하는 테스트 작성**

`src/agent/repetition.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn sr_error_second_consecutive_gets_dedicated_correction_once() {
        let mut t = RepetitionTracker::new();
        let body = format!("{SR_KEY}. Put the code as it is NOW in `search`, and the code AFTER your change in `replace`.");
        assert!(t.error_correction("edit_file", &body).is_none(), "1회차는 도구 오류문이 담당");
        assert_eq!(t.error_correction("edit_file", &body), Some(SR_CORRECTION), "2연속에 전용 교정 (M9 §3-2)");
        assert!(t.error_correction("edit_file", &body).is_none(), "런당 1회 래치");
        assert!(t.error_correction("edit_file", &body).is_none(), "S/R 스트릭에는 일반 교정도 불발 (전담 배제)");
    }

    #[test]
    fn sr_correction_does_not_consume_generic_latch() {
        let mut t = RepetitionTracker::new();
        let sr = format!("{SR_KEY}. x.");
        t.error_correction("edit_file", &sr);
        assert_eq!(t.error_correction("edit_file", &sr), Some(SR_CORRECTION));
        // 다른 오류 스트릭은 여전히 일반 교정을 받는다 (별도 래치)
        t.error_correction("grep", "Error: x");
        t.error_correction("grep", "Error: x");
        assert_eq!(t.error_correction("grep", "Error: x"), Some(GENERIC_STRATEGY_CORRECTION));
    }

    #[test]
    fn sr_text_via_non_edit_tool_takes_the_generic_path() {
        let mut t = RepetitionTracker::new();
        let sr = format!("{SR_KEY}. x.");
        assert!(t.error_correction("write_file", &sr).is_none());
        assert!(t.error_correction("write_file", &sr).is_none(), "전용 교정은 edit_file 한정 (§3-2 판정)");
        assert_eq!(t.error_correction("write_file", &sr), Some(EDIT_STRATEGY_CORRECTION), "3연속 일반 경로 불변");
    }

    #[test]
    fn sr_streak_resets_on_a_different_intervening_error() {
        let mut t = RepetitionTracker::new();
        let sr = format!("{SR_KEY}. x.");
        assert!(t.error_correction("edit_file", &sr).is_none());
        t.error_correction("edit_file", "Error: edit failed: search block not found. y");
        assert!(t.error_correction("edit_file", &sr).is_none(), "비연속 — 리셋 후 1회차 (스펙 §6)");
        assert_eq!(t.error_correction("edit_file", &sr), Some(SR_CORRECTION), "다시 2연속이면 발동");
    }

    #[test]
    fn sr_key_matches_actual_edit_file_error_first_sentence() {
        // 도구 오류문과 SR_KEY의 드리프트를 고정하는 교차 핀 (M9 §6)
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
        let ctx = crate::tools::ToolCtx::new(dir.path().to_path_buf());
        let err = crate::tools::Tool::run(
            &crate::tools::edit_file::EditFile,
            &serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}),
            &ctx,
        )
        .unwrap_err();
        let body = format!("Error: {err}");
        assert_eq!(body.split('.').next().unwrap(), SR_KEY);
    }

    #[test]
    fn seen_key_is_window_membership_by_key_only() {
        let mut t = RepetitionTracker::new();
        assert!(!t.seen_key("grep|{\"pattern\":\"x\"}"), "record 전에는 자기-매치 없음");
        t.record("grep|{\"pattern\":\"x\"}", "r1");
        assert!(t.seen_key("grep|{\"pattern\":\"x\"}"), "결과 해시가 달라도 키만 일치하면 참");
        assert!(!t.seen_key("grep|{\"pattern\":\"y\"}"));
        for i in 0..8 {
            t.record(&format!("pad|{i}"), "x");
        }
        assert!(!t.seen_key("grep|{\"pattern\":\"x\"}"), "윈도(8) 밖으로 밀려나면 거짓");
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib agent::repetition`
Expected: FAIL — `SR_KEY`/`SR_CORRECTION`/`seen_key` 미정의 컴파일 에러.

- [ ] **Step 3: 구현**

`src/agent/repetition.rs`에 상수 추가 (기존 두 교정 상수 아래):

```rust
/// edit_file S/R 자기-버그 2연속 전용 교정 (M9 §3-2). 모델 대상 — 영어
pub const SR_CORRECTION: &str = "Your `replace` is identical to `search`. Write the MODIFIED code in `replace`. \
If you cannot produce a different `replace`, rewrite the whole file with write_file, applying the fix.";

/// S/R 오류의 스트릭 키(첫 문장) — tools/edit_file.rs의 실제 오류문과
/// sr_key_matches_actual_edit_file_error_first_sentence 테스트로 고정 (M9 §3-2)
pub const SR_KEY: &str = "Error: edit failed: search and replace are identical - no change would be made";
```

`RepetitionTracker`에 필드 `sr_corrected: bool` 추가 (`new()`에서 `false`), `error_correction`을 다음으로 교체:

```rust
    pub fn error_correction(&mut self, tool: &str, body: &str) -> Option<&'static str> {
        if !body.starts_with("Error:") {
            self.last_error_key = None;
            self.error_streak = 0;
            return None;
        }
        // 동일성 키 = 첫 문장(첫 '.'까지). 개선된 에러들은 첫 줄 안에 가변 내용을
        // 붙이므로(스키마 에코의 키 목록, not-found의 `lines A-B`) 첫 줄 비교는 무력
        let key = body.split('.').next().unwrap_or(body).to_string();
        if self.last_error_key.as_deref() == Some(key.as_str()) {
            self.error_streak += 1;
        } else {
            self.last_error_key = Some(key);
            self.error_streak = 1;
        }
        // S/R 키 스트릭은 전용 교정이 전담 — 2연속(도구 오류문이 1회차 처방을 이미
        // 줬으므로) 발동, 일반 교정은 배제 (M9 §3-2)
        if tool == "edit_file" && self.last_error_key.as_deref() == Some(SR_KEY) {
            if self.error_streak >= 2 && !self.sr_corrected {
                self.sr_corrected = true;
                return Some(SR_CORRECTION);
            }
            return None;
        }
        if self.error_streak >= 3 && !self.error_corrected {
            self.error_corrected = true;
            return Some(if matches!(tool, "edit_file" | "write_file") {
                EDIT_STRATEGY_CORRECTION
            } else {
                GENERIC_STRATEGY_CORRECTION
            });
        }
        None
    }

    /// 윈도에 같은 (도구|인자) 키가 이미 있는가 — FINISH_NUDGE의 반복-호출 신호
    /// (M9 §4-2). 결과 해시는 무시하고 키만 본다. record() **전에** 조회해야
    /// 자기-매치가 없다.
    pub fn seen_key(&self, key: &str) -> bool {
        self.window.iter().any(|(k, _)| k == key)
    }
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test --lib agent::repetition && cargo clippy --all-targets -- -D warnings`
Expected: 전부 PASS (기존 `same_error_first_sentence_three_times_...` 포함 — "search block not found" 키는 SR_KEY와 다르므로 일반 경로 불변), clippy 0.

- [ ] **Step 5: 커밋**

```bash
git add src/agent/repetition.rs
git commit -m "feat(agent): S/R 전용 2연속 스트릭 교정(전담 배제)·seen_key 추가 — M9 §3-2"
```

---

### Task 7: `agent/finish_nudge.rs` — 검증완료 후 finish 유도 상태기계 (§4-2)

**Files:**
- Create: `src/agent/finish_nudge.rs`
- Modify: `src/agent/mod.rs:1-5` (모듈 선언 1줄)

**Interfaces:**
- Consumes: 없음 (독립 상태기계 — 이벤트 분류는 Task 8이 담당)
- Produces:
  - `pub const FINISH_NUDGE: &'static str`
  - `pub enum TurnEvent { MutationOk, MutationAttempt, VerifyOk { repeat: bool }, VerifyOther, ReadOnly { repeat: bool }, FinishAttempt, Other }`
  - `pub struct FinishNudge` — `new()`, `on_turn(&mut self, ev: TurnEvent) -> Option<&'static str>`

- [ ] **Step 1: 모듈 선언**

`src/agent/mod.rs`의 `pub mod repetition;` 아래에 `pub mod finish_nudge;` 추가.

- [ ] **Step 2: 실패하는 테스트를 포함한 모듈 뼈대 작성**

`src/agent/finish_nudge.rs` 신규 — 아래 전체 코드에서 `on_turn` 본문을 `None`만 반환하게 두고 테스트가 실패하는 것부터 확인해도 좋고, 테스트→구현 순서만 지키면 한 파일이므로 아래 최종 코드로 바로 가도 된다. 최종 코드:

```rust
//! 검증완료 후 finish 유도 상태기계 (M9 §4-2). 목표 패턴: "이미 확인한 사실을
//! 문자 그대로 재확인하는 루프" — 순수 동일-명령 루프는 순환 교정·반복정지가
//! 전담하고(우선순위는 agent 루프가 보장), 이 기계는 이종/혼합 재검증을 겨냥한다.

use std::collections::VecDeque;

/// 발동 시 1회 주입 (M9 §4-2). 모델 대상 — 영어
pub const FINISH_NUDGE: &str = "You already ran a successful verification. If the task is complete, \
call finish with a summary now; do not re-verify what you have already confirmed.";

/// 발동에 필요한 연속 카운트 턴 수 (§4-2: K=4)
const IDLE_WINDOW: usize = 4;

/// run() 루프가 턴마다 분류해 넘기는 이벤트 (M9 §4-2 전이 표와 1:1)
pub enum TurnEvent {
    /// edit_file/write_file 성공 디스패치 ("뮤테이션"의 정의 — is_mutating()과 다름)
    MutationOk,
    /// edit_file/write_file 실패 시도 (오류·게이트 거부 포함)
    MutationAttempt,
    /// run_command Ok ∧ 본문 첫 줄 `exit code: 0`. repeat = 반복-호출 여부
    VerifyOk { repeat: bool },
    /// run_command 그 외 (비0 종료코드·타임아웃·취소·Err)
    VerifyOther,
    /// read_file/grep/list_files. repeat = 반복-호출 여부
    ReadOnly { repeat: bool },
    /// finish 시도 (유효·무효 무관 — 무효 finish 교정은 §4-1이 전담)
    FinishAttempt,
    /// 그 외 (미지 도구, 게이트 거부된 run_command) — 상태 불변
    Other,
}

pub struct FinishNudge {
    mutated: bool,
    armed: bool,
    /// 카운트된 최근 IDLE_WINDOW턴의 반복-호출 여부 (§4-2 발동 조건)
    idle: VecDeque<bool>,
    latched: bool,
}

impl FinishNudge {
    pub fn new() -> Self {
        Self { mutated: false, armed: false, idle: VecDeque::with_capacity(IDLE_WINDOW), latched: false }
    }

    /// 이벤트를 반영하고, 발동 조건이 차면 FINISH_NUDGE를 1회 반환 (런당 래치)
    pub fn on_turn(&mut self, ev: TurnEvent) -> Option<&'static str> {
        match ev {
            TurnEvent::MutationOk => {
                self.mutated = true;
                self.disarm();
            }
            TurnEvent::MutationAttempt => self.disarm(),
            TurnEvent::VerifyOk { repeat } => {
                if self.armed {
                    // 재검증도 카운트 — 매 검증마다 리셋하면 run_command 재검증
                    // 루프에 영원히 발동하지 않는다 (§4-2 표 3행)
                    self.count(repeat);
                } else if self.mutated {
                    self.armed = true;
                    self.idle.clear();
                }
            }
            TurnEvent::VerifyOther => self.disarm(),
            TurnEvent::ReadOnly { repeat } => {
                if self.armed {
                    self.count(repeat);
                }
            }
            TurnEvent::FinishAttempt => self.idle.clear(),
            TurnEvent::Other => {}
        }
        if self.armed && self.idle.len() >= IDLE_WINDOW && self.idle.iter().any(|r| *r) && !self.latched {
            self.latched = true;
            return Some(FINISH_NUDGE);
        }
        None
    }

    fn count(&mut self, repeat: bool) {
        if self.idle.len() == IDLE_WINDOW {
            self.idle.pop_front();
        }
        self.idle.push_back(repeat);
    }

    fn disarm(&mut self) {
        self.armed = false;
        self.idle.clear();
    }
}

impl Default for FinishNudge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 뮤테이션 성공 + exit-0 검증까지 마친(무장된) 기계
    fn armed_machine() -> FinishNudge {
        let mut n = FinishNudge::new();
        assert!(n.on_turn(TurnEvent::MutationOk).is_none());
        assert!(n.on_turn(TurnEvent::VerifyOk { repeat: false }).is_none(), "무장 턴 자체는 카운트 없음");
        n
    }

    #[test]
    fn fires_on_fourth_counted_turn_with_a_repeat_then_latches() {
        let mut n = armed_machine();
        assert!(n.on_turn(TurnEvent::ReadOnly { repeat: false }).is_none());
        assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none());
        assert!(n.on_turn(TurnEvent::ReadOnly { repeat: false }).is_none());
        assert_eq!(n.on_turn(TurnEvent::ReadOnly { repeat: false }), Some(FINISH_NUDGE));
        assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none(), "런당 1회 래치");
    }

    #[test]
    fn four_novel_turns_do_not_fire_until_a_repeat_enters_the_window() {
        let mut n = armed_machine();
        for _ in 0..4 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: false }).is_none(), "신규 탐색만으로는 불발 (§4-2)");
        }
        assert_eq!(n.on_turn(TurnEvent::ReadOnly { repeat: true }), Some(FINISH_NUDGE), "반복이 창에 들어오면 발동");
    }

    #[test]
    fn armed_verify_ok_counts_toward_idle() {
        let mut n = armed_machine();
        for _ in 0..3 {
            assert!(n.on_turn(TurnEvent::VerifyOk { repeat: true }).is_none());
        }
        assert_eq!(n.on_turn(TurnEvent::VerifyOk { repeat: true }), Some(FINISH_NUDGE), "run_command 재검증도 카운트");
    }

    #[test]
    fn mutation_attempt_disarms_and_a_later_verify_rearms() {
        let mut n = armed_machine();
        n.on_turn(TurnEvent::ReadOnly { repeat: true });
        n.on_turn(TurnEvent::MutationAttempt); // S/R 루프 등 — 무장 해제 (§4-2 표 2행)
        for _ in 0..4 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none(), "비무장은 카운트 없음");
        }
        assert!(n.on_turn(TurnEvent::VerifyOk { repeat: false }).is_none()); // 재무장
        for _ in 0..3 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none());
        }
        assert_eq!(n.on_turn(TurnEvent::ReadOnly { repeat: true }), Some(FINISH_NUDGE));
    }

    #[test]
    fn verify_without_prior_mutation_does_not_arm() {
        let mut n = FinishNudge::new();
        n.on_turn(TurnEvent::VerifyOk { repeat: false });
        for _ in 0..5 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none(), "뮤테이션 없는 런은 무장 안 함");
        }
    }

    #[test]
    fn failed_or_timed_out_verify_disarms() {
        let mut n = armed_machine();
        n.on_turn(TurnEvent::VerifyOther);
        for _ in 0..5 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none(), "실패한 검증 뒤 finish 유도는 역효과 (§4-2)");
        }
    }

    #[test]
    fn finish_attempt_resets_idle_but_keeps_armed() {
        let mut n = armed_machine();
        for _ in 0..3 {
            n.on_turn(TurnEvent::ReadOnly { repeat: true });
        }
        n.on_turn(TurnEvent::FinishAttempt);
        for _ in 0..3 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none(), "리셋 후 다시 4턴 필요");
        }
        assert_eq!(n.on_turn(TurnEvent::ReadOnly { repeat: true }), Some(FINISH_NUDGE), "armed는 유지");
    }

    #[test]
    fn other_turns_leave_state_unchanged() {
        let mut n = armed_machine();
        for _ in 0..3 {
            n.on_turn(TurnEvent::ReadOnly { repeat: true });
        }
        n.on_turn(TurnEvent::Other); // 미지 도구·게이트 거부 run_command (§4-2 표 8행)
        assert_eq!(
            n.on_turn(TurnEvent::ReadOnly { repeat: false }),
            Some(FINISH_NUDGE),
            "Other가 카운터를 건드리지 않았으므로 4번째 카운트 턴에 발동"
        );
    }
}
```

- [ ] **Step 3: 통과 확인**

Run: `cargo test --lib agent::finish_nudge && cargo clippy --all-targets -- -D warnings`
Expected: 9개 테스트 전부 PASS, clippy 0.

- [ ] **Step 4: 커밋**

```bash
git add src/agent/finish_nudge.rs src/agent/mod.rs
git commit -m "feat(agent): finish_nudge 상태기계 — 검증완료 후 반복 재확인 감지 (M9 §4-2)"
```

---

### Task 8: agent 루프 배선 — finish 인자누락 교정 + FINISH_NUDGE (§4-1, §4-2)

**Files:**
- Modify: `src/agent/mod.rs` (상수 1개, `run()` 배선, 테스트 8개)

**Interfaces:**
- Consumes: Task 6 `seen_key`/`SR_CORRECTION`(자동 — error_correction 경유), Task 7 `FinishNudge`/`TurnEvent`/`FINISH_NUDGE`
- Produces: `pub const FINISH_ARGS_CORRECTION: &'static str`; run() 동작 변경 (외부 시그니처 불변)

- [ ] **Step 1: 상수 추가**

`src/agent/mod.rs`의 `VERIFY_NUDGE` 아래에:

```rust
/// summary 없는 finish 2연속 시 1회 주입 (M9 §4-1) — 모델이 내보내야 하는
/// 전체 턴 형태를 제시한다 (인자 예시만 담은 FINISH_ERR 에코는 5연속 반복을
/// 못 막은 실측이 있다). 모델 대상 — 영어
pub const FINISH_ARGS_CORRECTION: &str = "Your finish call is missing `summary`. Respond with exactly this shape: \
{\"thought\": \"...\", \"action\": {\"tool\": \"finish\", \"args\": {\"summary\": \"<your final answer>\"}}}. \
Do not call finish with empty args again.";
```

- [ ] **Step 2: 실패하는 테스트 작성**

`src/agent/mod.rs` 테스트 모듈에 헬퍼와 테스트 추가. 헬퍼 (`make_agent` 옆):

```rust
    fn make_guided_agent(script: &Scripted, root: std::path::PathBuf, max_turns: usize) -> Agent<&Scripted> {
        let config = Config { max_turns, ..Default::default() };
        Agent::new(script, Registry::guided(), ToolCtx::new(root), "test-model".into(), &config)
    }

    fn session_contains(session: &Session, needle: &str) -> bool {
        session.messages().iter().any(|m| m.content.contains(needle))
    }
```

테스트 (§6의 ①·⑥·⑦ — 셸 불필요, cfg 게이트 없음):

```rust
    #[tokio::test]
    async fn finish_missing_summary_twice_gets_args_correction_once() {
        let dir = tempfile::tempdir().unwrap();
        let empty = turn("finish", serde_json::json!({}));
        let script = Scripted::new(vec![ok(&empty), ok(&empty), ok(&finish("done"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)));
        let hits = session
            .messages()
            .iter()
            .filter(|m| m.content.contains("Do not call finish with empty args again"))
            .count();
        assert_eq!(hits, 1, "2연속에 정확히 1회 주입 (M9 §4-1)");
    }

    #[tokio::test]
    async fn dispatched_action_resets_finish_args_streak() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let empty = turn("finish", serde_json::json!({}));
        let read = turn("read_file", serde_json::json!({"path": "a.txt"}));
        let script = Scripted::new(vec![ok(&empty), ok(&read), ok(&empty), ok(&finish("done"))]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(!session_contains(&session, "Do not call finish with empty args again"), "사이에 디스패치된 액션 → 리셋 (§4-1)");
    }

    #[tokio::test]
    async fn length_cut_between_missing_finishes_keeps_the_streak() {
        let dir = tempfile::tempdir().unwrap();
        let empty = turn("finish", serde_json::json!({}));
        let script = Scripted::new(vec![
            ok(&empty),
            ok_with_reason("truncated...", "length"), // 무액션 턴 — 스트릭 유지 (§4-1)
            ok(&empty),
            ok(&finish("done")),
        ]);
        let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(session_contains(&session, "Do not call finish with empty args again"));
    }
```

테스트 (§6의 ②③④⑤⑧ — run_command가 셸을 쓰므로 `#[cfg(unix)]` 모듈로 감싼다, eval 테스트 관례):

```rust
    #[cfg(unix)]
    mod finish_nudge_loop {
        use super::*;

        fn write_turn(path: &str, content: &str) -> String {
            turn("write_file", serde_json::json!({"path": path, "content": content}))
        }
        fn run_turn(cmd: &str) -> String {
            turn("run_command", serde_json::json!({"command": cmd}))
        }
        fn read_turn(path: &str) -> String {
            turn("read_file", serde_json::json!({"path": path}))
        }
        fn grep_turn(pattern: &str) -> String {
            turn("grep", serde_json::json!({"pattern": pattern}))
        }

        #[tokio::test]
        async fn verified_then_repeated_rechecks_get_finish_nudge() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("true")), // exit 0 — 무장
                ok(&read_turn("a.txt")),
                ok(&grep_turn("answer")),
                ok(&read_turn("a.txt")), // 반복 호출
                ok(&turn("list_files", serde_json::json!({}))), // 4번째 카운트 턴 — 발동
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(matches!(outcome, AgentOutcome::Finished(_)));
            assert!(session_contains(&session, "do not re-verify"), "§4-2 발동");
        }

        #[tokio::test]
        async fn novel_exploration_after_verify_does_not_fire_nudge() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("true")),
                ok(&read_turn("a.txt")),
                ok(&grep_turn("p1")),
                ok(&grep_turn("p2")),
                ok(&turn("list_files", serde_json::json!({}))), // 4턴 전부 신규 — 불발
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(!session_contains(&session, "do not re-verify"), "반복-호출 조건 (§4-2)");
        }

        #[tokio::test]
        async fn edit_attempt_after_verify_disarms_nudge() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("true")),
                ok(&read_turn("a.txt")),
                ok(&read_turn("a.txt")), // 반복 — idle 2
                ok(&turn("edit_file", serde_json::json!({"path": "a.txt", "search": "answer", "replace": "answer"}))), // S/R 실패 시도 — 무장 해제
                ok(&grep_turn("x")),
                ok(&grep_turn("y")),
                ok(&turn("list_files", serde_json::json!({}))),
                ok(&grep_turn("z")),
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(!session_contains(&session, "do not re-verify"), "무장 해제 후 재검증 성공 없이는 불발 (§4-2 표 2행)");
        }

        #[tokio::test]
        async fn failing_verification_does_not_arm_nudge() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("false")), // exit 1 — 무장 안 함
                ok(&read_turn("a.txt")),
                ok(&grep_turn("x")),
                ok(&read_turn("a.txt")),
                ok(&turn("list_files", serde_json::json!({}))),
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(!session_contains(&session, "do not re-verify"), "비0 종료코드는 무장하지 않음 (§4-2 표 4행)");
        }

        #[tokio::test]
        async fn invalid_finish_resets_nudge_idle_counter() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("true")),
                ok(&read_turn("a.txt")),
                ok(&read_turn("a.txt")), // 반복 — idle 2
                ok(&grep_turn("x")),     // idle 3
                ok(&turn("finish", serde_json::json!({}))), // 무효 finish — idle 리셋 (§4-2 표 6행)
                ok(&turn("list_files", serde_json::json!({}))), // 리셋이 없었다면 4번째 카운트 턴으로 발동했을 자리
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(!session_contains(&session, "do not re-verify"), "무효 finish가 4-2 카운터를 리셋 (§6 ⑥)");
        }

        #[tokio::test]
        async fn no_action_turn_preserves_nudge_idle_counter() {
            let dir = tempfile::tempdir().unwrap();
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&run_turn("true")),
                ok(&read_turn("a.txt")),
                ok(&read_turn("a.txt")), // 반복 — idle 2
                ok(&grep_turn("x")),     // idle 3
                ok_with_reason("truncated...", "length"), // 무액션 턴 — 카운터 불변 (§4-2 표 7행)
                ok(&turn("list_files", serde_json::json!({}))), // idle 4 — 발동
                ok(&finish("done")),
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(session_contains(&session, "do not re-verify"), "무액션 턴은 카운터 불변 (§6 ⑦)");
        }

        #[tokio::test]
        async fn pure_identical_loop_prefers_repetition_stop() {
            let dir = tempfile::tempdir().unwrap();
            let echo = run_turn("echo hi");
            let script = Scripted::new(vec![
                ok(&write_turn("a.txt", "answer")),
                ok(&echo), // 무장 (윈도 1회째)
                ok(&echo), // idle 1 (2회째)
                ok(&echo), // idle 2 (3회째 — REPEAT_CORRECTION)
                ok(&echo), // idle 3 (4회째)
                ok(&echo), // 5회째 — RepetitionStop (idle 4 도달 전에 정지가 선점, §4-2 우선순위)
            ]);
            let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
            let mut session = new_session(&agent);
            let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
            assert!(matches!(outcome, AgentOutcome::RepetitionStop), "{outcome:?}");
            assert!(!session_contains(&session, "do not re-verify"), "정지 턴에는 니지를 평가하지 않는다");
        }
    }
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test --lib agent::tests 2>&1 | tail -20`
Expected: 신규 테스트 FAIL/컴파일 에러 (`FINISH_ARGS_CORRECTION` 미정의 등).

- [ ] **Step 4: run() 배선 구현**

(a) `run()` 지역 상태 — `let mut verify_nudged = false;` (mod.rs:157) 아래에:

```rust
        let mut finish_nudge = finish_nudge::FinishNudge::new();
        // summary 없는 finish 연속 카운트 (M9 §4-1) — 무액션 턴은 유지, 디스패치·거부된
        // 다른 액션이 리셋
        let mut finish_missing_streak: usize = 0;
        let mut finish_args_corrected = false;
```

(b) 노트 병합 헬퍼 — `looks_like_context_overflow` 근처 자유 함수로:

```rust
/// 교정 노트에 문장 하나를 덧붙인다 (없으면 새로) — tool_result 병합 규칙 유지
fn merge_note(note: Option<String>, extra: &str) -> Option<String> {
    Some(match note {
        Some(n) => format!("{n}\n{extra}"),
        None => extra.to_string(),
    })
}
```

(c) finish 유효 경로의 VERIFY_NUDGE 반려 분기(mod.rs:241-247) — `turns += 1;` 앞에:

```rust
                            finish_missing_streak = 0;
                            let _ = finish_nudge.on_turn(finish_nudge::TurnEvent::FinishAttempt); // idle만 리셋 — 발동 불가
```

(d) finish `None` 분기(mod.rs:250-271) 전체를 다음으로 교체:

```rust
                    None => {
                        const FINISH_ERR: &str = "Error: finish requires a string `summary` argument, e.g. {\"tool\": \"finish\", \"args\": {\"summary\": \"<your final answer>\"}}";
                        // summary 없는 finish도 반복 계수에 편입 (M5 §7.3 — 기존 §3 사각지대 폐지)
                        finish_missing_streak += 1;
                        // idle만 리셋 — 이 이벤트로는 발동 불가 (M9 §4-2 표 6행)
                        let _ = finish_nudge.on_turn(finish_nudge::TurnEvent::FinishAttempt);
                        let key = format!("finish|{}", turn.action.args);
                        let verdict = tracker.record(&key, FINISH_ERR);
                        // InjectCorrection을 버리면 record()가 래치한 실행당 1회 교정 기회가
                        // 소모된다 — 같은 user 메시지에 병합해 반드시 전달 (본선 스펙 §3 연속 user 금지)
                        let mut body = match verdict {
                            repetition::RepetitionVerdict::InjectCorrection => {
                                on_event(AgentEvent::Notice("(반복 감지 — 교정 메시지 주입)".to_string()));
                                format!("{FINISH_ERR}\n{REPEAT_CORRECTION}")
                            }
                            _ => FINISH_ERR.to_string(),
                        };
                        if finish_missing_streak >= 2 && !finish_args_corrected {
                            finish_args_corrected = true;
                            on_event(AgentEvent::Notice("(finish 인자 누락 반복 — 교정 주입)".to_string()));
                            body = format!("{body}\n{FINISH_ARGS_CORRECTION}");
                        }
                        session.push(tool_result_message("finish", &body));
                        if matches!(verdict, repetition::RepetitionVerdict::Stop) {
                            on_event(AgentEvent::Notice("(같은 툴 호출이 반복돼 조기 종료합니다)".to_string()));
                            return Ok(AgentOutcome::RepetitionStop);
                        }
                        turns += 1;
                        continue;
                    }
```

(e) 게이트 거부 분기(mod.rs:289-299) — `let body = format!("Denied: {reason}");` 아래에:

```rust
                    finish_missing_streak = 0;
                    let ev = match turn.action.tool.as_str() {
                        "edit_file" | "write_file" => finish_nudge::TurnEvent::MutationAttempt,
                        _ => finish_nudge::TurnEvent::Other, // 게이트 거부된 run_command — 불변 (§4-2 표)
                    };
                    let (mut note, stop) = self.track_and_note(&mut tracker, &turn, &body, on_event);
                    if !stop {
                        if let Some(nudge) = finish_nudge.on_turn(ev) {
                            on_event(AgentEvent::Notice("(검증 완료 후 재확인 반복 — finish 유도 주입)".to_string()));
                            note = merge_note(note, nudge);
                        }
                    }
                    session.push_tool_result(&turn.action.tool, &turn.action.args, &body, note.as_deref());
```

(기존 track_and_note/push 두 줄을 위 블록으로 대체 — stop 반환·turns 증가는 기존 그대로.)

(f) 본 디스패치 경로(mod.rs:304-329) — dispatch 전, `let registry = ...` 위에:

```rust
            // M9 §4-2: 반복-호출 신호는 tracker.record()(track_and_note 내부) **전에**
            // 조회해야 자기-매치가 없다
            let call_key = format!("{}|{}", turn.action.tool, turn.action.args);
            let repeated_call = tracker.seen_key(&call_key);
```

`mutated_since_verify` 갱신 블록(317-323) 뒤, 기존 track_and_note/push/stop 3줄을 다음으로 대체:

```rust
            finish_missing_streak = 0;
            let ev = match turn.action.tool.as_str() {
                "edit_file" | "write_file" => {
                    if dispatch_ok {
                        finish_nudge::TurnEvent::MutationOk
                    } else {
                        finish_nudge::TurnEvent::MutationAttempt
                    }
                }
                // §4-2: "성공 검증" = Ok ∧ 첫 줄 exit code 0. 타임아웃·취소·Err 본문에는
                // 이 줄이 없어 자연 배제 (VERIFY_NUDGE의 "종료코드 무관 Ok" 기준과 별개)
                "run_command" => {
                    if dispatch_ok && body.lines().next() == Some("exit code: 0") {
                        finish_nudge::TurnEvent::VerifyOk { repeat: repeated_call }
                    } else {
                        finish_nudge::TurnEvent::VerifyOther
                    }
                }
                "read_file" | "grep" | "list_files" => finish_nudge::TurnEvent::ReadOnly { repeat: repeated_call },
                _ => finish_nudge::TurnEvent::Other, // 미지 도구 (§4-2 표)
            };
            let (mut note, stop) = self.track_and_note(&mut tracker, &turn, &body, on_event);
            if !stop {
                // 반복정지 우선 (§4-2) — 정지 턴에는 니지를 평가하지 않는다
                if let Some(nudge) = finish_nudge.on_turn(ev) {
                    on_event(AgentEvent::Notice("(검증 완료 후 재확인 반복 — finish 유도 주입)".to_string()));
                    note = merge_note(note, nudge);
                }
            }
            session.push_tool_result(&turn.action.tool, &turn.action.args, &body, note.as_deref());
            if stop {
                return Ok(AgentOutcome::RepetitionStop);
            }
```

- [ ] **Step 5: 통과 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전체 PASS (기존 `finish_without_summary_gets_feedback`는 1회 누락이라 교정 없이 그대로 통과), clippy 0.

- [ ] **Step 6: 커밋**

```bash
git add src/agent/mod.rs
git commit -m "feat(agent): finish 규율 배선 — 인자누락 2연속 교정 + FINISH_NUDGE (M9 §4)"
```

---

### Task 9: CLAUDE.md 아키텍처 반영 + 전체 게이트

**Files:**
- Modify: `CLAUDE.md` (agent 불릿 — 영문 유지)

**Interfaces:**
- Consumes: Task 5~8의 최종 동작
- Produces: 문서화된 스캐폴딩 동작 (측정 수치는 Task 11에서)

- [ ] **Step 1: CLAUDE.md agent 불릿 갱신**

`CLAUDE.md`의 agent 항목에서 반복 감지/전략 교정 문장(`Both corrections latch once per run` 근처) 뒤에 다음 텍스트를 삽입 (영문 그대로):

```
M9 scaffolding: edit_file's identical-search/replace rejection now appends a prescription sentence (current code in `search`, changed code in `replace`); a dedicated 2-consecutive S/R streak correction (`SR_CORRECTION`, own latch, `agent/repetition.rs`) handles that error key exclusively — the generic 3-streak correction is excluded for it, while the window-based `REPEAT_CORRECTION` still stacks at the 3rd identical call (intended escalation). finish discipline: a missing-summary finish 2-consecutive streak injects `FINISH_ARGS_CORRECTION` once per run (no-action turns preserve the streak; dispatched or gate-denied actions reset it); `agent/finish_nudge.rs` is an event-driven state machine — "mutation" = successful edit_file/write_file dispatch, arms on a post-mutation `exit code: 0` run_command, counts non-mutating turns in a K=4 window requiring ≥1 repeated call (`RepetitionTracker::seen_key`, queried before record), injects `FINISH_NUDGE` once per run; RepetitionStop takes priority on the same turn.
```

- [ ] **Step 2: 전체 게이트 + verify**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify`
Expected: 전부 통과 (12/12, 3/3).

- [ ] **Step 3: 커밋**

```bash
git add CLAUDE.md
git commit -m "docs: CLAUDE.md M9 스캐폴딩 반영 — S/R 전용 교정·finish 규율"
```

---

### Task 10: 2단 측정 — 스캐폴딩 후 3배치 + tasks/ 스포트

**Files:**
- Modify: `.loco/config.toml` (배치별 임시 변경 → 원복, 커밋 금지)

**Interfaces:**
- Consumes: Task 9까지의 스캐폴딩 코드 (HEAD), Task 4의 기준선
- Produces: 2단 report.json 스탬프 4개 — Task 11의 판정 입력

- [ ] **Step 1: 사전 빌드 + 게이트**

Run: `cargo build && cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify`
Expected: 통과. 이후 배치 중 빌드 금지.

- [ ] **Step 2: 배치 D — gemma@8K.** config가 8K 조건(context 8192·output 4096·timeout 240)인지 확인 후, Task 2 Step 1~4 그대로 반복 (lms gemma 12288 로드 → curl 검증 → `cargo run -- eval tasks-large --repeats 3 --seed 0` 백그라운드 → 요약 추출).

- [ ] **Step 3: 배치 E — ornith@8K.** Task 3 그대로 반복.

- [ ] **Step 4: 배치 F — ornith@32K.** Task 4 Step 1~4 그대로 반복 (config 32768 ↔ 원복 포함).

- [ ] **Step 5: 배치 G — tasks/ 스포트, ornith@8K, v2 조건 (§5)**

1. `.loco/config.toml`에서 `command_timeout_secs = 240` 줄을 삭제(기본 60 적용), `context_tokens = 8192`·`max_output_tokens = 4096` 유지 — v2 기준선 조건 재현.
2. `lms unload --all && lms load ornith-1.0-9b --context-length 8192 -y` + curl 검증 (`8192`).
3. `cargo run -- eval tasks --repeats 3 --seed 0` (백그라운드, ~45-75분).
4. 요약 추출 후 `command_timeout_secs = 240` 줄 복원.

Expected: report.json 4개 스탬프 확보. 스포트 통과 34/36±1 (33 미만이면 회귀 — Task 11에서 판정하되 즉시 사용자에게 보고).

---

### Task 11: 행동 지표 추출 + 문서 갱신 + 성공 기준 판정

**Files:**
- Create: 스크래치 스크립트 (커밋하지 않음 — 레시피만 baselines.md에)
- Modify: `docs/baselines.md`, `README.md`, `CLAUDE.md`

**Interfaces:**
- Consumes: 1단(Task 2-4)·2단(Task 10) 스탬프 7개
- Produces: M9 종결 문서 + 스펙 §2 성공 기준 판정

- [ ] **Step 1: 행동 지표 추출 스크립트**

스크래치패드에 `m9_metrics.py`로 저장 후 각 배치 디렉토리에 실행:

```python
import json, sys, glob, os
# usage: python3 m9_metrics.py .loco/eval/<stamp>
# 트랜스크립트 kind: system/user/assistant/tool_result (M8 실패분석 §0)
MARKS = {
    "sr_error": "search and replace are identical",
    "sr_correction": "Write the MODIFIED code in `replace`",
    "finish_missing": "finish requires a string `summary`",
    "finish_args_corr": "Do not call finish with empty args again",
    "finish_nudge": "do not re-verify what you have already confirmed",
    "repeat_corr": "repeating the same tool call",
}
for path in sorted(glob.glob(os.path.join(sys.argv[1], "run-*.jsonl"))):
    counts = dict.fromkeys(MARKS, 0)
    with open(path) as f:
        for line in f:
            e = json.loads(line)
            if e.get("kind") == "assistant":
                continue  # 모델이 인용한 문구는 세지 않는다
            c = e.get("content", "") or ""
            for k, m in MARKS.items():
                counts[k] += c.count(m)
    print(os.path.basename(path), counts)
```

Expected: 런별 카운트 표. 여기에 더해 §5 지표 ②(교정 주입 후 다음 뮤테이션 성공까지 턴 수)·④(마지막 검증 성공→finish 간 턴 간격)는 해당 런의 트랜스크립트를 직접 정독해 기록한다(발생 런만 — M5 이후 관례).

- [ ] **Step 2: baselines.md M9 절 완성**

Task 4의 재베이스라인 절 뒤에 "M9 2단 (스캐폴딩 후)" 절 추가: 4배치 표(1단과 같은 컬럼 + 스포트) + 행동 지표 비교 표(§2 성공 기준 ①②의 각 항목: S/R발 반복정지, 2회 이내 회복률, finish 인자누락발 반복정지, 검증완료 후 finish 도달률 — 1단 대비. 발생 런이 배치당 3런 미만이면 §2의 소표본 규칙대로 전수 나열+방향성) + Step 1 스크립트를 추출 레시피로 수록.

- [ ] **Step 3: 성공 기준 판정 (스펙 §2 전건 대조)**

체크리스트로 명시 기록:
1. 게이트 4종 통과 여부
2. 행동 지표 ①② (1단에서 미발생이면 "비악화 + 단위 테스트 게이트"로 대체 판정)
3. tasks-large 엄격 1단 대비 비악화 / tasks/ 스포트 33/36 이상
4. 신규 모델-대면 텍스트 영문 (코드 리뷰로 확인)

판정 결과(충족/미충족 각 항목)를 baselines.md M9 절 말미에 기록. **미충족 항목이 있으면 커밋은 하되 사용자에게 보고하고 다음 단계(원인 분석 vs 스펙 §7 리스크 대응) 지시를 기다린다.**

- [ ] **Step 4: README·CLAUDE.md 수치 갱신**

- README: M8 수치 문장 옆에 M9 결과 1-2줄 (기존 M8 반영 문체 따름)
- CLAUDE.md 헤더: "M1-M8 done" → "M1-M9 done" + tasks-large 문단의 측정 수치에 M9 2단 결과 추가 (영문)

- [ ] **Step 5: 커밋**

```bash
git add docs/baselines.md README.md CLAUDE.md
git commit -m "docs: M9 2단 측정 결과 — 행동 지표 판정·기준선 갱신"
```

---

## 실행 순서 요약

Task 1→2→3→4 (1단 측정, **스캐폴딩 코드 착수 전**) → 5→6→7→8→9 (구현) → 10 (2단 측정) → 11 (판정·문서). 측정 태스크(2·3·4·10)는 LM Studio 상태를 바꾸므로 병렬 실행 금지 — 전부 직렬.
