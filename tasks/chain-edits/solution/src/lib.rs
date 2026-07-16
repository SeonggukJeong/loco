/// 재시도 상한
pub const MAX_RETRIES: u32 = 5;

/// 인사말
pub fn greeting() -> &'static str {
    "안녕하세요"
}

/// 재시도 대기시간(ms)
pub fn backoff_ms(attempt: u32) -> u64 {
    100 * 2u64.pow(attempt)
}
