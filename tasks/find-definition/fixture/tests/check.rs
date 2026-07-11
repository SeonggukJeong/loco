#[test]
fn answer_names_the_defining_file() {
    let answer = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/answer.txt"))
        .expect("answer.txt가 없습니다");
    let normalized = answer.trim().replace('\\', "/");
    assert_eq!(normalized.trim_start_matches("./"), "src/geometry.rs");
}
