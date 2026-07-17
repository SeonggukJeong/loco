//! 반복 감지 — (호출, 결과 해시) 8턴 윈도 + 동일 에러 연속 스트릭 (M5 스펙 §7.2).
//! 계수는 디스패치 후(결과 확보 시점) — 결과를 예단하는 정지는 두지 않는다.

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

/// 윈도 크기 8: 5회 정지는 사실상 연속 반복(주기 1)에서 도달한다. 엄격한 주기 2
/// 교대는 윈도 내 같은 항목이 최대 4회, 주기 3은 최대 3회라 교정(3회째)+max_turns가
/// 상한 (더 넓히면 "다른 편집 사이 동일한 실패 테스트 결과"를 오정지할 위험)
const WINDOW: usize = 8;

pub const EDIT_STRATEGY_CORRECTION: &str = "The same error keeps occurring. Change strategy: re-read the file, then rewrite it completely with write_file.";
pub const GENERIC_STRATEGY_CORRECTION: &str = "The same error keeps occurring. Step back and try a different approach.";

/// edit_file S/R 자기-버그 2연속 전용 교정 (M9 §3-2). 모델 대상 — 영어
pub const SR_CORRECTION: &str = "Your `replace` is identical to `search`. Write the MODIFIED code in `replace`. \
If you cannot produce a different `replace`, rewrite the whole file with write_file, applying the fix.";

/// S/R 오류의 스트릭 키(첫 문장) — tools/edit_file.rs의 실제 오류문과
/// sr_key_matches_actual_edit_file_error_first_sentence 테스트로 고정 (M9 §3-2)
pub const SR_KEY: &str = "Error: edit failed: search and replace are identical - no change would be made";

#[derive(Debug, PartialEq)]
pub enum RepetitionVerdict {
    Ok,
    /// 윈도 내 동일 (호출, 결과) 3회째 — 교정 1회 주입
    InjectCorrection,
    /// 5회째 — RepetitionStop
    Stop,
}

pub struct RepetitionTracker {
    window: VecDeque<(String, u64)>,
    cycle_corrected: bool,
    error_corrected: bool,
    sr_corrected: bool,
    last_error_key: Option<String>,
    error_streak: usize,
}

impl RepetitionTracker {
    pub fn new() -> Self {
        Self {
            window: VecDeque::with_capacity(WINDOW),
            cycle_corrected: false,
            error_corrected: false,
            sr_corrected: false,
            last_error_key: None,
            error_streak: 0,
        }
    }

    pub fn record(&mut self, key: &str, body: &str) -> RepetitionVerdict {
        let entry = (key.to_string(), hash_of(body));
        if self.window.len() == WINDOW {
            self.window.pop_front();
        }
        self.window.push_back(entry.clone());
        let count = self.window.iter().filter(|e| **e == entry).count();
        if count >= 5 {
            return RepetitionVerdict::Stop;
        }
        if count == 3 && !self.cycle_corrected {
            self.cycle_corrected = true;
            return RepetitionVerdict::InjectCorrection;
        }
        RepetitionVerdict::Ok
    }

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

    /// S/R 오류 연속 길이 (M10 §5) — 디코딩 섭동의 트리거 술어.
    /// error_correction()의 Some(SR_CORRECTION) 반환에 걸지 말 것 — 그쪽은 런당
    /// 1회 래치라 "스트릭 재도달 시 재활성"이 깨진다. §5 트리거의 tool==edit_file
    /// 술어는 생략 — SR_KEY 본문은 edit_file만 낸다(도구 층 오류문 교차 핀)
    pub fn sr_streak(&self) -> usize {
        if self.last_error_key.as_deref() == Some(SR_KEY) { self.error_streak } else { 0 }
    }
}

impl Default for RepetitionTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_of(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn third_identical_call_and_result_injects_once_fifth_stops() {
        let mut t = RepetitionTracker::new();
        assert!(matches!(t.record("grep|{}", "no matches"), RepetitionVerdict::Ok));
        assert!(matches!(t.record("grep|{}", "no matches"), RepetitionVerdict::Ok));
        assert!(matches!(t.record("grep|{}", "no matches"), RepetitionVerdict::InjectCorrection));
        assert!(matches!(t.record("grep|{}", "no matches"), RepetitionVerdict::Ok), "교정은 1회만");
        assert!(matches!(t.record("grep|{}", "no matches"), RepetitionVerdict::Stop));
    }

    #[test]
    fn period_two_alternation_is_caught() {
        // read↔edit 왕복 (qwen rename-function 실측 패턴) — 연속이 아니어도 윈도가 잡는다
        let mut t = RepetitionTracker::new();
        for _ in 0..2 {
            assert!(matches!(t.record("read_file|a", "same"), RepetitionVerdict::Ok));
            assert!(matches!(t.record("edit_file|x", "Error: not found"), RepetitionVerdict::Ok));
        }
        assert!(matches!(t.record("read_file|a", "same"), RepetitionVerdict::InjectCorrection));
    }

    #[test]
    fn changed_result_resets_the_pattern() {
        // 편집 후 재읽기: 같은 호출이라도 결과가 다르면 무해 (스펙 §7.2)
        let mut t = RepetitionTracker::new();
        for _ in 0..4 {
            t.record("read_file|a", "old content");
        }
        assert!(matches!(t.record("read_file|a", "NEW content"), RepetitionVerdict::Ok));
    }

    #[test]
    fn window_caps_at_eight_entries() {
        let mut t = RepetitionTracker::new();
        t.record("a|1", "r");
        t.record("a|1", "r");
        // 8턴 밀어내기 — 오래된 2건이 윈도 밖으로
        for i in 0..8 {
            t.record(&format!("pad|{i}"), "x");
        }
        assert!(matches!(t.record("a|1", "r"), RepetitionVerdict::Ok), "윈도 밖 반복은 무효");
    }

    #[test]
    fn partial_eviction_third_hit_evicts_oldest_and_stays_ok() {
        // M7 §6.2 — 완전 축출(window_caps_at_eight_entries)과 구별되는 오프바이원 경계:
        // 히트2+패딩6=만석 → 3번째 히트 푸시가 스스로 최고령 히트를 축출 → 카운트 2 → Ok
        let mut t = RepetitionTracker::new();
        t.record("a|1", "r");
        t.record("a|1", "r");
        for i in 0..6 {
            t.record(&format!("pad|{i}"), "x");
        }
        assert!(matches!(t.record("a|1", "r"), RepetitionVerdict::Ok), "축출로 3회 미달");
    }

    #[test]
    fn partial_eviction_one_fewer_pad_still_corrects() {
        // 위 케이스의 쌍 — 패딩 하나 적으면(2+5=7, 축출 없음) 3회가 성립해 교정 주입
        let mut t = RepetitionTracker::new();
        t.record("a|1", "r");
        t.record("a|1", "r");
        for i in 0..5 {
            t.record(&format!("pad|{i}"), "x");
        }
        assert!(matches!(t.record("a|1", "r"), RepetitionVerdict::InjectCorrection));
    }

    #[test]
    fn same_error_first_sentence_three_times_yields_strategy_correction_once() {
        let mut t = RepetitionTracker::new();
        assert!(t.error_correction("edit_file", "Error: edit failed: search block not found. Closest match at lines 3-5:\nfoo").is_none());
        assert!(t.error_correction("edit_file", "Error: edit failed: search block not found. Closest match at lines 8-9:\nbar").is_none(), "첫 문장(첫 마침표까지) 비교 — 뒤의 가변 내용은 무시");
        // 세 번째 — 파일 편집 계열이므로 write_file 권고
        let c = t.error_correction("edit_file", "Error: edit failed: search block not found. etc");
        assert_eq!(c, Some(EDIT_STRATEGY_CORRECTION));
        assert!(t.error_correction("edit_file", "Error: edit failed: search block not found. etc").is_none(), "1회만");
    }

    #[test]
    fn same_error_via_non_edit_tool_gets_generic_correction() {
        let mut t = RepetitionTracker::new();
        t.error_correction("run_command", "Error: invalid arguments: missing field `command`");
        t.error_correction("run_command", "Error: invalid arguments: missing field `command`");
        let c = t.error_correction("run_command", "Error: invalid arguments: missing field `command`");
        assert_eq!(c, Some(GENERIC_STRATEGY_CORRECTION));
    }

    #[test]
    fn non_errors_and_different_errors_reset_the_streak() {
        let mut t = RepetitionTracker::new();
        t.error_correction("grep", "Error: x");
        t.error_correction("grep", "Error: x");
        assert!(t.error_correction("grep", "ok result").is_none());
        t.error_correction("grep", "Error: x");
        t.error_correction("grep", "Error: x");
        assert!(t.error_correction("grep", "Error: x").is_some(), "리셋 후 다시 3연속");
    }

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
    fn sr_streak_tracks_consecutive_sr_errors_only() {
        let mut t = RepetitionTracker::new();
        let sr = format!("{SR_KEY}. x.");
        assert_eq!(t.sr_streak(), 0);
        t.error_correction("edit_file", &sr);
        assert_eq!(t.sr_streak(), 1);
        t.error_correction("edit_file", &sr);
        assert_eq!(t.sr_streak(), 2, "SR_CORRECTION 래치와 무관하게 스트릭은 계속 노출 (M10 §5 배선 주의)");
        t.error_correction("edit_file", &sr);
        assert_eq!(t.sr_streak(), 3);
        t.error_correction("edit_file", "ok result");
        assert_eq!(t.sr_streak(), 0, "비-S/R 결과로 리셋");
        t.error_correction("edit_file", "Error: edit failed: search block not found. y");
        assert_eq!(t.sr_streak(), 0, "다른 오류 키는 S/R 스트릭이 아님");
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
}
