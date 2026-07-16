/// answer.txt 정규화 사다리 (M6 §3): trim → 감싼 따옴표쌍 제거 → 정수 파싱·수치 비교.
/// 산문("4회")·여러 줄은 파싱 실패(None)로 남긴다 — 지시 불이행은 모델 실패다
fn parse_int_answer(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.lines().count() > 1 {
        return None;
    }
    strip_matched_quotes(s).parse().ok()
}

/// 같은 따옴표(" ' `)로 감싼 경우에만 한 겹 벗긴다
fn strip_matched_quotes(s: &str) -> &str {
    for q in ['"', '\'', '`'] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return &s[1..s.len() - 1];
        }
    }
    s
}

#[test]
fn answer_counts_call_sites() {
    let answer = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/answer.txt"))
        .expect("answer.txt가 없습니다");
    // title 1 + slug 1 + compare 2 (pub use 선언은 호출이 아님)
    assert_eq!(parse_int_answer(&answer), Some(4));
}

// --- 사다리 자체의 단위 테스트 (M6 §3·§7) — find-definition의 것과 같은 취지

#[test]
fn ladder_accepts_common_variants() {
    for raw in ["4", " 4\n", "\"4\"", "'4'", "`4`", "04"] {
        assert_eq!(parse_int_answer(raw), Some(4), "입력: {raw:?}");
    }
}

#[test]
fn ladder_rejects_prose_and_multiline() {
    for raw in ["4회", "호출은 4번", "4\n(설명)", ""] {
        assert_eq!(parse_int_answer(raw), None, "입력: {raw:?}");
    }
}
