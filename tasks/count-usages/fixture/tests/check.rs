#[test]
fn answer_counts_call_sites() {
    let answer = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/answer.txt"))
        .expect("answer.txt가 없습니다");
    assert_eq!(answer.trim(), "4"); // title 1 + slug 1 + compare 2
}
