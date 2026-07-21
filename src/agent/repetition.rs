//! 반복 감지 — (호출, 결과 해시) 8턴 윈도 + 동일 에러 연속 스트릭 (M5 스펙 §7.2).
//! 계수는 디스패치 후(결과 확보 시점) — 결과를 예단하는 정지는 두지 않는다.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};

/// 윈도 크기 8: 5회 정지는 사실상 연속 반복(주기 1)에서 도달한다. 엄격한 주기 2
/// 교대는 윈도 내 같은 항목이 최대 4회, 주기 3은 최대 3회라 교정(3회째)+max_turns가
/// 상한 (더 넓히면 "다른 편집 사이 동일한 실패 테스트 결과"를 오정지할 위험)
const WINDOW: usize = 8;

/// M12 §4-1 — 파일별 교정 완화가 다지점 과제에서 교정 총량을 키우는 풍선효과를
/// 막는 런당 상한 (M10 arm-block에서 실측된 실패 양식의 방지선)
const MAX_SR_CORRECTIONS: usize = 3;

/// missing-field BadArgs의 스트릭 키 접두 — tools/mod.rs의 스키마 에코 경로와
/// badargs_key_prefix_matches_actual_missing_field_errors_only 테스트로 교차 핀 (M12 §3-1).
/// 모듈 밖에서 쓰이지 않음 — T7은 이 상수가 아니라 badargs_streak() 게터를 소비한다
/// (플랜 §Task 7), 따라서 pub을 유지할 이유가 없어 가시성을 좁힌다
const BADARGS_KEY_PREFIX: &str = "Error: invalid arguments: missing field";

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
    last_error_key: Option<String>,
    error_streak: usize,
    /// M12 §4-1: 파일별 S/R 누적 (비연속 허용). 키 = status_note::normalize(path)
    sr_by_file: HashMap<String, usize>,
    /// 교정을 이미 발화한 파일 (파일별 래치)
    sr_latched: HashSet<String>,
    /// 런당 총 발화 수 (상한 MAX_SR_CORRECTIONS)
    sr_correction_count: usize,
    /// 마지막 S/R 오류의 파일 키 — sr_file_streak()의 조회 대상
    last_sr_file: Option<String>,
    /// M12 §3-1: missing-field 연속 길이
    badargs_streak: usize,
}

impl RepetitionTracker {
    pub fn new() -> Self {
        Self {
            window: VecDeque::with_capacity(WINDOW),
            cycle_corrected: false,
            error_corrected: false,
            last_error_key: None,
            error_streak: 0,
            sr_by_file: HashMap::new(),
            sr_latched: HashSet::new(),
            sr_correction_count: 0,
            last_sr_file: None,
            badargs_streak: 0,
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
        // 동일성 키 = 첫 문장(첫 '.'까지). 개선된 에러들은 첫 줄 안에 가변 내용을
        // 붙이므로(스키마 에코의 키 목록, not-found의 `lines A-B`) 첫 줄 비교는 무력
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
            if reached
                && !self.sr_latched.contains(&file)
                && self.sr_correction_count < MAX_SR_CORRECTIONS
            {
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
    }

    /// 마지막 S/R 오류가 난 파일의 누적치 (없으면 0) — M12 §4-1 섭동 술어
    pub fn sr_file_streak(&self) -> usize {
        self.last_sr_file.as_ref().and_then(|f| self.sr_by_file.get(f)).copied().unwrap_or(0)
    }

    /// missing-field BadArgs 연속 길이 (M12 §3-1) — 섭동 술어
    pub fn badargs_streak(&self) -> usize {
        self.badargs_streak
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

    /// error_correction의 args 매개변수 — path와 무관한 테스트에서 쓰는 빈 값
    fn no_args() -> serde_json::Value {
        serde_json::json!({})
    }

    #[test]
    fn same_error_first_sentence_three_times_yields_strategy_correction_once() {
        let mut t = RepetitionTracker::new();
        assert!(t.error_correction("edit_file", &no_args(), "Error: edit failed: search block not found. Closest match at lines 3-5:\nfoo").is_none());
        assert!(t.error_correction("edit_file", &no_args(), "Error: edit failed: search block not found. Closest match at lines 8-9:\nbar").is_none(), "첫 문장(첫 마침표까지) 비교 — 뒤의 가변 내용은 무시");
        // 세 번째 — 파일 편집 계열이므로 write_file 권고
        let c = t.error_correction("edit_file", &no_args(), "Error: edit failed: search block not found. etc");
        assert_eq!(c, Some(EDIT_STRATEGY_CORRECTION));
        assert!(t.error_correction("edit_file", &no_args(), "Error: edit failed: search block not found. etc").is_none(), "1회만");
    }

    #[test]
    fn same_error_via_non_edit_tool_gets_generic_correction() {
        let mut t = RepetitionTracker::new();
        t.error_correction("run_command", &no_args(), "Error: invalid arguments: missing field `command`");
        t.error_correction("run_command", &no_args(), "Error: invalid arguments: missing field `command`");
        let c = t.error_correction("run_command", &no_args(), "Error: invalid arguments: missing field `command`");
        assert_eq!(c, Some(GENERIC_STRATEGY_CORRECTION));
    }

    #[test]
    fn non_errors_and_different_errors_reset_the_streak() {
        let mut t = RepetitionTracker::new();
        t.error_correction("grep", &no_args(), "Error: x");
        t.error_correction("grep", &no_args(), "Error: x");
        assert!(t.error_correction("grep", &no_args(), "ok result").is_none());
        t.error_correction("grep", &no_args(), "Error: x");
        t.error_correction("grep", &no_args(), "Error: x");
        assert!(t.error_correction("grep", &no_args(), "Error: x").is_some(), "리셋 후 다시 3연속");
    }

    #[test]
    fn sr_error_second_consecutive_gets_dedicated_correction_once() {
        let mut t = RepetitionTracker::new();
        let body = format!("{SR_KEY}. Put the code as it is NOW in `search`, and the code AFTER your change in `replace`.");
        assert!(t.error_correction("edit_file", &no_args(), &body).is_none(), "1회차는 도구 오류문이 담당");
        assert_eq!(t.error_correction("edit_file", &no_args(), &body), Some(SR_CORRECTION), "2연속에 전용 교정 (M9 §3-2)");
        assert!(t.error_correction("edit_file", &no_args(), &body).is_none(), "런당 1회 래치");
        assert!(t.error_correction("edit_file", &no_args(), &body).is_none(), "S/R 스트릭에는 일반 교정도 불발 (전담 배제)");
    }

    #[test]
    fn sr_correction_does_not_consume_generic_latch() {
        let mut t = RepetitionTracker::new();
        let sr = format!("{SR_KEY}. x.");
        t.error_correction("edit_file", &no_args(), &sr);
        assert_eq!(t.error_correction("edit_file", &no_args(), &sr), Some(SR_CORRECTION));
        // 다른 오류 스트릭은 여전히 일반 교정을 받는다 (별도 래치)
        t.error_correction("grep", &no_args(), "Error: x");
        t.error_correction("grep", &no_args(), "Error: x");
        assert_eq!(t.error_correction("grep", &no_args(), "Error: x"), Some(GENERIC_STRATEGY_CORRECTION));
    }

    #[test]
    fn sr_text_via_non_edit_tool_takes_the_generic_path() {
        let mut t = RepetitionTracker::new();
        let sr = format!("{SR_KEY}. x.");
        assert!(t.error_correction("write_file", &no_args(), &sr).is_none());
        assert!(t.error_correction("write_file", &no_args(), &sr).is_none(), "전용 교정은 edit_file 한정 (§3-2 판정)");
        assert_eq!(t.error_correction("write_file", &no_args(), &sr), Some(EDIT_STRATEGY_CORRECTION), "3연속 일반 경로 불변");
    }

    #[test]
    fn sr_streak_resets_on_a_different_intervening_error_but_file_cumulative_persists() {
        // M9 시절 이 테스트는 "다른 오류가 끼면 완전히 리셋되어 3번째 호출도
        // 1회차 취급"을 검증했다. M12 §4-1은 그 사각(비연속 재발 미개입)을
        // 고치는 것이 목적이라, 파일별 누적은 성공 뮤테이션 외에는 리셋되지
        // 않는다 — 같은 파일이면 다른 종류의 오류가 껴도 누적 2 도달로 발화한다.
        // 연속 스트릭(sr_streak()) 자체는 여전히 리셋된다(기존 계약 유지) — 그
        // 부분만 옛 테스트와 동일하게 남긴다.
        let mut t = RepetitionTracker::new();
        let sr = format!("{SR_KEY}. x.");
        assert!(t.error_correction("edit_file", &args("a.rs"), &sr).is_none());
        t.error_correction("edit_file", &args("a.rs"), "Error: edit failed: search block not found. y");
        assert_eq!(t.sr_streak(), 0, "연속 스트릭은 다른 오류 키가 끼면 리셋된다 (기존 계약 유지)");
        assert_eq!(
            t.error_correction("edit_file", &args("a.rs"), &sr),
            Some(SR_CORRECTION),
            "파일별 누적은 성공 뮤테이션 외에는 리셋되지 않는다 — 같은 파일 재발은 비연속이어도 누적 2로 발화 (M12 §4-1)"
        );
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
    fn badargs_key_prefix_matches_actual_missing_field_errors_only() {
        // 도구 오류문(tools/mod.rs의 스키마 에코 경로)과 BADARGS_KEY_PREFIX의 드리프트를
        // 고정하는 교차 핀 (M12 §3-1). sr_key_matches_actual_edit_file_error_first_sentence를
        // 본떠, Registry::guided(false).dispatch로 실제 바디를 만들어 접두를 검증한다.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
        let ctx = crate::tools::ToolCtx::new(dir.path().to_path_buf());
        let reg = crate::tools::Registry::guided(false);

        // missing field — edit_file/write_file/run_command 전부 접두가 매치해야 한다
        for (tool, args) in [
            ("edit_file", serde_json::json!({"path": "f.rs"})),
            ("write_file", serde_json::json!({"path": "f.rs"})),
            ("run_command", serde_json::json!({})),
        ] {
            let err = reg.dispatch(tool, &args, &ctx).unwrap_err();
            let body = format!("Error: {err}");
            assert!(
                body.starts_with(BADARGS_KEY_PREFIX),
                "{tool}의 missing-field 바디가 접두와 어긋남: {body}"
            );
        }

        // invalid type — §3-1이 요구하는 배제: 스키마 에코 경로는 같지만 접두 불일치
        for (tool, args) in [
            ("edit_file", serde_json::json!({"path": 123, "search": "x", "replace": "y"})),
            ("write_file", serde_json::json!({"path": "f.rs", "content": 123})),
            ("run_command", serde_json::json!({"command": 123})),
        ] {
            let err = reg.dispatch(tool, &args, &ctx).unwrap_err();
            let body = format!("Error: {err}");
            assert!(
                !body.starts_with(BADARGS_KEY_PREFIX),
                "{tool}의 invalid-type 바디는 접두가 매치하면 안 됨(§3-1 배제): {body}"
            );
        }
    }

    #[test]
    fn sr_streak_tracks_consecutive_sr_errors_only() {
        let mut t = RepetitionTracker::new();
        let sr = format!("{SR_KEY}. x.");
        assert_eq!(t.sr_streak(), 0);
        t.error_correction("edit_file", &no_args(), &sr);
        assert_eq!(t.sr_streak(), 1);
        t.error_correction("edit_file", &no_args(), &sr);
        assert_eq!(t.sr_streak(), 2, "SR_CORRECTION 래치와 무관하게 스트릭은 계속 노출 (M10 §5 배선 주의)");
        t.error_correction("edit_file", &no_args(), &sr);
        assert_eq!(t.sr_streak(), 3);
        t.error_correction("edit_file", &no_args(), "ok result");
        assert_eq!(t.sr_streak(), 0, "비-S/R 결과로 리셋");
        t.error_correction("edit_file", &no_args(), "Error: edit failed: search block not found. y");
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

    // --- M12 §4-1: 파일별 S/R 누적 카운터 + missing-field 연속 카운터 ---

    const SR_BODY: &str = "Error: edit failed: search and replace are identical - no change would be made. Put the code as it is NOW in `search`.";
    const BADARGS_BODY: &str = "Error: invalid arguments: missing field `content`. Expected: write_file(path, content). You sent keys: [path, tool].";

    fn args(path: &str) -> serde_json::Value {
        serde_json::json!({"path": path})
    }

    #[test]
    fn non_consecutive_sr_on_the_same_file_still_fires_the_correction() {
        let mut t = RepetitionTracker::new();
        assert!(t.error_correction("edit_file", &args("src/a.rs"), SR_BODY).is_none(), "1회차는 도구 오류문이 처방");

        // T7 가드: 비-S/R 오류 바디 직후 last_sr_file이 즉시 풀리는지 (repetition.rs의
        // 비-S/R 분기 reset). 다른 파일/도구를 써서 src/a.rs의 누적(cum=1)은 건드리지
        // 않는다 — 이 줄이 사라지면 last_sr_file이 src/a.rs를 계속 가리켜
        // sr_file_streak()이 그 파일과 무관한 오류 후에도 값을 노출하고, T7의 섭동
        // 술어(sr_file_streak() >= 2)가 영구 참으로 걸릴 위험이 생긴다.
        assert!(t.error_correction("write_file", &args("other.rs"), "Error: x").is_none());
        assert_eq!(t.sr_file_streak(), 0, "비-S/R 오류 직후 last_sr_file 해제(T7 가드)");

        // 같은 가드를 성공(비-오류) 바디 경로에서도: b.rs로 last_sr_file을 재무장한 뒤
        // 성공 바디가 즉시 푸는지 확인 (a.rs/other.rs의 누적에는 영향 없음)
        assert!(t.error_correction("edit_file", &args("b.rs"), SR_BODY).is_none(), "b.rs 1회차도 누적 1 — 미발화");
        assert!(t.error_correction("read_file", &args("b.rs"), "fn main() {}").is_none());
        assert_eq!(t.sr_file_streak(), 0, "성공 바디 직후 last_sr_file 해제(T7 가드)");

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

    #[test]
    fn record_mutation_ok_on_an_untracked_file_is_a_harmless_noop() {
        // 한 번도 S/R 오류가 없던 파일에 성공 뮤테이션이 나도 패닉하거나
        // 다른 파일의 상태를 건드리지 않는다
        let mut t = RepetitionTracker::new();
        t.error_correction("edit_file", &args("a.rs"), SR_BODY);
        t.record_mutation_ok("never-touched.rs");
        assert_eq!(t.sr_file_streak(), 1, "무관 파일의 리셋은 a.rs 누적에 영향 없음");
    }

    #[test]
    fn record_mutation_ok_normalizes_the_path_like_error_correction_does() {
        // 파일 키는 status_note::normalize를 공유한다 — 표기만 다른 두 경로
        // (`./a.rs` vs `a.rs`)가 서로 다른 카운터로 갈라지면 리셋이 먹히지 않는다
        let mut t = RepetitionTracker::new();
        t.error_correction("edit_file", &args("./a.rs"), SR_BODY);
        t.error_correction("edit_file", &args("./a.rs"), SR_BODY);
        assert_eq!(t.sr_file_streak(), 2);
        t.record_mutation_ok("a.rs");
        assert_eq!(t.sr_file_streak(), 0, "정규화된 키가 같으므로 표기 변형이어도 리셋된다");
    }
}
