//! 통화 표기/파싱 헬퍼(주로 KRW 대상).
//!
//! 세율/할인 계산 자체는 `rules::pricing`에 있다 — 이 파일은 순수하게
//! "화면/보고서에 어떻게 찍히는가", "문자열을 어떻게 숫자로 되돌리는가"만
//! 다룬다.

/// 원화 금액을 천 단위 콤마가 있는 문자열로 포맷한다("1,234,000").
pub fn format_krw(amount: i64) -> String {
    let negative = amount < 0;
    let digits = amount.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().rev().enumerate() {
        if i != 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let grouped: String = grouped.chars().rev().collect();
    if negative {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// 콤마 포함 금액 문자열을 정수로 파싱한다("1,234,000" -> 1234000).
pub fn parse_krw(s: &str) -> Option<i64> {
    let cleaned: String = s.chars().filter(|c| *c != ',').collect();
    cleaned.trim().parse::<i64>().ok()
}

/// 원화 표기에 "원" 단위를 붙인다.
pub fn with_won_suffix(amount: i64) -> String {
    format!("{}원", format_krw(amount))
}

/// 금액을 지정한 단위(예: 10원)로 반올림한다.
pub fn round_to_unit(amount: i64, unit: i64) -> i64 {
    if unit <= 0 {
        return amount;
    }
    let half = unit / 2;
    ((amount + half) / unit) * unit
}

/// 금액을 지정한 단위로 내림한다(절사).
pub fn floor_to_unit(amount: i64, unit: i64) -> i64 {
    if unit <= 0 {
        return amount;
    }
    (amount / unit) * unit
}

/// 통화 코드가 3자리 대문자 형식(ISO 4217 스타일)인지 검사한다.
pub fn is_valid_currency_code(code: &str) -> bool {
    code.len() == 3 && code.chars().all(|c| c.is_ascii_uppercase())
}

/// 두 금액의 차이의 절댓값을 계산한다(음수 금액도 지원).
pub fn abs_diff(a: i64, b: i64) -> i64 {
    (a - b).abs()
}

/// 금액이 지정된 범위 안에 있는지 검사한다.
pub fn is_within_amount_range(amount: i64, min: i64, max: i64) -> bool {
    amount >= min && amount <= max
}

/// 여러 통화 코드 중 워크스페이스가 지원하는 것만 걸러낸다.
pub fn filter_supported_codes(codes: &[String]) -> Vec<String> {
    codes.iter().filter(|c| is_valid_currency_code(c)).cloned().collect()
}

/// 대략적인 환율(고정 테이블, 실시간 조회 없음 — 오프라인 픽스처용 근사치)로
/// KRW 금액을 다른 통화로 환산한다.
pub fn approx_convert_from_krw(amount_krw: i64, target_code: &str) -> Option<i64> {
    let rate = match target_code {
        "USD" => 0.00075,
        "JPY" => 0.11,
        "KRW" => 1.0,
        _ => return None,
    };
    Some((amount_krw as f64 * rate) as i64)
}

/// 금액 목록의 합계를 계산한다(오버플로 방지를 위해 saturating 연산 사용).
pub fn sum_amounts(amounts: &[i64]) -> i64 {
    amounts.iter().fold(0i64, |acc, a| acc.saturating_add(*a))
}

/// 금액이 "0원"인지(정확히 0) 검사한다.
pub fn is_zero(amount: i64) -> bool {
    amount == 0
}

/// 금액에 퍼센트 가산(할증)을 적용한다. 예: 10% 할증 -> amount * 110 / 100.
pub fn apply_surcharge_percent(amount_krw: i64, surcharge_percent: u32) -> i64 {
    amount_krw + (amount_krw * surcharge_percent as i64 / 100)
}

/// 금액 문자열이 순수 숫자(콤마/공백 제외 후) 형식인지 사전 검사한다.
pub fn looks_like_amount(s: &str) -> bool {
    let cleaned: String = s.chars().filter(|c| *c != ',' && !c.is_whitespace()).collect();
    !cleaned.is_empty() && cleaned.chars().all(|c| c.is_ascii_digit() || c == '-')
}
