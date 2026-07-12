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
    last_error_key: Option<String>,
    error_streak: usize,
}

impl RepetitionTracker {
    pub fn new() -> Self {
        Self {
            window: VecDeque::with_capacity(WINDOW),
            cycle_corrected: false,
            error_corrected: false,
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
}
