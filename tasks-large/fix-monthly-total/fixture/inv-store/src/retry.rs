//! 저장소 연산 재시도 정책.
//!
//! 락 경합이나 일시적인 저장소 오류(파일 잠금 실패 등) 발생 시 몇 번까지
//! 재시도할지 정하는 상한과, 재시도 여부/백오프를 계산하는 헬퍼를 담는다.

/// 저장소 쓰기 연산의 최대 재시도 횟수.
pub const RETRY_LIMIT: u32 = 3;

/// 재시도 사이 기본 대기 시간(밀리초). 시도 횟수에 비례해 늘어난다
/// (`backoff_ms`가 실제 계산식).
pub const BASE_BACKOFF_MS: u64 = 50;

/// 현재 시도 횟수(1부터 시작)가 재시도 한도를 넘었는지 판정한다.
pub fn is_exhausted(attempt: u32) -> bool {
    attempt >= RETRY_LIMIT
}

/// 시도 횟수에 따른 대기 시간(밀리초)을 계산한다(선형 증가 — 지수 백오프
/// 만큼 공격적이지 않아도 되는 짧은 락 경합 상황을 가정한다).
pub fn backoff_ms(attempt: u32) -> u64 {
    BASE_BACKOFF_MS * attempt as u64
}

/// 재시도 상태를 추적하는 작은 카운터.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RetryState {
    attempts: u32,
}

impl RetryState {
    pub fn new() -> Self {
        RetryState::default()
    }

    /// 시도 횟수를 1 늘리고, 갱신된 값을 반환한다.
    pub fn record_attempt(&mut self) -> u32 {
        self.attempts += 1;
        self.attempts
    }

    /// 현재까지의 시도 횟수.
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// 재시도를 더 해도 되는지(한도를 넘지 않았는지) 여부.
    pub fn can_retry(&self) -> bool {
        !is_exhausted(self.attempts)
    }

    /// 다음 재시도까지 기다려야 할 시간(밀리초).
    pub fn next_backoff_ms(&self) -> u64 {
        backoff_ms(self.attempts.max(1))
    }

    /// 카운터를 초기화한다(연산이 결국 성공했을 때 호출).
    pub fn reset(&mut self) {
        self.attempts = 0;
    }
}

/// 재시도 가능한 오류인지(일시적 오류인지) 판정하는 데 쓰는 오류 분류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryClass {
    Transient,
    Permanent,
}

/// 오류 메시지 문자열로부터 대략적인 재시도 분류를 추정한다. 락/타임아웃
/// 관련 키워드는 일시적 오류로, 그 외는 영구 오류로 본다.
pub fn classify_error_message(message: &str) -> RetryClass {
    let lower = message.to_ascii_lowercase();
    if lower.contains("lock") || lower.contains("timeout") || lower.contains("busy") {
        RetryClass::Transient
    } else {
        RetryClass::Permanent
    }
}

/// 오류 분류와 현재 시도 횟수를 함께 고려해 재시도해야 하는지 최종 판정한다.
pub fn should_retry(class: RetryClass, attempt: u32) -> bool {
    matches!(class, RetryClass::Transient) && !is_exhausted(attempt)
}

/// 재시도 이력을 담는 로그 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryLogEntry {
    pub attempt: u32,
    pub class: RetryClass,
    pub message: String,
}

/// 재시도 로그에서 마지막으로 영구 오류(재시도 불가)가 발생했는지 검사한다.
pub fn ended_in_permanent_failure(log: &[RetryLogEntry]) -> bool {
    matches!(log.last(), Some(entry) if matches!(entry.class, RetryClass::Permanent))
}

/// 재시도 로그의 총 소요 시간(밀리초)을 어림한다(각 시도 사이의 백오프
/// 합산 — 실제 연산 시간은 포함하지 않는다).
pub fn estimated_total_backoff_ms(log: &[RetryLogEntry]) -> u64 {
    log.iter().map(|e| backoff_ms(e.attempt)).sum()
}

/// 재시도 로그에서 일시적 오류만 걸러낸다.
pub fn transient_entries(log: &[RetryLogEntry]) -> Vec<RetryLogEntry> {
    log.iter().filter(|e| matches!(e.class, RetryClass::Transient)).cloned().collect()
}

/// 최대 재시도 횟수를 초과하지 않는 범위에서, 남은 재시도 가능 횟수를 계산한다.
pub fn remaining_attempts(current_attempt: u32) -> u32 {
    RETRY_LIMIT.saturating_sub(current_attempt)
}

/// 재시도 로그를 사람이 읽는 한 줄 요약으로 만든다.
pub fn summarize_log(log: &[RetryLogEntry]) -> String {
    format!("총 {}회 시도, 최종 상태: {}", log.len(), if ended_in_permanent_failure(log) { "영구 실패" } else { "일시 오류/성공" })
}

/// 재시도 로그에서 연속된 일시적 오류(Transient)가 몇 번 이어졌는지
/// 가장 긴 연속 구간의 길이를 구한다.
pub fn longest_transient_streak(log: &[RetryLogEntry]) -> usize {
    let mut longest = 0usize;
    let mut current = 0usize;
    for entry in log {
        if matches!(entry.class, RetryClass::Transient) {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

/// 시도 횟수가 재시도 한도의 절반을 넘었는지(경고 신호) 판정한다.
pub fn is_past_halfway(attempt: u32) -> bool {
    attempt * 2 >= RETRY_LIMIT
}

/// 재시도 로그 목록에서 가장 오래(가장 많이) 재시도한 항목의 시도 횟수를 찾는다.
pub fn max_attempts_seen(logs: &[Vec<RetryLogEntry>]) -> u32 {
    logs.iter().map(|l| l.len() as u32).max().unwrap_or(0)
}
