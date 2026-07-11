use chain_edits::{backoff_ms, greeting, MAX_RETRIES};

#[test]
fn retries_raised() {
    assert_eq!(MAX_RETRIES, 5);
}

#[test]
fn korean_greeting() {
    assert_eq!(greeting(), "안녕하세요");
}

#[test]
fn exponential_backoff() {
    assert_eq!(backoff_ms(0), 100);
    assert_eq!(backoff_ms(1), 200);
    assert_eq!(backoff_ms(3), 800);
}
