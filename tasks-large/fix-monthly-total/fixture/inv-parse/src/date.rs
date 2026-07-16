//! CSV에서 흔히 보이는 날짜 표기 여러 가지를 하나의 형태로 정규화한다.
//!
//! 벤더마다 `2024-11-03`, `20241103`, `2024/11/03` 세 가지 표기를 섞어
//! 보낸다. 이 모듈은 그 표기들을 파싱해 연/월/일로 나눈 뒤, 보고서에서는
//! 항상 ISO 형태(`YYYY-MM-DD`)로만 다루도록 정규화한다.

/// 파싱된 날짜(연/월/일). 달력 상 실존 여부(윤년 등)는 검증하지 않는다 —
/// 입력 데이터의 명백한 오타(월 13 등)만 걸러낸다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimpleDate {
    pub year: u32,
    pub month: u32,
    pub day: u32,
}

/// 날짜 문자열을 파싱한다. `-`, `/`, 구분자 없음(8자리 숫자) 세 형태를 지원한다.
pub fn parse_date(raw: &str) -> Option<SimpleDate> {
    let trimmed = raw.trim();
    if let Some(d) = parse_with_separator(trimmed, '-') {
        return Some(d);
    }
    if let Some(d) = parse_with_separator(trimmed, '/') {
        return Some(d);
    }
    parse_compact(trimmed)
}

fn parse_with_separator(s: &str, sep: char) -> Option<SimpleDate> {
    let parts: Vec<&str> = s.split(sep).collect();
    if parts.len() != 3 {
        return None;
    }
    let year = parts[0].parse::<u32>().ok()?;
    let month = parts[1].parse::<u32>().ok()?;
    let day = parts[2].parse::<u32>().ok()?;
    build_date(year, month, day)
}

fn parse_compact(s: &str) -> Option<SimpleDate> {
    if s.len() != 8 || !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let year = s[0..4].parse::<u32>().ok()?;
    let month = s[4..6].parse::<u32>().ok()?;
    let day = s[6..8].parse::<u32>().ok()?;
    build_date(year, month, day)
}

fn build_date(year: u32, month: u32, day: u32) -> Option<SimpleDate> {
    if is_valid_date(year, month, day) {
        Some(SimpleDate { year, month, day })
    } else {
        None
    }
}

/// 연/월/일이 명백히 잘못된 값이 아닌지 검사한다(윤년까지는 따지지 않음).
pub fn is_valid_date(year: u32, month: u32, day: u32) -> bool {
    (1..=9999).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// 날짜를 ISO 형태(`YYYY-MM-DD`)로 포맷한다.
pub fn format_iso(date: &SimpleDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}

/// 날짜를 압축 형태(`YYYYMMDD`)로 포맷한다.
pub fn format_compact(date: &SimpleDate) -> String {
    format!("{:04}{:02}{:02}", date.year, date.month, date.day)
}

/// `a`가 `b`보다 이전인지 비교한다.
pub fn is_before(a: &SimpleDate, b: &SimpleDate) -> bool {
    a < b
}

/// 두 날짜 목록에서 가장 이른 날짜를 찾는다.
pub fn earliest<'a>(dates: &'a [SimpleDate]) -> Option<&'a SimpleDate> {
    dates.iter().min()
}

/// 두 날짜 목록에서 가장 늦은 날짜를 찾는다.
pub fn latest<'a>(dates: &'a [SimpleDate]) -> Option<&'a SimpleDate> {
    dates.iter().max()
}

/// 대략적인 두 날짜 사이 일수를 어림한다(달력 정밀 계산이 아니라, 월을
/// 30일로 어림한 근사치 — 리포트의 "약 며칠 전" 표시 등 정밀도가
/// 필요없는 곳에서만 쓴다).
pub fn approx_days_between(a: &SimpleDate, b: &SimpleDate) -> i64 {
    let a_total = a.year as i64 * 360 + a.month as i64 * 30 + a.day as i64;
    let b_total = b.year as i64 * 360 + b.month as i64 * 30 + b.day as i64;
    (b_total - a_total).abs()
}

/// 날짜가 주어진 연도에 속하는지 검사한다.
pub fn is_in_year(date: &SimpleDate, year: u32) -> bool {
    date.year == year
}

/// 날짜 목록을 연도별로 센다(연도 -> 개수, 연도 오름차순).
pub fn count_by_year(dates: &[SimpleDate]) -> Vec<(u32, usize)> {
    let mut years: Vec<u32> = dates.iter().map(|d| d.year).collect();
    years.sort();
    years.dedup();
    years
        .into_iter()
        .map(|y| (y, dates.iter().filter(|d| d.year == y).count()))
        .collect()
}

/// 날짜 문자열이 세 형태(하이픈/슬래시/압축) 중 어느 것도 아닌지 검사한다.
pub fn is_unparseable(raw: &str) -> bool {
    parse_date(raw).is_none()
}

/// 날짜 문자열 목록 중 파싱 가능한 것만 걸러 정규화된 ISO 문자열로 반환한다.
pub fn normalize_all(raws: &[String]) -> Vec<String> {
    raws.iter().filter_map(|r| parse_date(r)).map(|d| format_iso(&d)).collect()
}

/// 날짜가 주어진 [from, to] 구간(양 끝 포함)에 속하는지 검사한다.
pub fn is_within_range(date: &SimpleDate, from: &SimpleDate, to: &SimpleDate) -> bool {
    date >= from && date <= to
}

/// 날짜 목록에서 특정 구간에 속하는 것만 걸러낸다.
pub fn filter_within_range(dates: &[SimpleDate], from: &SimpleDate, to: &SimpleDate) -> Vec<SimpleDate> {
    dates.iter().filter(|d| is_within_range(d, from, to)).copied().collect()
}

/// 날짜의 분기(1~4)를 계산한다.
pub fn quarter_of(date: &SimpleDate) -> u32 {
    (date.month - 1) / 3 + 1
}

/// 날짜가 월말(28~31일 사이의 마지막 날)에 가까운지 대략적으로 판정한다
/// (정확한 말일 계산 없이, 28일 이상이면 월말권으로 본다 — 정산 마감 임박
/// 경고 등 정밀도가 필요 없는 곳에서 쓴다).
pub fn is_near_month_end(date: &SimpleDate) -> bool {
    date.day >= 28
}

/// 날짜 목록을 오름차순으로 정렬한다.
pub fn sort_ascending(dates: &mut Vec<SimpleDate>) {
    dates.sort();
}

/// 두 날짜가 같은 월에 속하는지 비교한다.
pub fn is_same_month(a: &SimpleDate, b: &SimpleDate) -> bool {
    a.year == b.year && a.month == b.month
}

/// 날짜에서 다음 날짜(달력 정밀 계산 없이, 단순히 day+1 — 월말 롤오버는
/// 다루지 않는 어림 버전. 리포트의 "다음 영업일 어림" 등에만 쓴다)를 만든다.
pub fn naive_next_day(date: &SimpleDate) -> SimpleDate {
    SimpleDate { year: date.year, month: date.month, day: date.day + 1 }
}

/// 날짜 목록에서 중복(정확히 같은 연/월/일)을 제거한다.
pub fn dedup_dates(dates: &[SimpleDate]) -> Vec<SimpleDate> {
    let mut sorted = dates.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted
}

/// 날짜가 주말에 해당하는 요일인지는 알 수 없지만(요일 계산은 이 크레이트
/// 범위 밖), 적어도 월/일이 특정 공휴일 후보(1월 1일, 12월 25일 등)와
/// 겹치는지는 간단히 검사할 수 있다.
pub fn is_common_holiday_date(date: &SimpleDate) -> bool {
    matches!((date.month, date.day), (1, 1) | (12, 25) | (5, 5) | (8, 15))
}

/// 날짜 문자열 목록에서 파싱 실패한 원본만 걸러낸다(오류 리포트용).
pub fn unparseable_entries(raws: &[String]) -> Vec<String> {
    raws.iter().filter(|r| is_unparseable(r)).cloned().collect()
}

/// SimpleDate를 슬래시 구분 표기("2024/11/03")로 포맷한다.
pub fn format_slash(date: &SimpleDate) -> String {
    format!("{:04}/{:02}/{:02}", date.year, date.month, date.day)
}
