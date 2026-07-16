//! 보고 기간(period) 표현과 계산.
//!
//! 유닉스 epoch(초) 범위로 보고 기간을 표현하고, 기간 겹침/포함 판정,
//! 분기 계산 같은 보조 함수를 담는다. 달력 변환(연/월/일)까지는 하지
//! 않는다 — 필요하면 저장소 크레이트의 epoch 변환 헬퍼를 상위 계층에서
//! 함께 쓰는 것을 전제로 한다.

/// 하루의 초 수.
pub const SECONDS_PER_DAY: i64 = 86_400;

/// epoch 초 범위로 표현한 보고 기간(시작 포함, 끝 미포함).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodRange {
    pub start_epoch: i64,
    pub end_epoch: i64,
}

impl PeriodRange {
    /// 새 기간을 만든다. 끝이 시작보다 이르면 끝을 시작과 같게 보정한다
    /// (빈 기간으로 취급하기 위함).
    pub fn new(start_epoch: i64, end_epoch: i64) -> Self {
        PeriodRange { start_epoch, end_epoch: end_epoch.max(start_epoch) }
    }

    /// 기간의 길이(초)를 계산한다.
    pub fn duration_secs(&self) -> i64 {
        self.end_epoch - self.start_epoch
    }

    /// 기간의 길이(일)를 계산한다(내림).
    pub fn duration_days(&self) -> i64 {
        self.duration_secs() / SECONDS_PER_DAY
    }

    /// 특정 시각이 이 기간에 포함되는지 검사한다(끝은 미포함).
    pub fn contains(&self, epoch: i64) -> bool {
        epoch >= self.start_epoch && epoch < self.end_epoch
    }

    /// 다른 기간과 겹치는지 검사한다.
    pub fn overlaps(&self, other: &PeriodRange) -> bool {
        self.start_epoch < other.end_epoch && other.start_epoch < self.end_epoch
    }

    /// 기간이 비어 있는지(길이가 0인지) 검사한다.
    pub fn is_empty(&self) -> bool {
        self.duration_secs() == 0
    }
}

/// 두 기간의 교집합을 구한다. 겹치지 않으면 `None`.
pub fn intersection(a: &PeriodRange, b: &PeriodRange) -> Option<PeriodRange> {
    if !a.overlaps(b) {
        return None;
    }
    Some(PeriodRange::new(a.start_epoch.max(b.start_epoch), a.end_epoch.min(b.end_epoch)))
}

/// 기준 기간 바로 다음의, 같은 길이를 가진 기간을 계산한다(연속 구간 이동).
pub fn next_period(period: &PeriodRange) -> PeriodRange {
    let len = period.duration_secs();
    PeriodRange::new(period.end_epoch, period.end_epoch + len)
}

/// 기준 기간 바로 이전의, 같은 길이를 가진 기간을 계산한다.
pub fn previous_period(period: &PeriodRange) -> PeriodRange {
    let len = period.duration_secs();
    PeriodRange::new(period.start_epoch - len, period.start_epoch)
}

/// 월(1~12)로부터 회계 분기(1~4)를 계산한다(1~3월=1분기 관례).
pub fn quarter_of_month(month: u32) -> u32 {
    match month {
        1..=3 => 1,
        4..=6 => 2,
        7..=9 => 3,
        _ => 4,
    }
}

/// 분기가 성수기 분기(4분기, 연말)인지 판정한다.
pub fn is_peak_quarter(quarter: u32) -> bool {
    quarter == 4
}

/// 여러 기간을 시작 시각 오름차순으로 정렬한다.
pub fn sort_by_start(periods: &mut Vec<PeriodRange>) {
    periods.sort_by_key(|p| p.start_epoch);
}

/// 정렬된 기간 목록에 빈틈(gap)이 있는지 검사한다(연속 보고 기간의
/// 무결성 점검용 — 정렬되어 있다는 전제로 인접한 것끼리만 비교한다).
pub fn has_gaps(sorted_periods: &[PeriodRange]) -> bool {
    sorted_periods.windows(2).any(|w| w[0].end_epoch < w[1].start_epoch)
}

/// 여러 기간의 전체 길이(초) 합계를 구한다(겹침은 고려하지 않고 그대로 합산).
pub fn total_duration_secs(periods: &[PeriodRange]) -> i64 {
    periods.iter().map(|p| p.duration_secs()).sum()
}

/// 기간 목록 중 가장 긴 것을 찾는다.
pub fn longest_period(periods: &[PeriodRange]) -> Option<&PeriodRange> {
    periods.iter().max_by_key(|p| p.duration_secs())
}

/// 시작 시각과 일수로부터 기간을 만드는 편의 함수.
pub fn from_days(start_epoch: i64, days: i64) -> PeriodRange {
    PeriodRange::new(start_epoch, start_epoch + days * SECONDS_PER_DAY)
}

/// 기간을 지정한 일수 단위의 여러 하위 구간으로 균등 분할한다. 마지막
/// 구간은 남는 초를 모두 포함한다(경계가 딱 나누어떨어지지 않을 수 있음).
pub fn split_into_day_chunks(period: &PeriodRange, chunk_days: i64) -> Vec<PeriodRange> {
    if chunk_days <= 0 {
        return vec![*period];
    }
    let chunk_secs = chunk_days * SECONDS_PER_DAY;
    let mut chunks = Vec::new();
    let mut cursor = period.start_epoch;
    while cursor < period.end_epoch {
        let end = (cursor + chunk_secs).min(period.end_epoch);
        chunks.push(PeriodRange::new(cursor, end));
        cursor = end;
    }
    chunks
}

/// 기간이 다른 기간을 완전히 포함하는지(부분집합인지) 검사한다.
pub fn fully_contains(outer: &PeriodRange, inner: &PeriodRange) -> bool {
    outer.start_epoch <= inner.start_epoch && inner.end_epoch <= outer.end_epoch
}

/// 여러 기간 목록의 전체 범위(가장 이른 시작 ~ 가장 늦은 끝)를 계산한다.
/// 목록이 비어 있으면 `None`.
pub fn bounding_range(periods: &[PeriodRange]) -> Option<PeriodRange> {
    let start = periods.iter().map(|p| p.start_epoch).min()?;
    let end = periods.iter().map(|p| p.end_epoch).max()?;
    Some(PeriodRange::new(start, end))
}

/// 분기(1~4)에 대응하는 시작 월을 반환한다(1분기->1월, 2분기->4월, ...).
pub fn quarter_start_month(quarter: u32) -> u32 {
    match quarter {
        1 => 1,
        2 => 4,
        3 => 7,
        _ => 10,
    }
}

/// 두 기간이 인접해(맞닿아, 겹치지는 않고) 있는지 검사한다.
pub fn is_adjacent(a: &PeriodRange, b: &PeriodRange) -> bool {
    a.end_epoch == b.start_epoch || b.end_epoch == a.start_epoch
}

/// 기간 목록에서 지정 시각을 포함하는 기간을 찾는다.
pub fn find_containing(periods: &[PeriodRange], epoch: i64) -> Option<&PeriodRange> {
    periods.iter().find(|p| p.contains(epoch))
}
