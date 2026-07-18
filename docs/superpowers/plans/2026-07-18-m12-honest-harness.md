# M12 정직한 하네스 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 하네스가 모델에게 내보내는 검증 신호를 exit code가 아니라 실제 테스트 실질(몇 개 돌았고 몇 개 실패했는지)에 접지시키고, 이미 효과가 입증된 개입 메커니즘(S/R 교정·온도 섭동)의 발화 사각을 없앤다.

**Architecture:** libtest 요약을 파싱하는 순수 모듈(`src/test_summary.rs`)을 신설하고, run() 배선 지점에서 **1회 파싱**해 그 결과를 세 소비처(run_command 노트는 도구 층에서 자체 파싱, 상태선 verification 렌더, FINISH_NUDGE/VERIFY_NUDGE 술어)로 흘린다. 파서는 요약 줄이 없으면 `None`을 반환하고 모든 소비처는 `None`에서 기존 동작을 그대로 유지하는 보수 폴백이다. 별개 축으로 `RepetitionTracker`에 파일별 S/R 누적 카운터와 missing-field 연속 카운터를 추가해 SR_CORRECTION 발화와 온도 섭동의 트리거를 넓히고, `edit_file`의 검사 순서를 교체한다.

**Tech Stack:** Rust edition 2024, 표준 라이브러리만(신규 크레이트 금지), `cargo test` / `cargo clippy --all-targets -- -D warnings`, Python 3 stdlib(`scripts/exp_metrics.py`).

## Global Constraints

이 절의 제약은 **모든 태스크의 요구사항에 암묵적으로 포함된다.** 태스크를 서브에이전트에 위임할 때 이 절을 디스패치 본문에 그대로 동봉하라.

- **스펙이 유일한 진실:** `docs/superpowers/specs/2026-07-18-m12-honest-harness-design.md` (커밋 fc560f8). 플랜과 스펙이 어긋나면 스펙이 이긴다 — 어긋남을 발견하면 구현하지 말고 보고하라.
- **브랜치 규율:** T1~T9는 브랜치 `m12/honest-harness`에서 작업한다(T1 시작 시 `main`에서 생성). `main` 병합은 T11 판정 후에만. T10(사전등록)은 **사용자 승인 게이트에서 정지**한다.
- **신규 크레이트 금지:** `Cargo.toml`의 의존성 목록은 스펙이 고정한다. 어떤 태스크도 dependency를 추가하지 않는다.
- **모델 대면 텍스트는 전부 영어.** 사용자 대면 CLI 메시지(`AgentEvent::Notice` 등)는 한국어. 식별자·주석 정책은 기존 파일의 관례를 따른다(이 레포는 주석 한국어, 식별자 영어).
- **상태선 마커 계약 불변:** `"[status] "` 접두 + 9칸 연속 들여쓰기. 이 계약은 `src/agent/status_note.rs`·`src/session.rs`·`scripts/exp_metrics.py` 3파일이 공유한다. M12는 상태선의 **내용 행**만 바꾼다 — 마커·들여쓰기·블록 경계 구조는 건드리지 않는다.
- **오류 첫 문장 키 불변:** `Error: edit failed: search and replace are identical - no change would be made`는 `repetition::SR_KEY`와 지표가 의존하는 스트릭 키다. 문장을 **뒤에 덧붙이는** 것은 되지만 첫 문장(`.`까지)은 절대 바꾸지 않는다.
- **지표 위생:** `scripts/exp_metrics.py`의 **기존 컬럼 정의는 소급 변경 금지**. 신규 신호는 신규 컬럼으로만 추가한다.
- **게이트 (모든 태스크의 마지막 단계):** `cargo test`(전건 통과) + `cargo clippy --all-targets -- -D warnings`(무경고). `tasks/`·`tasks-large/`를 건드린 태스크는 없으므로 `--verify`는 T9에서 한 번만 돌린다.
- **커밋:** Conventional commits(제목 한국어 허용). 각 태스크는 최소 1커밋으로 끝난다.
- **측정 태스크(T10·T11) 규율:** GPU 배치 전 사전등록 문서가 **상태 "승인됨"으로 커밋**돼 있어야 한다(전언 승인은 문서 승인을 대체하지 못한다 — M11 전례). 배치는 `setsid` 데몬화로 분리 실행한다(하네스 백그라운드 60분 수명 상한 — 8K 배치 실측 61분). 측정 중 `cargo build`/`test` 병행 금지. 배치 전 `ls ${TMPDIR}/.cargo` 점검(존재 시 수동 제거).

---

## File Structure

| 파일 | 책임 | 태스크 |
|---|---|---|
| `src/test_summary.rs` (신설) | libtest 요약 파싱 순수 함수 — 입력 body, 출력 `Option<TestSummary>` | T2 |
| `src/tools/run_command.rs` (수정) | 0-테스트 무효화 노트 append (파이프 노트와 같은 자리) | T3 |
| `src/agent/status_note.rs` (수정) | `record_command_exit` → `record_command_result(exit, summary)` 시그니처 변경 + verification 렌더 5규칙 + `normalize` 절대경로 버그 수선 | T4 |
| `src/agent/mod.rs` (수정) | 배선: 1회 파싱 → 상태선 주입(T4) → 공허 런 배제 술어(T5) → 섭동 술어 확장(T7) | T4, T5, T7 |
| `src/agent/repetition.rs` (수정) | 파일별 S/R 누적 카운터·파일별 래치·총량 상한 3회, missing-field 연속 카운터 | T6 |
| `src/agent/protocol.rs` (수정) | `args` 안 `tool` 키 salvage 역방향 규칙 (규칙 1·3만 — 레지스트리 불요) | T8 |
| `src/agent/mod.rs` (수정) | 규칙 2(액션 tool 교체) — 레지스트리 조회 필요, 게이트보다 먼저 | T8 |
| `src/tools/edit_file.rs` (수정) | 검사 순서 교체(매치 확정 후 S/R 동일성) + 무변경 사실 문장 | T1 |
| `scripts/exp_metrics.py` (수정) | 신규 컬럼 4종 + `--selftest` 픽스처 확장 | T9 |
| `CLAUDE.md`·`docs/baselines.md` (수정) | 계약문 갱신·각주 | T9 |
| `docs/experiments/2026-07-18-honest-harness/pre-registration.md` (신설) | 경량 사전등록 | T10 |

**태스크 순서의 강제 의존:** T1의 두 소품은 **순서 교체(소품 2)가 무변경 문장(소품 1)보다 먼저**여야 한다(스펙 §4-2 — 교체 전에는 "still contains your search text"가 환각 케이스에서 거짓 문장이 된다). 한 태스크 안에서 단계 순서로 강제한다. T4는 T2의 `TestSummary` 타입에 의존하고, T5는 T4가 만든 `cmd_exit`/`cmd_summary`를 소비하므로 **T4→T5를 같은 작업자가 연속 수행하는 것을 권장한다**(T4 단독으로도 빌드·게이트는 통과하지만 `cmd_summary`가 T5까지 미소비 상태다). T7은 T6에 의존한다. `track_and_note`의 시그니처가 실제로 바뀌는 것은 T8 한 번뿐이다.

---

### Task 1: edit_file 검사 순서 교체 + 무변경 사실 명시

**Files:**
- Modify: `src/tools/edit_file.rs:313-321` (`dry_run`의 S/R 동일성 검사 위치)
- Test: `src/tools/edit_file.rs` 내 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: 없음 (첫 태스크)
- Produces: `ToolError::EditFailed`의 S/R 동일 오류 본문이 `"search and replace are identical - no change would be made. Put the code as it is NOW in `search`, and the code AFTER your change in `replace`. The file was NOT modified - it still contains your search text unchanged."` 가 된다. **첫 문장(첫 `.`까지)은 불변** — `repetition::SR_KEY`가 계속 매치한다.

**배경 (구현자가 알아야 할 것):** `apply_edit`은 3단 매치 사다리(정확 → 후행공백 무시 → 들여쓰기 시프트)를 돌려 `Ok(EditOutcome)` 또는 `Err(String)`을 낸다. 실패 문자열은 두 종류다: `not_found_message`(0매치, closest 인용)와 `ambiguity_message`(≥2매치, 위치 나열). 현재 `dry_run`은 사다리를 **돌리기 전에** `search == replace`를 검사하기 때문에, 파일에 존재하지도 않는 환각 코드를 search/replace에 똑같이 넣으면 "S/R 동일" 오류가 나가고 모델은 그 코드가 파일에 있다고 계속 믿는다(082449Z uv-3에서 실측). 순서를 뒤집으면 환각은 0매치 오류 + closest 인용으로 즉시 반증된다.

- [ ] **Step 1: 순서 교체의 실패 테스트를 쓴다**

`src/tools/edit_file.rs`의 `mod tests` 안에 추가한다. 기존 테스트가 쓰는 헬퍼 관례(임시 디렉토리 + `ToolCtx`)를 그 파일에서 확인해 동일하게 쓸 것 — 아래는 `identical_search_and_replace_is_an_error`(같은 파일 line 510 부근)의 형태를 그대로 따른다.

```rust
#[test]
fn hallucinated_identical_text_reports_not_found_not_sr() {
    // 파일에 존재하지 않는 텍스트를 search/replace에 똑같이 넣으면
    // "S/R 동일"이 아니라 0매치 오류가 나가야 한다 (M12 §4-2-2)
    let (_dir, ctx) = fixture("fn real() {}\n");
    let err = EditFile
        .preview(
            &serde_json::json!({
                "path": "a.rs",
                "search": "fn hallucinated() {}",
                "replace": "fn hallucinated() {}"
            }),
            &ctx,
        )
        .unwrap_err();
    let msg = err.to_string();
    assert!(!msg.contains("identical"), "환각은 S/R 오류로 위장되면 안 된다: {msg}");
    assert!(msg.contains("search block not found"), "{msg}");
}

#[test]
fn identical_error_states_the_file_was_not_modified() {
    let (_dir, ctx) = fixture("fn real() {}\n");
    let err = EditFile
        .preview(
            &serde_json::json!({"path": "a.rs", "search": "fn real() {}", "replace": "fn real() {}"}),
            &ctx,
        )
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.starts_with("Error: edit failed: search and replace are identical - no change would be made"),
        "첫 문장(SR_KEY) 불변: {msg}"
    );
    assert!(msg.contains("The file was NOT modified"), "{msg}");
}
```

`fixture` 헬퍼가 그 파일에 없으면, 기존 테스트가 파일을 만드는 방식을 그대로 복사해 두 테스트에 인라인하라 — 새 헬퍼를 도입하지 말 것.

- [ ] **Step 2: 테스트가 실패하는 것을 확인한다**

Run: `cargo test --lib edit_file`
Expected: `hallucinated_identical_text_reports_not_found_not_sr` FAIL (`identical`이 메시지에 있음), `identical_error_states_the_file_was_not_modified` FAIL (`The file was NOT modified` 없음)

- [ ] **Step 3: 순서를 교체하고 문장을 덧붙인다**

`src/tools/edit_file.rs`의 `dry_run`에서 S/R 동일성 검사 블록(line 313-319)을 **삭제**하고, `apply_edit` 성공 이후로 옮긴다:

```rust
    fn dry_run(&self, args: &Args, ctx: &ToolCtx) -> Result<(String, EditOutcome, bool), ToolError> {
        let path = confine(&ctx.root, &args.path)?;
        let bytes = std::fs::read(&path)?;
        let raw = String::from_utf8(bytes).map_err(|_| ToolError::NotUtf8(args.path.clone()))?;
        let crlf = dominant_crlf(&raw);
        let text = normalize_eol(&raw);
        let search = normalize_eol(&args.search);
        let replace = normalize_eol(&args.replace);
        // M12 §4-2-2: 매치가 확정된 뒤에만 동일성을 검사한다. 순서가 반대면
        // 파일에 없는 환각 코드가 "S/R 동일"로 위장돼 오신념이 교정 기회를 잃는다
        let outcome = apply_edit(&text, &search, &replace, args.replace_all).map_err(ToolError::EditFailed)?;
        if search == replace {
            return Err(ToolError::EditFailed(
                "search and replace are identical - no change would be made. \
                 Put the code as it is NOW in `search`, and the code AFTER your change in `replace`. \
                 The file was NOT modified - it still contains your search text unchanged."
                    .to_string(),
            ));
        }
        Ok((text, outcome, crlf))
    }
```

- [ ] **Step 4: 테스트가 통과하는 것을 확인한다**

Run: `cargo test --lib edit_file`
Expected: PASS (신규 2건 포함)

- [ ] **Step 5: 순서 교체로 깨진 기존 테스트를 정비한다**

Run: `cargo test`
기존 테스트 중 "search==replace이고 파일에 그 텍스트가 없는" 입력으로 `identical`을 기대하는 것이 있으면, 그 테스트의 **입력을 파일에 실재하는 텍스트로 고쳐** 의도(동일성 검사가 동작함)를 유지하라. 기대 문자열을 느슨하게 바꾸는 방식으로 통과시키지 말 것. `repetition.rs`의 `sr_key_matches_actual_edit_file_error_first_sentence` 교차 핀 테스트는 반드시 통과해야 한다(첫 문장 불변의 증거).

- [ ] **Step 6: 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과, 무경고

- [ ] **Step 7: 커밋**

```bash
git add src/tools/edit_file.rs src/agent/repetition.rs
git commit -m "fix(tools): edit_file 검사 순서 교체 — 매치 확정 후 S/R 동일성, 무변경 사실 명시"
```

---

### Task 2: libtest 요약 파서 (신설 순수 모듈)

**Files:**
- Create: `src/test_summary.rs`
- Modify: `src/lib.rs` (`pub mod test_summary;` 선언 추가 — 알파벳 순 유지)
- Test: `src/test_summary.rs` 내 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: 없음
- Produces:
  ```rust
  pub struct TestSummary {
      pub ran: usize,            // passed + failed 합산 (ignored 미포함)
      pub passed: usize,
      pub failed: usize,
      pub failed_names: Vec<String>,  // 수집 순서, 최대 MAX_FAILED_NAMES개
      pub filtered_out: usize,
  }
  pub fn parse_test_summary(body: &str) -> Option<TestSummary>;
  ```
  T3(참조용)·T4·T5가 이 타입과 함수를 쓴다.

**배경 (구현자가 알아야 할 것):** libtest(=`cargo test`의 테스트 러너) 출력은 테스트 바이너리마다 한 섹션이고, 각 섹션이 `test result:` 줄로 끝난다. 워크스페이스는 섹션이 여러 개라 **전 섹션을 합산**해야 한다. 실제 출력 형태:

```
running 2 tests
test tests::a ... ok
test tests::b ... FAILED

failures:

---- tests::b stdout ----
assertion failed

failures:
    tests::b

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

필터가 아무것도 못 맞히면:

```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.00s
```

`ran`을 `running N tests` 헤더가 아니라 **요약 줄의 passed+failed로 도출**하는 이유: 헤더는 `| tail -50`이나 8000바이트 중간 절단에서 잘려 나가기 쉽고 요약 줄은 꼬리에 살아남는다(리뷰 1R 실측).

- [ ] **Step 1: 실패 테스트를 쓴다**

`src/test_summary.rs`를 만들고 테스트부터 넣는다:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_single_failing_section() {
        let body = "exit code: 101\n\
running 2 tests\n\
test tests::a ... ok\n\
test tests::b ... FAILED\n\
\n\
failures:\n\
    tests::b\n\
\n\
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
        let s = parse_test_summary(body).expect("요약 줄이 있으면 Some");
        assert_eq!((s.ran, s.passed, s.failed, s.filtered_out), (2, 1, 1, 0));
        assert_eq!(s.failed_names, vec!["tests::b".to_string()]);
    }

    #[test]
    fn sums_every_section_in_a_workspace_run() {
        let body = "exit code: 101\n\
running 1 test\n\
test alpha ... ok\n\
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n\
\n\
running 2 tests\n\
test beta ... FAILED\n\
test gamma ... FAILED\n\
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n";
        let s = parse_test_summary(body).unwrap();
        assert_eq!((s.ran, s.passed, s.failed, s.filtered_out), (3, 1, 2, 3));
        assert_eq!(s.failed_names, vec!["beta".to_string(), "gamma".to_string()]);
    }

    #[test]
    fn filter_matching_nothing_is_zero_ran_with_filtered_out() {
        let body = "exit code: 0\n\
running 0 tests\n\
\n\
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.00s\n";
        let s = parse_test_summary(body).unwrap();
        assert_eq!((s.ran, s.filtered_out), (0, 13));
    }

    #[test]
    fn ignored_tests_do_not_count_as_ran() {
        // ignored만 있는 섹션은 ran=0 — §2-2 노트·렌더 규칙 3의 문안이 그 경우에도 정직하다
        let body = "test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        let s = parse_test_summary(body).unwrap();
        assert_eq!(s.ran, 0);
        assert_eq!(s.passed, 0);
    }

    #[test]
    fn no_summary_line_is_none() {
        assert!(parse_test_summary("exit code: 0\nhello world\n").is_none());
        assert!(parse_test_summary("").is_none());
    }

    #[test]
    fn summary_line_must_start_the_line() {
        // 임의 출력(cat한 로그 등)이 요약 문구를 줄 중간에 품는 오탐 봉쇄
        let body = "exit code: 0\necho 'test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'\n";
        assert!(parse_test_summary(body).is_none());
    }

    #[test]
    fn count_field_strips_the_status_prefix_on_the_first_field() {
        // 첫 필드는 `ok. ` / `FAILED. ` 접두를 함께 갖는다 — 회귀 방어선
        assert_eq!(count_field("ok. 27 passed; 0 failed; 0 ignored; 0 measured; 291 filtered out", "passed"), 27);
        assert_eq!(count_field("FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out", "passed"), 1);
        assert_eq!(count_field("FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out", "failed"), 2);
    }

    #[test]
    fn failed_names_are_capped() {
        let mut body = String::from("running 9 tests\n");
        for i in 0..9 {
            body.push_str(&format!("test t{i} ... FAILED\n"));
        }
        body.push_str("test result: FAILED. 0 passed; 9 failed; 0 ignored; 0 measured; 0 filtered out\n");
        let s = parse_test_summary(&body).unwrap();
        assert_eq!(s.failed, 9);
        assert_eq!(s.failed_names.len(), MAX_FAILED_NAMES);
    }
}
```

- [ ] **Step 2: 테스트가 컴파일에 실패하는 것을 확인한다**

Run: `cargo test --lib test_summary`
Expected: FAIL — `cannot find function parse_test_summary` (모듈 선언 전이면 모듈 자체를 못 찾는 에러)

- [ ] **Step 3: 파서를 구현한다**

`src/test_summary.rs` 파일 맨 위(테스트 모듈 앞)에 넣는다:

```rust
//! M12 §2-1 — libtest 요약 파서. 하네스가 이미 아는 검증 실질(몇 개 돌았고
//! 몇 개 실패했는지)을 exit code 대신 접지하기 위한 순수 함수.
//! 보수 폴백: 요약 줄이 없으면 None — 모든 소비처는 None에서 기존 동작을 유지한다.

/// verification 렌더·노트가 인용하는 실패 테스트명 상한
pub const MAX_FAILED_NAMES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestSummary {
    /// passed + failed (ignored 미포함 — §2-1)
    pub ran: usize,
    pub passed: usize,
    pub failed: usize,
    /// 수집 순서, MAX_FAILED_NAMES개까지
    pub failed_names: Vec<String>,
    pub filtered_out: usize,
}

/// run_command 결과 body에서 libtest 요약을 합산한다. 요약 줄이 하나도 없으면 None.
/// 줄 시작 앵커 — 임의 출력이 문구를 줄 중간에 품는 오탐을 막는다 (§2-1)
pub fn parse_test_summary(body: &str) -> Option<TestSummary> {
    let mut s = TestSummary { ran: 0, passed: 0, failed: 0, failed_names: Vec::new(), filtered_out: 0 };
    let mut saw_summary = false;
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("test result: ") {
            saw_summary = true;
            s.passed += count_field(rest, "passed");
            s.failed += count_field(rest, "failed");
            s.filtered_out += count_field(rest, "filtered out");
        } else if let Some(name) = line.strip_prefix("test ").and_then(|r| r.strip_suffix(" ... FAILED"))
            && s.failed_names.len() < MAX_FAILED_NAMES
        {
            s.failed_names.push(name.to_string());
        }
    }
    if !saw_summary {
        return None;
    }
    s.ran = s.passed + s.failed;
    Some(s)
}

/// `ok. 1 passed; 2 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s`에서
/// 라벨 앞 숫자를 뽑는다. 없으면 0 (문구 드리프트에 대한 보수 폴백).
/// **첫 필드는 상태 접두(`ok. ` / `FAILED. `)를 함께 갖는다** — 라벨을 떼고 남은
/// 문자열을 통째로 파싱하면 `passed`가 항상 0이 된다(플랜 리뷰 실측). 마지막
/// 공백 토큰만 취해야 한다
fn count_field(rest: &str, label: &str) -> usize {
    rest.split(';')
        .find_map(|part| {
            let n = part.trim().strip_suffix(label)?;
            n.trim().rsplit(char::is_whitespace).next()?.parse::<usize>().ok()
        })
        .unwrap_or(0)
}
```

이 구현은 실제 출력 3형태(`ok. 1 passed; …` / `FAILED. 27 passed; 3 failed; …; 291 filtered out` / `ok. 0 passed; …; 13 filtered out`)에 대해 컨트롤러가 별도 크레이트로 컴파일해 실측 검증했다.



`src/lib.rs`에 모듈을 선언한다 (알파벳 순 유지):

```rust
pub mod agent;
pub mod config;
pub mod eval;
pub mod llm;
pub mod session;
pub mod test_summary;
pub mod tools;
pub mod ui;
```

**위치 근거:** `src/tools/run_command.rs`(T3)와 `src/agent/*`(T4·T5)가 모두 이 파서를 쓴다. `src/agent/` 아래 두면 tools→agent 역방향 의존이 생겨 현행 단방향(agent→tools) 계층이 흐려진다.

- [ ] **Step 4: 테스트가 통과하는 것을 확인한다**

Run: `cargo test --lib test_summary`
Expected: PASS (8건)

- [ ] **Step 5: 실제 cargo 출력으로 파서를 교차 검증한다**

이 레포에서 실제 출력을 뽑아 파서 가정이 맞는지 눈으로 확인한다:

```bash
cargo test zzz_no_such_test_name 2>&1 | tail -20
```

Expected: `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; N filtered out` 형태의 줄이 여러 섹션에 걸쳐 나온다.

**불일치 처방 (순서를 지켜라):** 실출력과 **픽스처**가 다르면 픽스처를 실출력에 맞춰 고친다. 픽스처와 **구현 결과**가 다르면 구현을 고친다. 플랜 리뷰에서 이 지점의 처방이 반대로 적혀 있어 실제 파서 버그를 픽스처 수정으로 덮을 뻔했다 — 픽스처는 실출력과 일치했고 틀린 것은 구현이었다.

추가로 통과 케이스를 직접 확인하라:

```bash
cargo test --lib edit_file 2>&1 | grep "test result:"
```

이 줄의 `N passed`가 파서 결과의 `passed`와 일치해야 한다. 0이 나오면 `count_field`가 첫 필드의 상태 접두를 못 벗긴 것이다.

- [ ] **Step 6: 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과, 무경고

- [ ] **Step 7: 커밋**

```bash
git add src/test_summary.rs src/lib.rs
git commit -m "feat: libtest 요약 파서 신설 — 검증 실질 접지의 공용 기반"
```

---

### Task 3: run_command 0-테스트 무효화 노트

**Files:**
- Modify: `src/tools/run_command.rs:68-79` (`Done` 분기의 노트 append 자리)
- Test: `src/tools/run_command.rs` 내 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::test_summary::{parse_test_summary, TestSummary}` (T2)
- Produces: run_command 결과 body가 조건 충족 시 마지막에 다음 줄을 갖는다:
  `note: 0 tests ran (N filtered out) - cargo test filters by test NAME, not file name; this exit 0 did not verify anything`
  T9의 exp_metrics가 `"0 tests ran ("` 부분문자열로 이 노트를 센다.

**배경:** 현재 `Done` 분기는 `exit code: {code}\n{body}`를 만들고, 명령에 인용되지 않은 파이프가 있으면 파이프 노트 한 줄을 덧붙인다(M11). 0-테스트 노트는 **같은 자리**에 붙고 파이프 노트와 공존할 수 있다. 타임아웃·취소 분기에는 붙이지 않는다(파이프 노트와 동일 규율).

082449Z uv-8이 이 노트가 필요한 이유다: 모델이 `cargo test --package inv-report check_vat_report`를 돌렸는데 cargo가 `check_vat_report`를 **파일명이 아니라 테스트명 필터**로 해석해 0개 실행 + exit 0을 냈고, 모델은 그 exit 0을 네 번 모두 "통과"로 읽고 거짓 finish를 했다.

- [ ] **Step 1: 실패 테스트를 쓴다**

`src/tools/run_command.rs`의 `mod tests` 안(`#[cfg(unix)] mod unix` 바깥의 순수 함수 테스트 자리, `has_unquoted_pipe` 테스트들 옆)에 넣는다. 노트 조립을 순수 함수로 뽑아 테스트하는 형태다 — Step 3에서 그 함수를 만든다.

```rust
#[test]
fn empty_test_run_with_exit_zero_gets_the_invalidation_note() {
    let body = "exit code: 0\n\
running 0 tests\n\
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.00s\n";
    let note = super::empty_test_note(body, "0");
    assert_eq!(
        note.as_deref(),
        Some("note: 0 tests ran (13 filtered out) - cargo test filters by test NAME, not file name; this exit 0 did not verify anything")
    );
}

#[test]
fn a_crate_with_no_tests_at_all_gets_no_note() {
    // filtered_out == 0 이면 "원래 테스트가 없는 크레이트" — 정상이므로 침묵
    let body = "exit code: 0\n\
running 0 tests\n\
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
    assert!(super::empty_test_note(body, "0").is_none());
}

#[test]
fn a_real_test_run_gets_no_note() {
    let body = "exit code: 0\n\
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s\n";
    assert!(super::empty_test_note(body, "0").is_none());
}

#[test]
fn non_zero_exit_gets_no_note() {
    // 실패 exit는 자체 신호가 이미 있다 (§2-2)
    let body = "exit code: 101\n\
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s\n";
    assert!(super::empty_test_note(body, "101").is_none());
}

#[test]
fn non_cargo_output_gets_no_note() {
    assert!(super::empty_test_note("exit code: 0\nhello\n", "0").is_none());
}
```

- [ ] **Step 2: 테스트가 실패하는 것을 확인한다**

Run: `cargo test --lib run_command`
Expected: FAIL — `cannot find function empty_test_note`

- [ ] **Step 3: 노트 함수와 배선을 구현한다**

`src/tools/run_command.rs`의 `has_unquoted_pipe` 옆(파일 상단, line 16 부근)에 추가한다:

```rust
/// M12 §2-2 — 필터가 아무 테스트도 못 맞힌 exit 0을 "검증"으로 읽지 못하게 하는 노트.
/// filtered_out > 0 조건이 "테스트가 원래 없는 크레이트"(정상)와 구분한다
fn empty_test_note(body: &str, exit_code: &str) -> Option<String> {
    if exit_code != "0" {
        return None;
    }
    let s = crate::test_summary::parse_test_summary(body)?;
    (s.ran == 0 && s.filtered_out > 0).then(|| {
        format!(
            "note: 0 tests ran ({} filtered out) - cargo test filters by test NAME, \
             not file name; this exit 0 did not verify anything",
            s.filtered_out
        )
    })
}
```

`Done` 분기(line 68-79)를 고친다:

```rust
            ExecEnd::Done(status) => {
                let code = status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "(terminated by signal)".to_string());
                let mut out = format!("exit code: {code}\n{}", exec.body);
                if has_unquoted_pipe(&args.command) {
                    out.push_str(
                        "\nnote: this command is a pipeline - the exit code reflects only the last command in the pipe",
                    );
                }
                if let Some(note) = empty_test_note(&out, &code) {
                    out.push('\n');
                    out.push_str(&note);
                }
                out
            }
```

- [ ] **Step 4: 테스트가 통과하는 것을 확인한다**

Run: `cargo test --lib run_command`
Expected: PASS (신규 5건 포함)

- [ ] **Step 5: 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과, 무경고

- [ ] **Step 6: 커밋**

```bash
git add src/tools/run_command.rs
git commit -m "feat(tools): 0-테스트 exit 0 무효화 노트 — 필터 오해석 거짓 초록불 차단"
```

---

### Task 4: 상태선 verification 실질화 + normalize 버그 수선

**Files:**
- Modify: `src/agent/status_note.rs` (`record_command_exit` → `record_command_result`, `render`의 verification 분기, `normalize`)
- Test: `src/agent/status_note.rs` 내 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::test_summary::{parse_test_summary, TestSummary, MAX_FAILED_NAMES}` (T2)
- Produces:
  ```rust
  // 시그니처 변경 — 호출자(T5)는 배선 지점에서 1회 파싱해 둘 다 넘긴다
  pub fn record_command_result(&mut self, exit: Option<String>, summary: Option<TestSummary>);
  pub fn normalize(path: &str) -> String;  // pub 승격 — T6이 파일 키로 재사용
  ```
  verification 행은 5규칙으로 렌더된다(아래 Step 3).

**배경:** 현재 `record_command_exit(body)`가 내부에서 body 첫 줄의 `exit code: ` 접두를 파싱한다. M12는 **배선 지점에서 1회 파싱**해 exit와 summary를 함께 주입하는 형태로 바꾼다(스펙 §2-3 — 저장 조건과 §2-4 술어 조건의 계약을 한 지점에 모은다).

**저장 규칙이 핵심이다:** exit 줄이 없는 본문(타임아웃·취소)은 exit와 summary를 **둘 다 None으로 덮는다**. 타임아웃으로 잘린 부분 출력의 통과 섹션만 보고 "all passed"를 접지하는 신규 거짓 초록을 봉쇄하기 위해서다. 그리고 규칙 4(all passed)는 `last_cmd_exit == "0"` 교차 검증을 **추가로** 요구한다(중간 절단으로 실패 섹션이 유실된 exit 101 출력 방어). 규칙 2(failed>0)는 exit 무관 — `cargo test 2>&1 | tail`류 파이프 위장에서 실패를 잡는 순기능이 있다.

`normalize`의 절대경로 버그: `Path::new("/src/a.rs").components()`는 `RootDir`을 먼저 내고, 현재 코드가 그것을 `"/"` 문자열로 밀어 넣은 뒤 `join("/")`을 하기 때문에 `"//src/a.rs"`가 된다. T6이 이 함수를 파일 키로 재사용하므로 여기서 고친다.

- [ ] **Step 1: 실패 테스트를 쓴다**

`src/agent/status_note.rs`의 `mod tests`에 추가한다. 기존 `ctx()` 헬퍼를 그대로 쓴다.

```rust
    fn summary(passed: usize, failed: usize, filtered: usize, names: &[&str]) -> crate::test_summary::TestSummary {
        crate::test_summary::TestSummary {
            ran: passed + failed,
            passed,
            failed,
            failed_names: names.iter().map(|s| s.to_string()).collect(),
            filtered_out: filtered,
        }
    }

    #[test]
    fn failed_tests_render_names_not_exit_code() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        s.record_command_result(Some("101".to_string()), Some(summary(1, 3, 0, &["alpha", "beta", "gamma"])));
        let note = s.on_turn(&ctx(15, false, true, false)).unwrap();
        assert!(note.contains("verification: last cargo test: 3 failed (alpha, beta and 1 more)"), "{note}");
    }

    #[test]
    fn failed_render_omits_the_paren_when_names_were_truncated_away() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        s.record_command_result(Some("101".to_string()), Some(summary(0, 3, 0, &[])));
        let note = s.on_turn(&ctx(15, false, true, false)).unwrap();
        let verification = note.lines().find(|l| l.contains("verification:")).unwrap();
        assert_eq!(verification.trim(), "verification: last cargo test: 3 failed", "{note}");
    }

    #[test]
    fn zero_test_run_renders_filter_matched_nothing() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        s.record_command_result(Some("0".to_string()), Some(summary(0, 0, 13, &[])));
        let note = s.on_turn(&ctx(15, false, true, false)).unwrap();
        assert!(note.contains("verification: last cargo test ran 0 tests (filter matched nothing)"), "{note}");
    }

    #[test]
    fn all_passed_requires_exit_zero_cross_check() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        // exit 0 — 정상 승격
        s.record_command_result(Some("0".to_string()), Some(summary(5, 0, 0, &[])));
        let note = s.on_turn(&ctx(15, false, true, false)).unwrap();
        assert!(note.contains("verification: last cargo test: all 5 passed"), "{note}");
        // exit 101인데 통과 섹션만 남은 출력(중간 절단) — 승격 금지, 규칙 5로 폴백
        s.record_command_result(Some("101".to_string()), Some(summary(5, 0, 0, &[])));
        let note = s.on_turn(&ctx(20, false, true, false)).unwrap();
        assert!(note.contains("verification: last command exited 101"), "{note}");
        assert!(!note.contains("all 5 passed"), "{note}");
    }

    #[test]
    fn timeout_body_clears_both_exit_and_summary() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        s.record_command_result(Some("0".to_string()), Some(summary(5, 0, 0, &[])));
        // 타임아웃 — exit 줄 없음. 배선 지점이 (None, None)을 넘긴다
        s.record_command_result(None, None);
        let note = s.on_turn(&ctx(15, false, true, false)).unwrap();
        assert!(note.contains("verification: last command gave no exit code"), "{note}");
        assert!(!note.contains("cargo test"), "스테일 요약이 남으면 안 된다: {note}");
    }

    #[test]
    fn non_cargo_command_keeps_the_legacy_line() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        s.record_command_result(Some("0".to_string()), None);
        let note = s.on_turn(&ctx(15, false, true, false)).unwrap();
        assert!(note.contains("verification: last command exited 0"), "{note}");
    }

    #[test]
    fn mutated_since_verify_still_wins_over_summary() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        s.record_command_result(Some("0".to_string()), Some(summary(5, 0, 0, &[])));
        let note = s.on_turn(&ctx(15, false, true, true)).unwrap(); // msv=true
        assert!(note.contains("verification: none since your last edit"), "{note}");
    }

    #[test]
    fn normalize_does_not_double_slash_absolute_paths() {
        assert_eq!(normalize("/src/a.rs"), "/src/a.rs");
        assert_eq!(normalize("./src/a.rs"), "src/a.rs");
        assert_eq!(normalize("src//a.rs"), "src/a.rs");
    }
```

- [ ] **Step 2: 테스트가 실패하는 것을 확인한다**

Run: `cargo test --lib status_note`
Expected: FAIL — `record_command_result` 없음, `normalize`가 private + 이중 슬래시

- [ ] **Step 3: 구현한다**

`src/agent/status_note.rs`:

(a) 구조체 필드와 저장 함수를 바꾼다:

```rust
use crate::test_summary::TestSummary;

pub struct StatusNote {
    mutated_paths: Vec<String>,
    last_cmd_exit: Option<String>,
    last_test_summary: Option<TestSummary>,
    pending: bool,
}
```

`new()`에 `last_test_summary: None`을 추가하고, `record_command_exit`를 다음으로 **교체**한다:

```rust
    /// run_command Ok의 결과를 저장한다. 파싱은 배선 지점이 1회 수행해 넘긴다 (§2-3).
    /// exit이 None(타임아웃·취소·무-exit 본문)이면 summary도 함께 None으로 **덮는다** —
    /// 잘린 부분 출력의 통과 섹션이 "all passed"로 접지되는 거짓 초록을 봉쇄
    pub fn record_command_result(&mut self, exit: Option<String>, summary: Option<TestSummary>) {
        if exit.is_none() {
            self.last_cmd_exit = None;
            self.last_test_summary = None;
            return;
        }
        self.last_cmd_exit = exit;
        self.last_test_summary = summary;
    }
```

(b) `render`의 verification 분기를 5규칙으로 바꾼다:

```rust
        let verification = if ctx.mutated_since_verify {
            // 규칙 1 (불변)
            "verification: none since your last edit".to_string()
        } else {
            self.verification_line()
        };
```

그리고 `impl StatusNote`에 헬퍼를 추가한다:

```rust
    /// §2-3 렌더 우선순위 2~5. 규칙 1(mutated_since_verify)은 호출자가 선점한다
    fn verification_line(&self) -> String {
        if let Some(s) = &self.last_test_summary {
            // 규칙 2: 실패 실질 — exit 무관(파이프 위장에서 실패를 잡는 순기능)
            if s.failed > 0 {
                let shown: Vec<&str> = s.failed_names.iter().take(2).map(String::as_str).collect();
                // 절단으로 `test … FAILED` 줄이 유실되면 이름 목록이 빈다 —
                // 그때는 괄호부를 통째로 생략한다("3 failed ( and 3 more)" 방지)
                if shown.is_empty() {
                    return format!("verification: last cargo test: {} failed", s.failed);
                }
                let extra = s.failed.saturating_sub(shown.len());
                let names = if extra > 0 {
                    format!("{} and {extra} more", shown.join(", "))
                } else {
                    shown.join(", ")
                };
                return format!("verification: last cargo test: {} failed ({names})", s.failed);
            }
            // 규칙 3: 필터가 아무것도 못 맞힘
            if s.ran == 0 && s.filtered_out > 0 {
                return "verification: last cargo test ran 0 tests (filter matched nothing)".to_string();
            }
            // 규칙 4: 전부 통과 — exit 0 교차 검증 필수
            if s.failed == 0 && s.ran > 0 && self.last_cmd_exit.as_deref() == Some("0") {
                return format!("verification: last cargo test: all {} passed", s.passed);
            }
        }
        // 규칙 5: 기존 문안
        match &self.last_cmd_exit {
            Some(code) => format!("verification: last command exited {code}"),
            None => "verification: last command gave no exit code".to_string(),
        }
    }
```

(c) `normalize`를 `pub`으로 올리고 `RootDir`을 살린다:

```rust
/// 렉시컬 정규화 — CurDir 제거·ParentDir 팝, 파일시스템 조회 없음
/// (m10/arm-block:src/agent/sr_block.rs에서 포팅 — M10 스펙 §4).
/// M12 §4-1: repetition의 파일별 S/R 카운터가 같은 키를 쓰도록 pub 승격
pub fn normalize(path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut absolute = false;
    for c in Path::new(path).components() {
        match c {
            Component::CurDir => {}
            Component::RootDir => absolute = true,
            Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other.as_os_str().to_string_lossy().into_owned()),
        }
    }
    let joined = parts.join("/");
    if absolute { format!("/{joined}") } else { joined }
}
```

기존 테스트 `verified_state_shows_last_exit_and_overwrite_pins`가 `record_command_exit(body)`를 호출하므로 새 시그니처로 고친다:

```rust
    #[test]
    fn verified_state_shows_last_exit_and_overwrite_pins() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        s.record_command_result(Some("101".to_string()), None);
        let note = s.on_turn(&ctx(15, false, true, false)).unwrap();
        assert!(note.contains("verification: last command exited 101"), "{note}");
        // 덮어쓰기 핀: exit 줄 없는 Ok(타임아웃 본문)는 None으로 덮는다 (§4)
        s.record_command_result(None, None);
        let note = s.on_turn(&ctx(20, false, true, false)).unwrap();
        assert!(note.contains("verification: last command gave no exit code"), "{note}");
    }
```

- [ ] **Step 4: mod.rs 호출부를 제대로 배선한다 (임시 셰임 금지)**

`record_command_exit`가 사라졌으므로 `src/agent/mod.rs`의 호출부를 **이 태스크에서 최종 형태로** 고친다. 임시 셰임을 남기고 커밋하지 말 것 — 모든 커밋은 완결 상태여야 한다.

디스패치 후 블록(line 399-409)을 다음으로 바꾼다:

```rust
            // M12 §2-3: run_command 결과를 여기서 **1회** 파싱해 상태선과
            // (T5에서) 두 술어가 공유한다. 저장 조건과 술어 조건의 계약이 한 지점에 모인다
            let cmd_exit = if turn.action.tool == "run_command" && dispatch_ok {
                body.lines().next().and_then(|l| l.strip_prefix("exit code: ")).map(str::to_string)
            } else {
                None
            };
            let cmd_summary = cmd_exit
                .as_ref()
                .and_then(|_| crate::test_summary::parse_test_summary(&body));
            if dispatch_ok {
                if turn.action.tool == "run_command" {
                    mutated_since_verify = false; // 검증 실행으로 인정 — 종료 코드 무관 (M5 §7.1)
                    status.record_command_result(cmd_exit.clone(), cmd_summary.clone());
                } else if self.registry.get(&turn.action.tool).is_some_and(|t| t.is_mutating()) {
                    mutated_since_verify = true;
                }
                if matches!(turn.action.tool.as_str(), "edit_file" | "write_file") {
                    status.record_mutation(&turn.action.args);
                }
            }
```

- [ ] **Step 5: 테스트가 통과하는 것을 확인한다**

Run: `cargo test --lib status_note && cargo test`
Expected: PASS (신규 8건 포함, 기존 전건 유지)

- [ ] **Step 6: 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과, 무경고

- [ ] **Step 7: 커밋**

```bash
git add src/agent/status_note.rs src/agent/mod.rs
git commit -m "feat(agent): 상태선 verification 실질화 — 테스트 실질 5규칙, normalize 절대경로 수선"
```

---

### Task 5: 공허 검증 배제 — VerifyOk·VERIFY_NUDGE 술어

**Files:**
- Modify: `src/agent/mod.rs` (T4가 만든 `cmd_summary` 소비 지점 + TurnEvent 분류)
- Test: `src/agent/mod.rs` 내 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: T4가 계산한 `cmd_exit`/`cmd_summary`, `finish_nudge::TurnEvent` (기존)
- Produces: 없음 (배선 완결)

**배경:** T4가 파싱을 1회로 모아 뒀다. 이 태스크는 그 결과로 **공허 런**(필터가 아무 테스트도 못 맞힌 실행)을 두 장치에서 배제한다. 082449Z uv-8에서 FINISH_NUDGE가 0-테스트 exit 0을 근거로 무장해 거짓 finish를 직접 처방했고, VERIFY_NUDGE도 그 공허 런을 "검증 시도"로 인정해 해제됐다.

**주의:** `body`는 T3이 노트를 덧붙인 뒤의 문자열이다 — 노트는 `note: `로 시작하는 줄이라 파서의 `test result: ` 줄 시작 앵커에 걸리지 않으므로 파싱에 영향이 없다.

- [ ] **Step 1: 통합 테스트를 쓴다**

`src/agent/mod.rs`의 `mod tests`에 넣는다. 헬퍼는 이 파일에 이미 있는 것들이다(`turn`/`finish`/`ok`/`Scripted::new`/`make_guided_agent`/`new_session`/`run_quiet`/`session_contains`/`script.requests`). 셸을 쓰므로 `#[cfg(unix)]` 게이트가 필요하다(레포 관례).

```rust
    #[cfg(unix)]
    const EMPTY_RUN: &str = "printf 'running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out\n'";
    #[cfg(unix)]
    const REAL_RUN: &str = "printf 'running 1 test\ntest alpha ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n'";

    #[cfg(unix)]
    #[tokio::test]
    async fn empty_test_run_does_not_clear_the_verify_nudge_flag() {
        let dir = tempfile::tempdir().unwrap();
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "a.txt", "content": "x"}))),
            ok(&turn("run_command", serde_json::json!({"command": EMPTY_RUN}))),
            ok(&finish("done")), // 공허 런은 검증 시도가 아니므로 1회 반려된다
            ok(&finish("done")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(_)), "{outcome:?}");
        assert!(session_contains(&session, "never ran a verification command"), "M12 §2-4");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn empty_test_run_does_not_arm_the_finish_nudge() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "answer").unwrap();
        let read = turn("read_file", serde_json::json!({"path": "a.txt"}));
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "b.txt", "content": "x"}))),
            ok(&turn("run_command", serde_json::json!({"command": EMPTY_RUN}))),
            ok(&read),
            ok(&turn("grep", serde_json::json!({"pattern": "answer"}))),
            ok(&read), // 반복 호출
            ok(&turn("list_files", serde_json::json!({}))), // 무장돼 있었다면 4번째 카운트 턴
            ok(&finish("done")),
            ok(&finish("done")), // VERIFY_NUDGE 반려분
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(!session_contains(&session, "do not re-verify"), "공허 런은 무장하지 않는다 (M12 §2-4)");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn real_test_run_still_arms_the_finish_nudge() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "answer").unwrap();
        let read = turn("read_file", serde_json::json!({"path": "a.txt"}));
        let script = Scripted::new(vec![
            ok(&turn("write_file", serde_json::json!({"path": "b.txt", "content": "x"}))),
            ok(&turn("run_command", serde_json::json!({"command": REAL_RUN}))),
            ok(&read),
            ok(&turn("grep", serde_json::json!({"pattern": "answer"}))),
            ok(&read),
            ok(&turn("list_files", serde_json::json!({}))),
            ok(&finish("done")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(session_contains(&session, "do not re-verify"), "실질 통과는 기존대로 무장 (회귀 방어)");
    }
```

- [ ] **Step 2: 테스트가 실패하는 것을 확인한다**

Run: `cargo test --lib agent::tests::empty_test_run agent::tests::real_test_run`
Expected: 앞의 두 건 FAIL — 현재는 공허 런도 `mutated_since_verify = false`로 만들고 `VerifyOk`로 분류한다. `real_test_run_still_arms_the_finish_nudge`는 이미 통과한다(회귀 방어선).

- [ ] **Step 3: 술어를 고친다**

T4가 만든 블록에 `empty_verify`를 추가하고 `mutated_since_verify` 해제를 조건부로 바꾼다:

```rust
            // 공허 런 = 필터가 아무 테스트도 못 맞힌 실행. "검증"으로 인정하지 않는다 (M12 §2-4)
            let empty_verify = cmd_summary.as_ref().is_some_and(|s| s.ran == 0 && s.filtered_out > 0);
            if dispatch_ok {
                if turn.action.tool == "run_command" {
                    // M12 §2-4: 공허 런은 VERIFY_NUDGE를 해제하지 않는다
                    // (해제 조건이었던 "Ok이면 종료코드 무관"에서 공허 런만 제외)
                    if !empty_verify {
                        mutated_since_verify = false;
                    }
                    status.record_command_result(cmd_exit.clone(), cmd_summary.clone());
                } else if self.registry.get(&turn.action.tool).is_some_and(|t| t.is_mutating()) {
                    mutated_since_verify = true;
                }
                if matches!(turn.action.tool.as_str(), "edit_file" | "write_file") {
                    status.record_mutation(&turn.action.args);
                }
            }
```

TurnEvent 분류의 `run_command` 분기:

```rust
                // §4-2: "성공 검증" = Ok ∧ 첫 줄 exit code 0. 타임아웃·취소·Err 본문에는
                // 이 줄이 없어 자연 배제. M12 §2-4: 공허 런(필터 0매치)도 배제 —
                // VerifyOther로 떨어뜨려 기존 무장까지 내린다
                "run_command" => {
                    if dispatch_ok && cmd_exit.as_deref() == Some("0") && !empty_verify {
                        finish_nudge::TurnEvent::VerifyOk { repeat: repeated_call }
                    } else {
                        finish_nudge::TurnEvent::VerifyOther
                    }
                }
```

- [ ] **Step 4: 테스트가 통과하는 것을 확인한다**

Run: `cargo test --lib agent`
Expected: PASS (신규 3건 포함, 기존 전건 유지)

- [ ] **Step 5: 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과, 무경고

- [ ] **Step 6: 커밋**

```bash
git add src/agent/mod.rs
git commit -m "feat(agent): 공허 검증 배제 — VerifyOk 무장·VERIFY_NUDGE 해제 모두 제외"
```

---

### Task 6: 파일별 S/R 누적 카운터 + missing-field 연속 카운터

**Files:**
- Modify: `src/agent/repetition.rs` (`RepetitionTracker` 필드·`error_correction`·신규 접근자)
- Test: `src/agent/repetition.rs` 내 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::agent::status_note::normalize` (T4에서 pub 승격)
- Produces:
  ```rust
  // error_correction의 시그니처 변경 — 호출자(mod.rs track_and_note)가 path를 넘긴다
  pub fn error_correction(&mut self, tool: &str, args: &serde_json::Value, body: &str) -> Option<&'static str>;
  pub fn sr_file_streak(&self) -> usize;   // 마지막 S/R 오류 파일의 누적치 (없으면 0)
  pub fn badargs_streak(&self) -> usize;   // missing-field 연속 길이
  ```
  T7의 섭동 술어가 뒤 두 함수를 쓴다.

**배경 (왜 필요한가):** 082449Z 정독에서 S/R 오류 12건 중 2연속 트리거에 도달한 4건은 **회복률 2/2**였는데, 나머지 8건은 트리거를 비껴갔다(사이에 read/cargo가 끼면 `sr_streak`이 리셋되고, `sr_corrected` 래치는 런당 1회라 두 번째 파일부터는 처방이 아예 없다). 그 8건의 회복은 1건뿐이었다.

**설계 규칙 (스펙 §4-1):**
- 파일별 누적 카운터: 키는 `normalize(path)`, **비연속 허용**. 그 파일에 성공 뮤테이션이 나면 카운터 **리셋 + 래치도 함께 해제**(편집 성공 후 재발한 루프는 별개 사건).
- SR_CORRECTION 발화 조건: **현행 연속 2(파일 무관 — A→B 교차 파일 케이스 보존) ∨ 파일별 누적 2**.
- 래치는 **파일별 1회**, 그리고 **런당 총 발화 상한 3회**(다지점 과제에서 교정 총량이 문맥을 잠식하는 풍선효과 방지 — M10 arm-block 실패 양식).
- `badargs_streak`: `Error: invalid arguments: missing field` 접두로 **한정**한 연속 카운터. 0매치 같은 탐색성 오류로 확대하지 않는다.
- 기존 `sr_streak()`은 **제거하지 않는다**(연속 신호 자체는 지표·기존 테스트 계약).

- [ ] **Step 1: 실패 테스트를 쓴다**

`src/agent/repetition.rs`의 `mod tests`에 추가한다. 기존 테스트가 `error_correction(tool, body)`를 부르므로 **전부 새 시그니처로 고쳐야 한다** — 이 단계에서 함께 처리한다(기존 호출에는 `&serde_json::json!({})`를 넘기면 된다).

```rust
    const SR_BODY: &str = "Error: edit failed: search and replace are identical - no change would be made. Put the code as it is NOW in `search`.";
    const BADARGS_BODY: &str = "Error: invalid arguments: missing field `content`. Expected: write_file(path, content). You sent keys: [path, tool].";

    fn args(path: &str) -> serde_json::Value {
        serde_json::json!({"path": path})
    }

    #[test]
    fn non_consecutive_sr_on_the_same_file_still_fires_the_correction() {
        let mut t = RepetitionTracker::new();
        assert!(t.error_correction("edit_file", &args("src/a.rs"), SR_BODY).is_none(), "1회차는 도구 오류문이 처방");
        // 사이에 성공적인 read가 끼어 연속 스트릭은 끊긴다
        assert!(t.error_correction("read_file", &args("src/a.rs"), "fn main() {}").is_none());
        assert_eq!(t.sr_streak(), 0, "연속 스트릭은 리셋된다(기존 계약 유지)");
        // 같은 파일에서 재발 — 파일별 누적 2 도달로 발화
        assert_eq!(
            t.error_correction("edit_file", &args("src/a.rs"), SR_BODY),
            Some(SR_CORRECTION),
            "비연속이어도 파일별 누적 2면 발화 (M12 §4-1)"
        );
        assert_eq!(t.sr_file_streak(), 2);
    }

    #[test]
    fn the_latch_is_per_file_not_per_run() {
        let mut t = RepetitionTracker::new();
        for _ in 0..2 {
            t.error_correction("edit_file", &args("a.rs"), SR_BODY);
        }
        // 성공 결과를 끼워 **연속** 스트릭을 끊는다 — 그래야 파일별 경로만 검증된다
        // (연속 트리거는 파일 무관이라, 안 끊으면 b.rs 첫 호출에서 이미 발화한다)
        t.error_correction("read_file", &args("a.rs"), "fn main() {}");
        // 두 번째 파일도 자기 몫의 교정을 받는다 (런당 1회 래치 완화)
        assert!(t.error_correction("edit_file", &args("b.rs"), SR_BODY).is_none());
        t.error_correction("read_file", &args("b.rs"), "fn main() {}");
        assert_eq!(t.error_correction("edit_file", &args("b.rs"), SR_BODY), Some(SR_CORRECTION));
    }

    #[test]
    fn a_successful_mutation_resets_that_files_counter_and_latch() {
        let mut t = RepetitionTracker::new();
        for _ in 0..2 {
            t.error_correction("edit_file", &args("a.rs"), SR_BODY);
        }
        // 성공 편집 결과 — 실 런에서는 이 호출이 연속 스트릭을 리셋한다
        t.error_correction("edit_file", &args("a.rs"), "Edited a.rs (matched exact)");
        t.record_mutation_ok("a.rs");
        assert_eq!(t.sr_file_streak(), 0);
        // 편집 성공 후 재발한 루프는 별개 사건 — 교정을 다시 받는다
        assert!(t.error_correction("edit_file", &args("a.rs"), SR_BODY).is_none());
        assert_eq!(t.error_correction("edit_file", &args("a.rs"), SR_BODY), Some(SR_CORRECTION));
    }

    #[test]
    fn total_corrections_are_capped_at_three_per_run() {
        let mut t = RepetitionTracker::new();
        let mut fired = 0;
        for f in ["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"] {
            for _ in 0..2 {
                if t.error_correction("edit_file", &args(f), SR_BODY) == Some(SR_CORRECTION) {
                    fired += 1;
                }
            }
        }
        assert_eq!(fired, 3, "런당 총 발화 상한 3회 (M12 §4-1 풍선효과 방지선)");
    }

    #[test]
    fn cross_file_consecutive_sr_still_fires_the_legacy_way() {
        // A→B 교차 파일 2연속: 파일별 누적은 각 1이지만 연속 스트릭 2로 발화한다
        let mut t = RepetitionTracker::new();
        assert!(t.error_correction("edit_file", &args("a.rs"), SR_BODY).is_none());
        assert_eq!(t.error_correction("edit_file", &args("b.rs"), SR_BODY), Some(SR_CORRECTION));
    }

    #[test]
    fn badargs_streak_counts_only_missing_field_errors() {
        let mut t = RepetitionTracker::new();
        assert_eq!(t.badargs_streak(), 0);
        t.error_correction("write_file", &args("a.rs"), BADARGS_BODY);
        assert_eq!(t.badargs_streak(), 1);
        t.error_correction("write_file", &args("a.rs"), BADARGS_BODY);
        assert_eq!(t.badargs_streak(), 2);
        // 다른 오류류는 스트릭이 아니다 (오발동 봉쇄 — §3-1)
        t.error_correction("edit_file", &args("a.rs"), "Error: edit failed: search block not found. Closest match at lines 3-4");
        assert_eq!(t.badargs_streak(), 0);
    }
```

- [ ] **Step 2: 테스트가 실패하는 것을 확인한다**

Run: `cargo test --lib repetition`
Expected: FAIL — `sr_file_streak`/`badargs_streak`/`record_mutation_ok` 없음, `error_correction` 인자 수 불일치

- [ ] **Step 3: 구현한다**

`src/agent/repetition.rs`:

```rust
use std::collections::HashMap;

/// M12 §4-1 — 파일별 교정 완화가 다지점 과제에서 교정 총량을 키우는 풍선효과를
/// 막는 런당 상한 (M10 arm-block에서 실측된 실패 양식의 방지선)
const MAX_SR_CORRECTIONS: usize = 3;

/// missing-field BadArgs의 스트릭 키 접두 — tools/mod.rs의 스키마 에코 경로와 교차 핀
pub const BADARGS_KEY_PREFIX: &str = "Error: invalid arguments: missing field";

pub struct RepetitionTracker {
    window: VecDeque<(String, u64)>,
    cycle_corrected: bool,
    error_corrected: bool,
    last_error_key: Option<String>,
    error_streak: usize,
    /// M12 §4-1: 파일별 S/R 누적 (비연속 허용). 키 = status_note::normalize(path)
    sr_by_file: HashMap<String, usize>,
    /// 교정을 이미 발화한 파일 (파일별 래치)
    sr_latched: std::collections::HashSet<String>,
    /// 런당 총 발화 수 (상한 MAX_SR_CORRECTIONS)
    sr_correction_count: usize,
    /// 마지막 S/R 오류의 파일 키 — sr_file_streak()의 조회 대상
    last_sr_file: Option<String>,
    /// M12 §3-1: missing-field 연속 길이
    badargs_streak: usize,
}
```

`new()`에서 `sr_corrected` 초기화를 **삭제**하고 대응 초기값을 추가한다(`sr_by_file: HashMap::new()`, `sr_latched: HashSet::new()`, `sr_correction_count: 0`, `last_sr_file: None`, `badargs_streak: 0`).

**`sr_corrected` 필드는 구조체·`new()`·대입문에서 완전히 제거한다.** 파일별 래치가 그 역할을 대신하므로 남겨두면 "런당 래치가 아직 있다"는 오독을 부르는 쓰기 전용 상태가 된다. 현행 읽기 지점은 `repetition.rs:88` 하나뿐이고 테스트는 private 필드에 접근하지 않는다.

`error_correction`을 다음으로 바꾼다:

```rust
    pub fn error_correction(
        &mut self,
        tool: &str,
        args: &serde_json::Value,
        body: &str,
    ) -> Option<&'static str> {
        // M12 §3-1: missing-field 연속만 센다 — 다른 오류류로 확대하지 않는다(오발동 봉쇄)
        if body.starts_with(BADARGS_KEY_PREFIX) {
            self.badargs_streak += 1;
        } else {
            self.badargs_streak = 0;
        }
        if !body.starts_with("Error:") {
            self.last_error_key = None;
            self.error_streak = 0;
            self.last_sr_file = None;
            return None;
        }
        let key = body.split('.').next().unwrap_or(body).to_string();
        if self.last_error_key.as_deref() == Some(key.as_str()) {
            self.error_streak += 1;
        } else {
            self.last_error_key = Some(key);
            self.error_streak = 1;
        }
        // S/R 키 스트릭은 전용 교정이 전담 (M9 §3-2). M12 §4-1: 연속 2(파일 무관)
        // ∨ 파일별 누적 2, 래치는 파일별, 런당 총 상한 MAX_SR_CORRECTIONS
        if tool == "edit_file" && self.last_error_key.as_deref() == Some(SR_KEY) {
            let file = args
                .get("path")
                .and_then(|v| v.as_str())
                .map(crate::agent::status_note::normalize)
                .unwrap_or_default();
            let cum = self.sr_by_file.entry(file.clone()).or_insert(0);
            *cum += 1;
            let cum = *cum;
            self.last_sr_file = Some(file.clone());
            let reached = self.error_streak >= 2 || cum >= 2;
            if reached && !self.sr_latched.contains(&file) && self.sr_correction_count < MAX_SR_CORRECTIONS {
                self.sr_latched.insert(file);
                self.sr_correction_count += 1;
                return Some(SR_CORRECTION);
            }
            return None;
        }
        self.last_sr_file = None;
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

    /// M12 §4-1: 성공 뮤테이션은 그 파일의 누적과 래치를 함께 푼다 —
    /// 편집이 한 번 성공한 뒤 재발한 S/R 루프는 별개 사건이므로 교정을 다시 받는다
    pub fn record_mutation_ok(&mut self, path: &str) {
        let file = crate::agent::status_note::normalize(path);
        self.sr_by_file.remove(&file);
        self.sr_latched.remove(&file);
        if self.last_sr_file.as_deref() == Some(file.as_str()) {
            self.last_sr_file = None;
        }
    }

    /// 마지막 S/R 오류가 난 파일의 누적치 (없으면 0) — M12 §4-1 섭동 술어
    pub fn sr_file_streak(&self) -> usize {
        self.last_sr_file
            .as_ref()
            .and_then(|f| self.sr_by_file.get(f))
            .copied()
            .unwrap_or(0)
    }

    /// missing-field BadArgs 연속 길이 (M12 §3-1) — 섭동 술어
    pub fn badargs_streak(&self) -> usize {
        self.badargs_streak
    }
```

`src/agent/mod.rs`의 `track_and_note`에서 호출부를 고친다:

```rust
        if let Some(strategy) = tracker.error_correction(&turn.action.tool, &turn.action.args, body) {
```

그리고 디스패치 후 성공 뮤테이션 지점(T5에서 손댄 블록)에 카운터 리셋을 추가한다:

```rust
                if matches!(turn.action.tool.as_str(), "edit_file" | "write_file") {
                    status.record_mutation(&turn.action.args);
                    if let Some(p) = turn.action.args.get("path").and_then(|v| v.as_str()) {
                        tracker.record_mutation_ok(p);
                    }
                }
```

**주의:** `tracker`는 이 시점에 `track_and_note`로 넘어가기 전이라 가변 대여가 가능하다. 빌림 검사에 걸리면 `record_mutation_ok` 호출을 `track_and_note` 호출 **직후**로 옮겨라 — 순서상 같은 턴이므로 의미는 동일하다.

- [ ] **Step 4: 테스트가 통과하는 것을 확인한다**

Run: `cargo test --lib repetition`
Expected: PASS (신규 6건 + 기존 전건, 기존 테스트는 새 시그니처로 수정됨)

- [ ] **Step 5: 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과, 무경고

- [ ] **Step 6: 커밋**

```bash
git add src/agent/repetition.rs src/agent/mod.rs
git commit -m "feat(agent): S/R 교정 도달률 — 파일별 누적·파일별 래치·총량 상한, badargs 스트릭"
```

---

### Task 7: 온도 섭동 트리거 확대

**Files:**
- Modify: `src/agent/mod.rs:494-504` (`update_perturb`)
- Test: `src/agent/mod.rs` 내 `mod tests`

**Interfaces:**
- Consumes: `tracker.sr_streak()`, `tracker.sr_file_streak()`, `tracker.badargs_streak()` (T6)
- Produces: 없음

**배경:** M10 실험 1이 입증한 것은 "저온(0.1) 복사 어트랙터는 텍스트 교정이 아니라 온도로 깬다"였다. 현재 그 개입은 S/R **연속** 스트릭에만 걸려 있다. 082449Z에서 missing-field 오형이 최대 5연속 복사로 반복정지까지 간 런이 여럿이었는데(fix-failing-test-0, multiline-string-edit-1, rename-function-2, uv-4 등) 그 경로에는 디코딩층 개입이 전혀 없었다. 확대는 **트리거만** 넓히는 것이고 메커니즘·수명·원복 규칙은 그대로다.

- [ ] **Step 1: 실패 테스트를 쓴다**

`src/agent/mod.rs`의 `mod tests`에 넣는다. 온도 수집 방식은 기존 `sr_streak_of_two_raises_temperature_until_streak_breaks`(line 1536 부근)와 동일하다.

```rust
    #[tokio::test]
    async fn badargs_streak_of_two_raises_temperature() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("g.rs"), "y\n").unwrap();
        let bad = turn("write_file", serde_json::json!({"path": "a.txt"})); // content 누락
        let read = turn("read_file", serde_json::json!({"path": "g.rs"}));
        let script = Scripted::new(vec![ok(&bad), ok(&bad), ok(&read), ok(&finish("d"))]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
        assert_eq!(temps, vec![0.1, 0.1, 0.7, 0.1], "missing-field 2연속 → 섭동, 성공 결과로 원복 {temps:?}");
    }

    #[tokio::test]
    async fn non_consecutive_sr_on_the_same_file_raises_temperature() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
        std::fs::write(dir.path().join("g.rs"), "y\n").unwrap();
        let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
        let read = turn("read_file", serde_json::json!({"path": "g.rs"}));
        let script = Scripted::new(vec![ok(&sr), ok(&read), ok(&sr), ok(&finish("d"))]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
        // 연속 스트릭은 read가 끊었지만 f.rs 파일 누적 2로 섭동 (M12 §4-1)
        assert_eq!(temps, vec![0.1, 0.1, 0.1, 0.7], "{temps:?}");
    }
```

- [ ] **Step 2: 테스트가 실패하는 것을 확인한다**

Run: `cargo test --lib agent::tests::badargs_streak agent::tests::non_consecutive_sr`
Expected: FAIL — 현재 술어는 `sr_streak() >= 2`뿐이라 온도가 0.1로 유지된다

- [ ] **Step 3: 술어를 확대한다**

```rust
    /// M10 §5: 스트릭 상태를 오버라이드에 반영 — track_and_note(error_correction
    /// 경유) 직후에만 호출한다. 무액션·finish 턴은 호출 지점에 닿지 않아 유지된다.
    /// M12 §3-1·§4-1: 트리거만 확대한다(메커니즘·수명·원복 규칙은 불변) —
    /// 파일별 비연속 S/R 재발과 missing-field 오형 복사 루프도 저온 어트랙터다
    fn update_perturb(
        &mut self,
        tracker: &repetition::RepetitionTracker,
        on_event: &mut dyn FnMut(AgentEvent<'_>),
    ) {
        let triggered =
            tracker.sr_streak() >= 2 || tracker.sr_file_streak() >= 2 || tracker.badargs_streak() >= 2;
        let want = triggered.then_some(SR_PERTURB_TEMPERATURE);
        if want.is_some() && self.temperature_override.is_none() {
            on_event(AgentEvent::Notice("(동일 오류 반복 감지 — temperature 일시 상향)".to_string()));
        }
        self.temperature_override = want;
    }
```

- [ ] **Step 4: 테스트가 통과하는 것을 확인한다**

Run: `cargo test --lib agent`
Expected: PASS. 기존 섭동 테스트 전건이 그대로 통과해야 한다(확대는 기존 케이스의 동작을 바꾸지 않는다 — 연속 2회면 파일 누적도 2다). Notice 문구를 바꿨으므로 그 문자열을 기대하는 테스트가 있으면 함께 고친다.

- [ ] **Step 5: 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과, 무경고

- [ ] **Step 6: 커밋**

```bash
git add src/agent/mod.rs
git commit -m "feat(agent): 온도 섭동 트리거 확대 — 파일별 비연속 S/R·missing-field 스트릭"
```

---

### Task 8: salvage 역방향 규칙 (args 안 `tool` 키)

**Files:**
- Modify: `src/agent/protocol.rs` (규칙 1·3 — 레지스트리 불요, 파싱 단계에서 처리 가능한 부분)
- Modify: `src/agent/mod.rs:330-341` (규칙 2 — 레지스트리 조회 필요, **게이트보다 먼저**)
- Test: `src/agent/protocol.rs`, `src/agent/mod.rs`

**Interfaces:**
- Consumes: `self.registry.get(name)` (기존)
- Produces: 신규 상수
  ```rust
  pub const ARGS_TOOL_KEY_NOTE: &str = "note: the `tool` key inside \"args\" is not a parameter - it was removed. Put only the tool's own parameters inside \"args\".";
  pub const ARGS_TOOL_SWITCH_NOTE: &str = "note: \"args\" named a different tool, so this call was dispatched as that tool instead. Put the tool name only in \"action\".\"tool\".";
  ```

**배경 (관측된 오형):** 082449Z·092740Z에서 지배적인 오형은 페이로드 인자를 통째로 빠뜨리고 `tool` 이름을 args 안에 복사하는 것이다:

```json
{"action": {"args": {"path": "src/lib.rs", "tool": "write_file"}, "tool": "write_file"},
 "thought": "Rewrite the file with the fix."}
```

두 번째 형태(uv-2, 5연속 반복정지의 원인)는 액션 tool과 args의 tool이 **다르다** — `read_file` 액션에 `args.tool = "list_files"`. 이건 모델이 "디렉토리에는 list_files를 쓰라"는 오류 힌트를 따르려다 표현을 args 안에 넣은 것이라, 규칙 2로 교체하면 그 호출이 즉시 성공하고 루프가 성립하지 않는다.

**중요한 배선 제약:** 규칙 2의 교체는 **승인 게이트·preview 판정보다 먼저** 적용해야 한다. 교체로 비뮤테이션 액션이 뮤테이션 도구로 바뀔 수 있고, 게이트는 교체 결과 도구를 기준으로 판정해야 하기 때문이다. `finish`는 레지스트리 밖(루프가 직접 처리)이므로 규칙 2의 교체 대상이 될 수 없다 — `args.tool == "finish"`는 미등록 이름으로 규칙 3에 떨어진다.

기존 `SALVAGE_NOTE`("fields outside \"args\" were accepted - put them inside \"args\"")를 **재사용하면 안 된다.** 이 오형은 잉여 키가 args **안**에 있는 정반대 상황이라 오도한다.

- [ ] **Step 1: 실패 테스트를 쓴다**

`src/agent/mod.rs`의 `mod tests`에 넣는다(레지스트리가 필요하므로 protocol 단위 테스트로는 규칙 2를 검증할 수 없다).

```rust
    #[tokio::test]
    async fn duplicate_tool_key_inside_args_is_stripped_with_a_note() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "a.txt", "tool": "read_file"}))),
            ok(&finish("d")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(session_contains(&session, "hi"), "read_file은 정상 디스패치");
        assert!(session_contains(&session, ARGS_TOOL_KEY_NOTE), "전용 노트 (M12 §3-2 규칙 1)");
        assert!(!session_contains(&session, SALVAGE_NOTE), "SALVAGE_NOTE는 정반대 진술이라 붙으면 안 된다");
    }

    #[tokio::test]
    async fn a_different_tool_named_in_args_switches_the_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/inner.rs"), "x").unwrap();
        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "sub", "tool": "list_files"}))),
            ok(&finish("d")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(!session_contains(&session, "is a directory, not a file"), "교체로 루프가 성립하지 않는다 (uv-2)");
        assert!(session_contains(&session, "inner.rs"), "list_files로 교체 디스패치");
        assert!(session_contains(&session, ARGS_TOOL_SWITCH_NOTE), "규칙 2 전용 노트");
    }

    #[tokio::test]
    async fn an_unknown_tool_name_in_args_is_only_stripped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let script = Scripted::new(vec![
            // finish는 레지스트리 밖 — 교체 대상이 아니다(규칙 3)
            ok(&turn("read_file", serde_json::json!({"path": "a.txt", "tool": "finish"}))),
            ok(&finish("d")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
        assert!(matches!(outcome, AgentOutcome::Finished(s) if s == "d"), "액션 도구 유지");
        assert!(session_contains(&session, "hi"));
        assert!(session_contains(&session, ARGS_TOOL_KEY_NOTE));
    }

    #[tokio::test]
    async fn the_switched_tool_is_what_the_approval_gate_sees() {
        struct DenyAll;
        impl crate::agent::approval::Approver for DenyAll {
            fn approve(&mut self, _req: &ApprovalRequest) -> Decision {
                Decision::Deny { reason: "테스트 거부".into() }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let script = Scripted::new(vec![
            // 교체 전이면 read_file(비뮤테이션)이라 게이트를 아예 통과하지 못한다
            ok(&turn("read_file", serde_json::json!({"path": "a.txt", "content": "x", "tool": "write_file"}))),
            ok(&finish("d")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session = new_session(&agent);
        agent.run(&mut session, "x", &mut DenyAll, &mut |_| {}).await.unwrap();
        assert!(session_contains(&session, "Denied:"), "게이트는 교체 결과 도구로 판정해야 한다 (M12 §3-2)");
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "hi", "거부됐으므로 미수정");
    }
```

- [ ] **Step 2: 테스트가 실패하는 것을 확인한다**

Run: `cargo test --lib agent::tests::duplicate_tool_key agent::tests::a_different_tool agent::tests::an_unknown_tool agent::tests::the_switched_tool`
Expected: FAIL — 현재는 `tool` 키가 args에 남아 `read_file`의 BadArgs(unknown field 또는 무시)로 흐른다

- [ ] **Step 3: 정규화를 구현한다**

`src/agent/mod.rs`에 상수와 헬퍼를 추가한다(`SALVAGE_NOTE` 옆, line 55 부근):

```rust
/// M12 §3-2 — args 안의 잉여 `tool` 키를 제거했을 때 붙이는 노트. SALVAGE_NOTE는
/// "args 바깥의 필드를 안으로"라는 정반대 진술이라 이 오형에 재사용하면 오도한다
pub const ARGS_TOOL_KEY_NOTE: &str =
    "note: the `tool` key inside \"args\" is not a parameter - it was removed. \
     Put only the tool's own parameters inside \"args\".";

/// args가 다른 등록 도구를 지목해 그 도구로 교체 디스패치했을 때의 노트 (M12 §3-2)
pub const ARGS_TOOL_SWITCH_NOTE: &str =
    "note: \"args\" named a different tool, so this call was dispatched as that tool instead. \
     Put the tool name only in \"action\".\"tool\".";
```

`run()`의 `on_event(AgentEvent::Action { ... })` **직전**(line 330 앞)에 정규화를 넣는다:

```rust
            // M12 §3-2: args 안의 잉여 `tool` 키 정규화. 게이트·preview보다 **먼저** —
            // 규칙 2의 교체로 비뮤테이션 액션이 뮤테이션 도구가 될 수 있고,
            // 게이트는 교체 결과 도구를 기준으로 판정해야 한다
            let mut args_tool_note: Option<&'static str> = None;
            if let Some(inner) = turn
                .action
                .args
                .get("tool")
                .and_then(|v| v.as_str())
                .map(str::to_string)
            {
                if let Some(map) = turn.action.args.as_object_mut() {
                    map.remove("tool");
                }
                if inner == turn.action.tool {
                    args_tool_note = Some(ARGS_TOOL_KEY_NOTE); // 규칙 1
                } else if self.registry.get(&inner).is_some() {
                    turn.action.tool = inner; // 규칙 2 — 등록 도구면 교체
                    args_tool_note = Some(ARGS_TOOL_SWITCH_NOTE);
                } else {
                    args_tool_note = Some(ARGS_TOOL_KEY_NOTE); // 규칙 3 — 미등록(finish 포함): 키만 제거
                }
            }
```

`turn`이 `let turn = ...`으로 불변 바인딩돼 있으면 `let mut turn = ...`으로 바꿔야 한다. `parse_turn`이 반환하는 지점을 찾아 수정하라.

노트를 결과에 실어야 한다 — `track_and_note`가 노트를 조립하므로 그 함수에 `args_tool_note: Option<&'static str>` 인자를 하나 더 넘긴다. 두 노트는 서로 다른 오형을 가리키므로 **배타로 두지 않는다**(액션 레벨 필드 salvage와 `args.tool` 오형이 한 턴에 겹치면 둘 다 나가야 한다). salvage 분기를 이렇게 고친다:

```rust
        // 액션 레벨 필드 salvage (기존 M5 경로)
        if turn.salvaged {
            notes.push(SALVAGE_NOTE);
        }
        // args 안 `tool` 키 (M12 §3-2) — 위와 배타가 아니다. 한 턴에 겹칠 수 있다
        if let Some(n) = args_tool_note {
            notes.push(n);
        }
```

**주의:** 그러려면 T8의 정규화가 `turn.salvaged = true`를 **세우면 안 된다**(세우면 `args.tool`만 있던 턴에도 정반대 진술인 `SALVAGE_NOTE`가 붙는다). 위 정규화 코드에 그 대입문이 없는 이유다 — `turn.salvaged`는 기존 의미(액션 레벨 필드가 args로 내려옴)를 그대로 유지하고, `args.tool` 오형의 신호는 `args_tool_note`가 단독으로 담당한다.

(시그니처: `fn track_and_note(&self, tracker, turn, body, args_tool_note: Option<&'static str>, on_event)`. 호출 지점 두 곳 모두 갱신하라 — 게이트 거부 경로와 정상 디스패치 경로.)

- [ ] **Step 4: 테스트가 통과하는 것을 확인한다**

Run: `cargo test --lib agent`
Expected: PASS (신규 4건 포함). 기존 salvage 테스트(`salvaged_turn_gets_a_note_with_the_tool_result` 등)가 그대로 통과해야 한다.

- [ ] **Step 5: 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과, 무경고

- [ ] **Step 6: 커밋**

```bash
git add src/agent/mod.rs src/agent/protocol.rs
git commit -m "feat(agent): salvage 역방향 규칙 — args 안 tool 키 제거·교체 디스패치"
```

---

### Task 9: 지표 컬럼·문서·전체 게이트

**Files:**
- Modify: `scripts/exp_metrics.py` (신규 컬럼 4종 + `--selftest` 픽스처)
- Modify: `CLAUDE.md` (Architecture 절 — 신규 장치 + VERIFY_NUDGE 계약문 갱신)
- Modify: `docs/baselines.md` (sr_error·normalize 각주)
- Modify: `docs/superpowers/specs/2026-07-18-m12-honest-harness-design.md` (상태 행)

**Interfaces:**
- Consumes: T1~T8의 산출 전부
- Produces: 없음

- [ ] **Step 1: exp_metrics 신규 컬럼을 추가한다**

`scripts/exp_metrics.py`의 마커 딕셔너리(line 27 부근)와 컬럼 목록(line 31-32)에 추가한다. **기존 컬럼의 정의는 절대 바꾸지 않는다.**

```python
    "empty_test_note": "0 tests ran (",
    "verify_total": "verification: last cargo test: ",
    "verify_zero": "verification: last cargo test ran 0 tests",
    "verify_allpass": "verification: last cargo test: all ",
```

컬럼 목록에 추가할 신규 4종:
- `empty_test_note` — 0-테스트 무효화 노트 발동 수
- `verify_zero` / `verify_allpass` / `verify_failed` — verification 실질 렌더의 **규칙별 분포**(§6이 "분포"를 요구하므로 총수 한 컬럼으로 뭉치지 않는다). 규칙 2는 실패 개수가 가변이라 고정 부분문자열로 잡을 수 없으므로(`MARKS`는 순수 `str.count()` 기반), `MARKS`에는 `verify_total`·`verify_zero`·`verify_allpass` 3종만 두고 행 조립에서 **`verify_failed = verify_total - verify_allpass`로 파생**한다(`verify_total` 마커가 규칙 2·4 양쪽에 매치되기 때문). `--selftest` 픽스처에 세 규칙 렌더를 각각 넣어 (1,1,1)과 파생값 1을 단언할 것
- `sr_file_corr` — 파일별 누적으로 발화한 SR_CORRECTION 수(연속 2 경로와 구분되지 않으면 SR_CORRECTION 총수로 두고 컬럼명을 `sr_corr_total`로 하라 — 트랜스크립트만으로 경로를 구분할 수 없으면 구분하지 말 것)
- `perturb_turns_ext` — 확대 트리거(파일별 S/R·badargs 포함) 기준 섭동 턴 수. **기존 `perturb_turns`는 그대로 둔다** — 그것은 S/R 연속 전용 재구성이고, 소급 재정의는 금지다.

- [ ] **Step 2: selftest 픽스처를 확장하고 돌린다**

`selftest()`의 픽스처 트랜스크립트에 신규 마커가 포함된 이벤트를 추가하고 기대값 단언을 넣는다.

Run: `python3 scripts/exp_metrics.py --selftest`
Expected: 통과 (종료 코드 0)

- [ ] **Step 3: 실제 배치로 추출이 깨지지 않는지 확인한다**

Run: `python3 scripts/exp_metrics.py .loco/eval/20260718T082449Z | tail -3`
Expected: 정상 출력. 신규 컬럼은 M11 배치에 대해 0(장치가 없던 배치이므로) — 기존 컬럼 값이 이전 실행과 **동일해야 한다**(지표 위생 확인).

- [ ] **Step 4: CLAUDE.md를 갱신한다**

Architecture 절에 M12 장치를 영문으로 추가하고, **기존 VERIFY_NUDGE 계약문을 고친다**. 현재 문장:

> A summary-carrying `finish` after unverified mutations is rejected once per run (`VERIFY_NUDGE`; any `run_command` Ok clears the flag regardless of exit code).

M12 이후:

> A summary-carrying `finish` after unverified mutations is rejected once per run (`VERIFY_NUDGE`; a `run_command` Ok clears the flag regardless of exit code **except an empty test run** — see M12 below).

M12 항목은 M11 항목과 같은 밀도로 쓴다: `agent/test_summary.rs` 파서(요약 줄 앵커·전 섹션 합산·`ran = passed + failed`·None 폴백), 0-테스트 노트(`filtered_out > 0` ∧ exit 0), 상태선 verification 5규칙(규칙 4의 exit 0 교차), VerifyOk/VERIFY_NUDGE의 공허 런 배제, 파일별 S/R 누적·파일별 래치·상한 3, 섭동 트리거 확대, salvage 역방향 규칙, edit_file 검사 순서 교체.

- [ ] **Step 5: baselines.md에 각주를 넣는다**

두 각주를 M12 절에 기록한다:
- **sr_error 비교 주의:** T1의 검사 순서 교체로 M12 이후 배치의 `sr_error`는 "매치가 실재하는 진짜 S/R"만 센다. 환각(0매치)과 모호 매치(¬replace_all)로 각각 이동했으므로 이전 배치와 직접 비교하지 말 것.
- **normalize 표기 영향:** 절대경로 입력 시 상태선 파일 목록이 `//src/a.rs` → `/src/a.rs`로 바뀐다(exp_metrics `sr_files`는 basename이라 지표 무영향).

- [ ] **Step 6: 전체 게이트**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify
cargo run -- eval tasks-large --verify
python3 scripts/exp_metrics.py --selftest
```

Expected: tests 전건 통과 / 무경고 / 12/12 / 3/3 / selftest 통과

- [ ] **Step 7: 커밋**

```bash
git add scripts/exp_metrics.py CLAUDE.md docs/
git commit -m "docs: M12 지표 컬럼·계약문 갱신·비교 각주"
```

---

### Task 10: 경량 사전등록 (**사용자 승인 게이트 — 여기서 정지**)

**Files:**
- Create: `docs/experiments/2026-07-18-honest-harness/pre-registration.md`

**Interfaces:**
- Consumes: T9까지의 브랜치 상태(`git rev-parse HEAD`)
- Produces: 승인된 사전등록 문서 — T11의 러너가 이것 없이는 배치를 돌리지 않는다

**배경:** 이건 실험이 아니라 회귀 게이트다. 그래도 PROTOCOL의 "GPU 배치 전 사전등록" 규율은 유지한다(경량 양식: 배치 목록·게이트 값·재측정 규칙·중단 규칙만).

- [ ] **Step 1: 사전등록 문서를 쓴다**

`docs/experiments/PROTOCOL.md`와 `docs/experiments/2026-07-18-progress-grounding/pre-registration.md`를 먼저 읽고 양식을 따른다. 담을 내용:

- **상태: 초안** (승인 전)
- 대상 커밋: T9 완료 시점의 `git rev-parse HEAD` (브랜치 `m12/honest-harness`)
- **배치 1 (게이트):** `tasks/` 스포트 36런, ornith-1.0-9b, seed 0, v2 조건(ctx 8192 / out 4096 / timeout 60 / temp 0.1). 대조 = `20260718T115152Z`(33/36). **게이트: ≥33/36**
- **배치 2 (관찰, 게이트 아님):** `tasks-large` uv+fm@8K 20런, M11 조건(timeout 240). 대조 = `20260718T082449Z`
- **재측정 규칙 (사전 공약):** 배치 1이 <33/36이면 실패 런 전수 법의학 → 신규 장치 귀속 실패가 없으면 **1회 재측정, ≥33/36이면 병합**. 귀속 실패가 있으면 장치 수선 후 재게이트.
- **종결 규칙:** 재측정도 <33/36이면 **귀속 유무와 무관하게 비병합·정지하고 사용자에게 보고한다.** 추가 재측정·규칙 개정은 이 스코프에 없다.
- **관찰 지표(승패 아님, 기록 의무):** `empty_test_note` 발동 수, verification 실질 렌더 분포, sr recovered(대조 23/30), 오형 스트릭발 반복정지, missing-field 오형률 센서스
- **중단 규칙:** 배치 사망 시 1회 재수행, 재실패면 정지·보고(M11 전례)
- **GPU 예산:** 스포트 ~40분 + 8K ~60분 ≈ 1.5–2h. 32K 없음

- [ ] **Step 2: 커밋**

```bash
git add docs/experiments/2026-07-18-honest-harness/pre-registration.md
git commit -m "docs: M12 회귀 게이트 사전등록 초안"
```

- [ ] **Step 3: 사용자 승인을 받고 상태 행을 커밋한다 — 여기서 정지**

사용자에게 문서를 검토받는다. 승인을 받으면 문서의 상태를 "승인됨 (YYYY-MM-DD)"으로 고쳐 **커밋**한다.

**승인 커밋 없이 T11로 진행하지 말 것.** 전언 승인은 문서 승인을 대체하지 못한다(M11에서 러너가 이 게이트에 걸려 정지한 전례가 있다).

---

### Task 11: 회귀 게이트 수행·판정·병합

**Files:**
- Create: `docs/experiments/2026-07-18-honest-harness/report.md`
- Modify: `docs/baselines.md`, `README.md`, 메모리

**Interfaces:**
- Consumes: 승인된 사전등록(T10)
- Produces: 판정 + 병합(또는 정지·보고)

- [ ] **Step 1: 배치 전 게이트를 확인한다**

```bash
git rev-parse HEAD                      # 사전등록의 대상 커밋과 일치하는가
ls ${TMPDIR}/.cargo                     # 존재하면 수동 제거 (트립와이어)
cargo build
cargo run -- eval tasks --verify        # 12/12
cargo run -- eval tasks-large --verify  # 3/3
lms ps                                  # ornith-1.0-9b 단독 로드, ctx 확인
```

모델 교체가 필요하면 `AskUserQuestion`으로 대행 승인을 먼저 받는다.

- [ ] **Step 2: 배치를 수행한다 (러너 무인)**

`loco-experiment-runner` 에이전트에 위임한다. 지시에 반드시 포함할 것:
- 사전등록 문서 경로와 "상태: 승인됨" 확인 후 시작
- **`setsid` 데몬화로 분리 실행** (하네스 백그라운드 60분 수명 상한 — 8K 배치는 실측 61분이라 상한에 걸려 2회 죽은 전례)
- **통지 의존 금지** — 종료는 exit code와 스탬프 디렉토리로 직접 확인
- 측정 중 `cargo build`/`test` 병행 금지
- 배치별 `git rev-parse HEAD`·`effective_config`·`lms` 로드 상태를 report에 기록

- [ ] **Step 3: 지표를 추출한다**

```bash
python3 scripts/exp_metrics.py .loco/eval/<스포트-스탬프> .loco/eval/20260718T115152Z
python3 scripts/exp_metrics.py .loco/eval/<8K-스탬프> .loco/eval/20260718T082449Z
```

- [ ] **Step 4: 판정한다**

사전등록 규칙을 **기계적으로** 적용한다(사후 재해석 금지). 게이트 미달이면 T10에 적은 재측정·종결 규칙을 그대로 따른다.

`docs/experiments/2026-07-18-honest-harness/report.md`에 배치↔커밋↔스탬프 표, 게이트 판정, 관찰 지표, 이상 사항(정직 기록)을 쓴다.

- [ ] **Step 5: 병합한다 (게이트 통과 시에만)**

```bash
git checkout main
git merge --no-ff m12/honest-harness -m "feat: M12 병합 — 정직한 하네스 (회귀 게이트 통과)"
cargo test && cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify
```

- [ ] **Step 6: 문서를 마감한다**

`docs/baselines.md`에 M12 절(스탬프·게이트 결과·관찰 지표), `README.md`에 M12 요지, 스펙 상태 행을 "완료"로. 메모리(`loco-m1-status-and-m2-backlog.md`)에 M12 결과와 **다음 결정 지점 = 실사용 파일럿 논의**를 기록한다.

---

## Self-Review

**스펙 커버리지:**

| 스펙 § | 태스크 |
|---|---|
| §2-1 libtest 파서 (앵커·합산·ran 도출·ignored·None 폴백) | T2 |
| §2-2 0-테스트 무효화 노트 | T3 |
| §2-3 verification 5규칙·저장 규칙·1회 파싱 | T4(렌더·저장), T5(배선) |
| §2-4 VerifyOk·VERIFY_NUDGE 공허 배제·VerifyOther 매핑 | T5 |
| §3-1 badargs 섭동 확대 | T6(카운터), T7(술어) |
| §3-2 salvage 역방향 3규칙·전용 노트·게이트 선행 | T8 |
| §4-1 파일별 누적·래치 파일별·상한 3·수명·리셋 시 해제·normalize 수선 | T4(normalize), T6(카운터·래치), T7(섭동) |
| §4-2 소품 2종 (순서 교체 선행) | T1 |
| §5 회귀 게이트·사전등록·종결 규칙 | T10, T11 |
| §6 신규 컬럼·각주·CLAUDE.md 계약문 | T9 |
| §7 리스크 대응 | 각 태스크의 테스트(None 폴백·클래스 한정·상한)로 흡수 |
| §8 normalize 채택 / has_unquoted_pipe·remove_status_note 이월 | T4(채택), 이월은 무태스크(의도) |

**타입 일관성 확인:** `TestSummary`(T2) → `record_command_result(Option<String>, Option<TestSummary>)`(T4) → 배선의 `cmd_exit`/`cmd_summary`(T5)가 같은 타입으로 흐른다. `normalize`(T4 pub 승격) → `repetition`의 파일 키(T6). `error_correction(tool, args, body)` 시그니처 변경(T6)의 호출자는 `track_and_note`(mod.rs) 한 곳 + 테스트다. `track_and_note` 자신은 이미 `turn`을 갖고 있으므로 T6에서는 **내부 호출문 한 줄만** 바뀐다 — 시그니처가 실제로 바뀌는 것은 T8(`args_tool_note` 추가) 한 번뿐이고, 재배열은 이득이 없다.

**플랜 리뷰 반영 (1R Ready=No → 전건 반영):** ① T2 `count_field`가 첫 필드의 상태 접두(`ok. ` / `FAILED. `)를 못 벗겨 `passed`를 항상 0으로 파싱하던 Critical 버그를 수정했다(컨트롤러가 별도 크레이트로 실측 재현·검증). 이 버그는 정상 통과한 필터 실행을 "0 tests ran"으로 오판시켜 M12의 목적을 반대 방향으로 위반했을 것이다. ② T6 테스트 2건이 자기 구현과 모순(연속 트리거가 파일 무관이라 이미 발화)이던 것을 성공 결과를 끼워 연속 스트릭을 끊는 형태로 고쳤다. ③ T5·T7·T8의 통합 테스트 9건을 실측한 헬퍼 시그니처로 **완전한 코드로 채웠다** — 골격 주석은 남아 있지 않다. ④ Minor: `sr_corrected` 필드 제거, `test_summary`를 `src/lib.rs` 최상위로 이동(tools→agent 역방향 의존 회피), T4의 임시 셰임 제거(모든 커밋이 완결 상태), 규칙 2의 빈 이름 목록 괄호 생략, verify 지표를 규칙별 3컬럼으로 분리.
