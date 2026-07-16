//! 숫자 필드 파싱 보조 — 천 단위 구분자, 단위 접미사 등.
//!
//! 원 단위 금액을 사람이 읽기 좋게 "1,234,500"처럼 천 단위 구분자를 넣어
//! 보내는 벤더가 있다(따옴표로 감싸져 있어 CSV 구분자와는 충돌하지
//! 않는다). `csv::parse_row`는 순수 숫자만 처리하므로, 이런 변형 표기는
//! 이 모듈에서 먼저 정규화한 뒤 넘긴다.

/// 천 단위 구분자(쉼표)를 제거한다.
pub fn strip_thousands_separators(s: &str) -> String {
    s.chars().filter(|c| *c != ',').collect()
}

/// "1,234,500" 형태의 금액 문자열을 정수로 파싱한다.
pub fn parse_krw_amount(raw: &str) -> Option<i64> {
    strip_thousands_separators(raw.trim()).parse::<i64>().ok()
}

/// 수량 뒤에 단위 접미사(EA/BOX 등)가 붙은 표기("10EA")를 숫자만 뽑아 파싱한다.
pub fn parse_qty_with_unit(raw: &str) -> Option<(i64, String)> {
    let trimmed = raw.trim();
    let split_at = trimmed.find(|c: char| !c.is_ascii_digit() && c != '-')?;
    if split_at == 0 {
        return None;
    }
    let (num_part, unit_part) = trimmed.split_at(split_at);
    let qty = num_part.parse::<i64>().ok()?;
    Some((qty, unit_part.trim().to_string()))
}

/// 백분율 문자열("15%" 또는 "15")을 정수로 파싱한다.
pub fn parse_percent(raw: &str) -> Option<u32> {
    raw.trim().trim_end_matches('%').parse::<u32>().ok()
}

/// 값을 지정한 배수로 반올림한다(0이면 원래 값 그대로).
pub fn round_to_multiple(value: i64, multiple: i64) -> i64 {
    if multiple == 0 {
        return value;
    }
    let half = multiple / 2;
    ((value + half) / multiple) * multiple
}

/// 값을 [min, max] 구간으로 자른다.
pub fn clamp_i64(value: i64, min: i64, max: i64) -> i64 {
    value.clamp(min, max)
}

/// 숫자 문자열에 부호가 있는지(음수 표기인지) 검사한다.
pub fn is_negative_literal(raw: &str) -> bool {
    raw.trim().starts_with('-')
}

/// 정수를 천 단위 구분자를 넣은 문자열로 포맷한다(재출력/로그용).
pub fn format_with_thousands(value: i64) -> String {
    let negative = value < 0;
    let digits = value.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().rev().enumerate() {
        if i != 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let reversed: String = grouped.chars().rev().collect();
    if negative {
        format!("-{reversed}")
    } else {
        reversed
    }
}

/// 두 정수의 평균을 정수로 어림한다(내림).
pub fn average_i64(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    values.iter().sum::<i64>() / values.len() as i64
}

/// 값이 허용 오차 범위 안에서 같은지 비교한다.
pub fn approximately_equal(a: i64, b: i64, tolerance: i64) -> bool {
    (a - b).abs() <= tolerance
}

/// 문자열이 순수 숫자(부호 없음)로만 이루어졌는지 검사한다.
pub fn is_plain_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// 문자열에서 숫자가 아닌 문자를 모두 제거한다(단위/통화 기호가 섞인
/// 필드에서 숫자만 뽑아낼 때 쓰는 거친 버전 — 부호는 보존하지 않는다).
pub fn digits_only(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// 값이 0인지 검사하는 짧은 헬퍼(가독성을 위해 조건식 대신 이름 붙임).
pub fn is_zero(value: i64) -> bool {
    value == 0
}

/// 두 값의 차이를 백분율로 계산한다(기준값이 0이면 0을 반환 — 0으로
/// 나누기 방지).
pub fn percent_difference(base: i64, other: i64) -> i64 {
    if base == 0 {
        return 0;
    }
    ((other - base) * 100) / base
}

/// 값 목록의 최솟값/최댓값을 함께 구한다(빈 목록이면 `None`).
pub fn min_max(values: &[i64]) -> Option<(i64, i64)> {
    if values.is_empty() {
        return None;
    }
    let min = *values.iter().min().unwrap();
    let max = *values.iter().max().unwrap();
    Some((min, max))
}

/// 값이 [min, max] 범위 밖인지(이상치인지) 검사한다.
pub fn is_out_of_bounds(value: i64, min: i64, max: i64) -> bool {
    value < min || value > max
}

/// 숫자 문자열 목록을 정수로 일괄 파싱하되, 실패한 항목은 건너뛴다.
pub fn parse_all_valid(raws: &[String]) -> Vec<i64> {
    raws.iter().filter_map(|r| r.trim().parse::<i64>().ok()).collect()
}

/// 값 목록의 중앙값을 계산한다(빈 목록이면 0, 짝수 개면 가운데 두 값의
/// 평균을 내림).
pub fn median_i64(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2
    } else {
        sorted[mid]
    }
}

/// 값이 [expected - tolerance_pct%, expected + tolerance_pct%] 범위 안인지 검사한다.
pub fn within_percent_tolerance(value: i64, expected: i64, tolerance_pct: u32) -> bool {
    if expected == 0 {
        return value == 0;
    }
    let allowed = (expected.abs() * tolerance_pct as i64) / 100;
    (value - expected).abs() <= allowed
}

/// 값을 지정된 자리수(10의 거듭제곱)로 내림한다(예: floor_to_power(1234, 100) -> 1200).
pub fn floor_to_power(value: i64, power_of_ten: i64) -> i64 {
    if power_of_ten <= 0 {
        return value;
    }
    (value / power_of_ten) * power_of_ten
}

/// 문자열이 유효한 정수 리터럴(선택적 부호 + 숫자)인지 검사한다.
pub fn is_integer_literal(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    let body = t.strip_prefix(['-', '+']).unwrap_or(t);
    !body.is_empty() && body.chars().all(|c| c.is_ascii_digit())
}
