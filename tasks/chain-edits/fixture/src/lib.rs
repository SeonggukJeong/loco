/// 재시도 상한
pub const MAX_RETRIES: u32 = 3;

/// 인사말
pub fn greeting() -> &'static str {
    "Hello"
}

/// 재시도 대기시간(ms)
pub fn backoff_ms(attempt: u32) -> u64 {
    (attempt as u64) * 100
}
