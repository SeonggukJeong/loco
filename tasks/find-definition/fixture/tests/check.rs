/// answer.txt 정규화 사다리 (M6 §3): trim → 감싼 따옴표쌍 제거 → 경로 정규화.
/// 흔한 형식 변형(따옴표·후행 슬래시·후행 마침표·역슬래시·./ 접두)은 맞는 답으로
/// 인정하고, 여러 줄·산문은 정규화하지 않는다 — "한 줄로 저장" 지시 불이행은
/// 판정기 협소가 아니라 모델 실패다
fn normalize_path_answer(raw: &str) -> String {
    let s = raw.trim();
    if s.lines().count() > 1 {
        return s.to_string(); // 여러 줄은 그대로 두어 불일치로 실패시킨다
    }
    let s = strip_matched_quotes(s);
    let s = s.replace('\\', "/");
    let s = s.trim_start_matches("./");
    let s = s.trim_end_matches('/');
    let s = s.trim_end_matches('.');
    s.to_string()
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
fn answer_names_the_defining_file() {
    let answer = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/answer.txt"))
        .expect("answer.txt가 없습니다");
    assert_eq!(normalize_path_answer(&answer), "src/geometry.rs");
}

// --- 사다리 자체의 단위 테스트 (M6 §3·§7) — 에이전트 산출물과 무관한 고정 케이스.
// check 실행 시 항상 함께 돌아 eval·verify 양쪽에서 사다리를 검증한다.
// 메타테스트는 정규형 솔루션만 보므로, 변형 허용·거부는 이 테스트만이 담보한다

#[test]
fn ladder_accepts_common_variants() {
    for raw in [
        "src/geometry.rs",
        "  src/geometry.rs\n",
        "\"src/geometry.rs\"",
        "'src/geometry.rs'",
        "`src/geometry.rs`",
        "./src/geometry.rs",
        "src\\geometry.rs",
        "src/geometry.rs/",
        "src/geometry.rs.",
    ] {
        assert_eq!(normalize_path_answer(raw), "src/geometry.rs", "입력: {raw:?}");
    }
}

#[test]
fn ladder_rejects_prose_multiline_and_wrong_path() {
    for raw in [
        "정답은 src/geometry.rs 입니다",
        "src/geometry.rs\n(area 함수가 여기 있음)",
        "src/text.rs",
    ] {
        assert_ne!(normalize_path_answer(raw), "src/geometry.rs", "입력: {raw:?}");
    }
}
