//! 보충/배송 스케줄 헬퍼.
//!
//! 유닉스 epoch(초) 기반의 단순 날짜 산술만 다룬다 — 타임존/달력 라이브러리
//! 없이(외부 의존성 0 제약) 하루 = 86400초로 어림 계산한다.

const SECS_PER_DAY: i64 = 86_400;

/// 다음 보충 예정일을 계산한다: 마지막 보충일 + 주기(일).
pub fn next_restock_date(last_epoch: i64, interval_days: u32) -> i64 {
    last_epoch + (interval_days as i64) * SECS_PER_DAY
}

/// 두 시각 사이의 경과 일수(내림)를 계산한다.
pub fn days_between(from_epoch: i64, to_epoch: i64) -> i64 {
    (to_epoch - from_epoch) / SECS_PER_DAY
}

/// 예정일이 이미 지났는지(연체) 판정한다.
pub fn is_overdue(scheduled_epoch: i64, now_epoch: i64) -> bool {
    now_epoch > scheduled_epoch
}

/// 예정일까지 남은 일수를 계산한다(이미 지났으면 음수).
pub fn days_until(scheduled_epoch: i64, now_epoch: i64) -> i64 {
    days_between(now_epoch, scheduled_epoch)
}

/// 반복 스케줄의 다음 N회 발생 시각을 계산한다.
pub fn next_occurrences(start_epoch: i64, interval_days: u32, count: u32) -> Vec<i64> {
    (0..count).map(|i| start_epoch + (i as i64) * (interval_days as i64) * SECS_PER_DAY).collect()
}

/// 스케줄 주기(일)가 정책상 허용 범위(1~90일)인지 검사한다.
pub fn is_valid_interval(interval_days: u32) -> bool {
    interval_days >= 1 && interval_days <= 90
}

/// 두 스케줄이 같은 날(같은 86400초 구간)에 속하는지 비교한다.
pub fn is_same_day(epoch_a: i64, epoch_b: i64) -> bool {
    epoch_a / SECS_PER_DAY == epoch_b / SECS_PER_DAY
}

/// 배송 슬롯(0~23시)이 영업시간(9~18시) 안인지 검사한다.
pub fn is_within_business_hours(hour: u32) -> bool {
    (9..=18).contains(&hour)
}

/// 다가오는 예정 목록 중 지정된 일수 이내에 도래하는 것만 걸러낸다.
pub fn upcoming_within_days(scheduled: &[i64], now_epoch: i64, within_days: i64) -> Vec<i64> {
    scheduled
        .iter()
        .copied()
        .filter(|&s| s >= now_epoch && days_between(now_epoch, s) <= within_days)
        .collect()
}

/// 연체된(이미 지난) 예정만 걸러낸다.
pub fn overdue_entries(scheduled: &[i64], now_epoch: i64) -> Vec<i64> {
    scheduled.iter().copied().filter(|&s| is_overdue(s, now_epoch)).collect()
}

/// 두 스케줄 사이의 간격(일)이 최소 간격 이상인지 검사한다(연속 배송 방지).
pub fn respects_min_gap(a_epoch: i64, b_epoch: i64, min_gap_days: i64) -> bool {
    days_between(a_epoch, b_epoch).abs() >= min_gap_days
}

/// 예정 시각을 다음 영업일 09시로 반올림한다(단순화: 같은 날짜의 09시로
/// 맞추고, 이미 09시 이후라면 다음날 09시로 넘긴다).
pub fn snap_to_next_business_hour(epoch: i64) -> i64 {
    let day_start = (epoch / SECS_PER_DAY) * SECS_PER_DAY;
    let nine_am = day_start + 9 * 3600;
    if epoch <= nine_am {
        nine_am
    } else {
        nine_am + SECS_PER_DAY
    }
}

/// 스케줄 목록을 시각 오름차순으로 정렬한다.
pub fn sort_ascending(mut scheduled: Vec<i64>) -> Vec<i64> {
    scheduled.sort();
    scheduled
}

/// 가장 이른 예정 시각을 찾는다.
pub fn earliest(scheduled: &[i64]) -> Option<i64> {
    scheduled.iter().copied().min()
}

/// 가장 늦은 예정 시각을 찾는다.
pub fn latest(scheduled: &[i64]) -> Option<i64> {
    scheduled.iter().copied().max()
}

/// 요일(0=일요일 ~ 6=토요일)을 epoch로부터 대략 계산한다.
///
/// 1970-01-01(목요일, weekday=4)을 기준으로 어림 계산한다(윤초 등은 무시).
pub fn weekday_of(epoch: i64) -> u8 {
    let days_since_epoch = epoch / SECS_PER_DAY;
    (((days_since_epoch + 4) % 7 + 7) % 7) as u8
}

/// 주말(토/일)인지 판정한다.
pub fn is_weekend(epoch: i64) -> bool {
    matches!(weekday_of(epoch), 0 | 6)
}
