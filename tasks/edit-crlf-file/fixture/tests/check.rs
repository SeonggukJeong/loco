#[test]
fn greeting_is_updated_and_still_crlf() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/data/greeting.txt")).unwrap();
    let text = String::from_utf8(bytes).expect("UTF-8 유지");
    assert!(text.contains("hello loco"), "단어가 교체돼야 함: {text:?}");
    assert!(!text.contains("world"), "원래 단어가 남아있음: {text:?}");
    assert!(text.contains("\r\n"), "CRLF 줄바꿈이 보존돼야 함: {text:?}");
}
