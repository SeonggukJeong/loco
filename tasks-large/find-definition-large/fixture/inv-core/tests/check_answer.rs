//! 과제 판정: answer.txt가 restock_threshold의 실제 정의 파일을 가리키는가.
//! 정규화 사다리는 M6 §3 관례(트림 → 짝 따옴표 제거 → 경로 정규화)의 이식이며
//! 같은 파일 안에서 자기시험한다.
use std::fs;

const TARGET: &str = "inv-core/src/rules/mod.rs";

// M6 §3 사다리와 동일하게 따옴표 3종("·'·`)을 벗기고 후행 마침표를 제거한다
// (기존 tasks/find-definition 사다리 이식 — 백틱·마침표 누락은 거짓 실패 재도입).
fn strip_quotes(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2 {
        let (f, l) = (b[0], b[b.len() - 1]);
        if (f == b'"' && l == b'"') || (f == b'\'' && l == b'\'') || (f == b'`' && l == b'`') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

fn normalize(raw: &str) -> String {
    let t = strip_quotes(raw.trim());
    let t = t.trim_end_matches('.');
    let t = t.replace('\\', "/");
    let t = t.strip_prefix("./").unwrap_or(&t);
    t.trim_end_matches('/').to_string()
}

fn matches_target(raw: &str) -> bool {
    let n = normalize(raw);
    n == TARGET || n.ends_with(&format!("/{TARGET}"))
}

#[test]
fn answer_names_definition_file() {
    let raw = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../answer.txt"))
        .expect("answer.txt가 프로젝트 루트에 없다");
    assert!(matches_target(&raw), "정의 파일 경로가 아니다: {raw:?}");
}

#[test]
fn ladder_accepts_variants() {
    for ok in [
        "inv-core/src/rules/mod.rs",
        "./inv-core/src/rules/mod.rs",
        "\"inv-core/src/rules/mod.rs\"",
        "inv-core\\src\\rules\\mod.rs",
        "  inv-core/src/rules/mod.rs\n",
        "/sandbox/proj/inv-core/src/rules/mod.rs",
        "`inv-core/src/rules/mod.rs`",
        "inv-core/src/rules/mod.rs.",
    ] {
        assert!(matches_target(ok), "수용해야 함: {ok:?}");
    }
}

#[test]
fn ladder_rejects_wrong_paths() {
    for bad in [
        "inv-core/src/inventory.rs",        // 재수출 지점
        "inv-store/tests/support/mod.rs",   // 테스트 헬퍼
        "inv-store/src/legacy_import.rs",   // 유사 사본
        "inv-core/src/rules",
    ] {
        assert!(!matches_target(bad), "기각해야 함: {bad:?}");
    }
}
