//! M11 §4 — 진행 상태 접지(상태선). 하네스가 이미 아는 진행 상태(수정 파일·
//! 검증 여부·잔여 턴)를 조건부로 툴 결과 note에 접지한다. 유도형 — 차단 없음.
//! 채널 격리: note는 body와 분리되므로 반복 해시·오류 스트릭·exit 파싱 무영향.

use std::path::{Component, Path};

use crate::test_summary::TestSummary;

/// 상태선 마커 — session::remove_status_note·scripts/exp_metrics.py와 공유 계약
pub const STATUS_MARKER: &str = "[status] ";
/// 연속 줄 들여쓰기(9칸 = 마커 길이) — 블록 경계 판정 구조 (§4 블록 경계 핀)
pub const CONT_INDENT: &str = "         ";

/// 뮤테이션 0회 케이던스 (조건 2 — 탐색 루프 겨냥).
/// M13 §5-2-1에서 [5,10,15,20] → 초기 조밀화. M12 법의학이 확인한 두 사례
/// (fix-failing-test-1·update-vat-rate-0)가 모두 turn5를 finish가 소비해
/// 렌더되지 못했고, 3이 있었다면 둘 다 turn3에서 렌더됐을 것이다.
/// (turn2에서 finish하는 런은 어떤 케이던스로도 도달 불가 — 이 수선의 상한)
const ZERO_MUT_CADENCE: [usize; 6] = [3, 5, 7, 10, 15, 20];
/// 무조건 페이싱 임계 (조건 3 — 턴 소진 겨냥)
const PACING: [usize; 2] = [15, 20];

/// run()이 turns를 증가시키는 매 턴 종료 지점에서 넘기는 이번 턴의 사실
pub struct TurnCtx {
    /// 이번 액션 턴의 1-기준 순번 = run()의 `turns + 1` (도달 판정 시점 핀)
    pub turn: usize,
    pub max_turns: usize,
    /// 이번 턴이 성공 뮤테이션(edit_file/write_file 디스패치 Ok)인가 (조건 1)
    pub mutation_ok: bool,
    /// push_tool_result 경로인가 — false(length·finish 오류·VERIFY_NUDGE 반려)면
    /// 임계는 pending 이월 (§4 이월 핀)
    pub has_note_channel: bool,
    /// run()의 mutated_since_verify (VERIFY_NUDGE 의미론 상속 — §4)
    pub mutated_since_verify: bool,
}

pub struct StatusNote {
    mutated_paths: Vec<String>,
    last_cmd_exit: Option<String>,
    last_test_summary: Option<TestSummary>,
    pending: bool,
}

impl Default for StatusNote {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusNote {
    pub fn new() -> Self {
        Self {
            mutated_paths: Vec::new(),
            last_cmd_exit: None,
            last_test_summary: None,
            pending: false,
        }
    }

    /// 성공 뮤테이션의 path 수집 — 렉시컬 정규화·중복 제거·삽입 순서 유지.
    /// path 부재/비문자열은 생략 (§4)
    pub fn record_mutation(&mut self, args: &serde_json::Value) {
        let Some(p) = args.get("path").and_then(|v| v.as_str()) else { return };
        let n = normalize(p);
        if !self.mutated_paths.contains(&n) {
            self.mutated_paths.push(n);
        }
    }

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

    /// 턴 종료 지점 판정 — 발동이면 렌더된 상태선(턴당 최대 1회는 호출 지점이
    /// 턴당 하나라는 배선 계약으로 성립). 채널 없는 턴의 임계는 pending 이월
    pub fn on_turn(&mut self, ctx: &TurnCtx) -> Option<String> {
        let cadence = self.mutated_paths.is_empty() && ZERO_MUT_CADENCE.contains(&ctx.turn);
        let pacing = PACING.contains(&ctx.turn);
        if !(ctx.mutation_ok || cadence || pacing || self.pending) {
            return None;
        }
        if !ctx.has_note_channel {
            self.pending = true;
            return None;
        }
        self.pending = false;
        Some(self.render(ctx))
    }

    fn render(&self, ctx: &TurnCtx) -> String {
        let turns_line = format!("turns: {} of {} used", ctx.turn, ctx.max_turns);
        if self.mutated_paths.is_empty() {
            // M13 §5-2-2 — 뮤테이션이 없어도 마지막 검증 결과는 접지한다.
            // 규칙 1(mutated_since_verify)은 뮤테이션을 전제하므로 여기선 도달 불가:
            // verification_line()의 규칙 2~5만 탄다.
            let verification = self.verification_line();
            return format!("{STATUS_MARKER}files edited: none yet | {verification} | {turns_line}");
        }
        let shown = self.mutated_paths.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
        let extra = self.mutated_paths.len().saturating_sub(5);
        let files = if extra > 0 {
            format!("{} ({shown} and {extra} more)", self.mutated_paths.len())
        } else {
            format!("{} ({shown})", self.mutated_paths.len())
        };
        let verification = if ctx.mutated_since_verify {
            // 규칙 1 (불변)
            "verification: none since your last edit".to_string()
        } else {
            self.verification_line()
        };
        format!("{STATUS_MARKER}files edited: {files}\n{CONT_INDENT}{verification}\n{CONT_INDENT}{turns_line}")
    }

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
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(turn: usize, mutation_ok: bool, channel: bool, msv: bool) -> TurnCtx {
        TurnCtx { turn, max_turns: 25, mutation_ok, has_note_channel: channel, mutated_since_verify: msv }
    }

    #[test]
    fn zero_mutation_cadence_fires_at_3_5_7_10_15_20() {
        let mut s = StatusNote::new();
        for t in 1..=25 {
            let got = s.on_turn(&ctx(t, false, true, false)).is_some();
            let want = matches!(t, 3 | 5 | 7 | 10 | 15 | 20);
            assert_eq!(got, want, "turn {t}");
        }
    }

    #[test]
    fn zero_mutation_note_renders_verification_line() {
        // 수선 B — 뮤테이션 0회에서도 마지막 cargo test 결과를 접지한다.
        // fix-failing-test-1 재현: turn1 cargo test가 1 failed(max_of_list)
        let mut s = StatusNote::new();
        s.record_command_result(
            Some("101".to_string()),
            Some(TestSummary {
                ran: 5,
                passed: 4,
                failed: 1,
                failed_names: vec!["max_of_list".to_string()],
                filtered_out: 0,
            }),
        );
        let note = s.on_turn(&ctx(3, false, true, false)).expect("케이던스 3 발동");
        assert!(note.contains("files edited: none yet"), "{note}");
        assert!(note.contains("1 failed (max_of_list)"), "검증 줄이 실려야 함: {note}");
        assert!(note.contains("turns: 3 of 25 used"), "{note}");
    }

    #[test]
    fn zero_mutation_note_without_any_command_keeps_the_old_shape() {
        // run_command가 한 번도 없었으면 검증 줄은 규칙 5로 떨어진다 —
        // 없는 사실을 지어내지 않는지 핀
        let mut s = StatusNote::new();
        let note = s.on_turn(&ctx(3, false, true, false)).expect("케이던스 3 발동");
        assert!(note.contains("files edited: none yet"), "{note}");
        assert!(note.contains("gave no exit code"), "{note}");
    }

    #[test]
    fn mutation_turn_fires_immediately_and_cadence_stops_after_mutation() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "src/a.rs"}));
        let note = s.on_turn(&ctx(2, true, true, true)).expect("조건 1");
        assert!(note.contains("files edited: 1 (src/a.rs)"), "{note}");
        assert!(note.contains("verification: none since your last edit"), "{note}");
        assert!(note.contains("turns: 2 of 25 used"), "{note}");
        // 뮤테이션 이후 케이던스(조건 2)는 침묵 — 15·20(조건 3)만 남는다
        assert!(s.on_turn(&ctx(5, false, true, true)).is_none());
        assert!(s.on_turn(&ctx(10, false, true, true)).is_none());
        assert!(s.on_turn(&ctx(15, false, true, true)).is_some(), "조건 3");
    }

    #[test]
    fn zero_mutation_render_is_single_line() {
        // 수선 B 이후에도 여전히 한 줄이다 — 검증 줄이 파이프로 끼어들 뿐
        let mut s = StatusNote::new();
        let note = s.on_turn(&ctx(5, false, true, false)).unwrap();
        assert_eq!(
            note,
            "[status] files edited: none yet | verification: last command gave no exit code | turns: 5 of 25 used"
        );
        assert_eq!(note.lines().count(), 1, "여전히 한 줄: {note}");
    }

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
        // 실패 요약으로 채운다 — 규칙 2는 exit 무관 렌더이므로, last_test_summary가
        // 실제로 지워지지 않으면 아래 타임아웃 이후에도 "N failed"가 새어나온다
        // (통과 요약을 쓰면 규칙 4의 exit=="0" 교차 검증에 가려 이 테스트가 공허해진다)
        s.record_command_result(Some("101".to_string()), Some(summary(0, 3, 0, &["alpha", "beta"])));
        // 타임아웃 — exit 줄 없음. 배선 지점이 (None, None)을 넘긴다
        s.record_command_result(None, None);
        let note = s.on_turn(&ctx(15, false, true, false)).unwrap();
        assert!(note.contains("verification: last command gave no exit code"), "{note}");
        assert!(!note.contains("cargo test"), "스테일 실패 요약 잔존: {note}");
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

    #[test]
    fn path_list_caps_at_five_with_lexical_dedup() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "./src/a.rs"}));
        s.record_mutation(&serde_json::json!({"path": "src/a.rs"})); // 표기 변형 — 합산
        for p in ["b.rs", "c.rs", "d.rs", "e.rs", "f.rs", "g.rs"] {
            s.record_mutation(&serde_json::json!({"path": p}));
        }
        let note = s.on_turn(&ctx(9, true, true, true)).unwrap();
        assert!(note.contains("files edited: 7 (src/a.rs, b.rs, c.rs, d.rs, e.rs and 2 more)"), "{note}");
    }

    #[test]
    fn threshold_on_channelless_turn_carries_over_once() {
        // 세 번째 프로브는 케이던스 아닌 턴이어야 "이월 소진"과 "새 케이던스 발동"이
        // 구별된다 — turn 7은 M13에서 케이던스 지점이 됐으므로 turn 8로 옮긴다
        let mut s = StatusNote::new();
        assert!(s.on_turn(&ctx(5, false, false, false)).is_none(), "채널 없음 — 이월");
        let note = s.on_turn(&ctx(6, false, true, false)).expect("이월분 1회 주입");
        assert!(note.contains("turns: 6 of 25"), "{note}");
        assert!(s.on_turn(&ctx(8, false, true, false)).is_none(), "이월은 1회로 소진");
    }

    #[test]
    fn continuation_lines_use_nine_space_indent() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        let note = s.on_turn(&ctx(3, true, true, true)).unwrap();
        for line in note.lines().skip(1) {
            assert!(line.starts_with(CONT_INDENT), "블록 경계 구조 계약: {line:?}");
        }
    }

    #[test]
    fn missing_or_non_string_path_is_skipped() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({}));
        s.record_mutation(&serde_json::json!({"path": 42}));
        assert!(s.on_turn(&ctx(5, false, true, false)).unwrap().contains("none yet"));
    }
}
