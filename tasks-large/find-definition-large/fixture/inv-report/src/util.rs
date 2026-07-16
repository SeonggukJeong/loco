//! 보고서 출력에서 반복적으로 쓰는 소소한 포맷팅 헬퍼 모음.
//!
//! 통화 표기, 백분율 문자열, 표 컬럼 정렬 등 보고서 텍스트를 만들 때마다
//! 필요한 자잘한 함수들을 모아둔다.

/// 금액(원)을 천 단위 구분 쉼표를 넣어 포맷한다("1,234,000").
pub fn format_krw_with_commas(value: i64) -> String {
    let negative = value < 0;
    let digits = value.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().rev().enumerate() {
        if i != 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let mut result: String = out.chars().rev().collect();
    if negative {
        result.insert(0, '-');
    }
    result
}

/// 백분율 값을 부호 포함 문자열로 포맷한다("+12%", "-5%").
pub fn format_signed_percent(value: i64) -> String {
    if value > 0 {
        format!("+{value}%")
    } else {
        format!("{value}%")
    }
}

/// 문자열을 지정한 너비로 오른쪽 정렬한다(보고서 표의 금액 컬럼용).
pub fn pad_left(s: &str, width: usize) -> String {
    if s.chars().count() >= width {
        s.to_string()
    } else {
        let padding = " ".repeat(width - s.chars().count());
        format!("{padding}{s}")
    }
}

/// 문자열을 지정한 너비로 왼쪽 정렬한다(보고서 표의 라벨 컬럼용).
pub fn pad_right(s: &str, width: usize) -> String {
    if s.chars().count() >= width {
        s.to_string()
    } else {
        let padding = " ".repeat(width - s.chars().count());
        format!("{s}{padding}")
    }
}

/// 여러 문자열 중 최대 길이를 구한다(표 컬럼 너비를 자동으로 맞출 때 쓴다).
pub fn max_width(items: &[String]) -> usize {
    items.iter().map(|s| s.chars().count()).max().unwrap_or(0)
}

/// 값이 0이면 대체 텍스트를, 아니면 값 자체를 문자열로 반환한다(표에서
/// 0원을 굳이 "0"으로 찍지 않고 "-"로 표시하는 관례에 쓰인다).
pub fn zero_as_dash(value: i64) -> String {
    if value == 0 {
        "-".to_string()
    } else {
        value.to_string()
    }
}

/// 백분율 값을 [0, 100] 범위로 clamp한다(표시 전 방어적 보정).
pub fn clamp_percent(value: i64) -> i64 {
    value.clamp(0, 100)
}

/// 긴 라벨을 지정 길이로 잘라 말줄임표를 붙인다(좁은 컬럼에 긴 SKU명이
/// 들어갈 때 쓴다).
pub fn truncate_label(label: &str, max_chars: usize) -> String {
    if label.chars().count() <= max_chars {
        label.to_string()
    } else {
        let truncated: String = label.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// 문자열 목록을 지정한 구분자로 이어붙이되, 빈 문자열은 건너뛴다.
pub fn join_non_empty(items: &[String], sep: &str) -> String {
    items.iter().filter(|s| !s.trim().is_empty()).cloned().collect::<Vec<_>>().join(sep)
}

/// 두 문자열 중 더 긴 쪽을 반환한다(길이 동률이면 첫 번째).
pub fn longer_of<'a>(a: &'a str, b: &'a str) -> &'a str {
    if b.chars().count() > a.chars().count() {
        b
    } else {
        a
    }
}

/// 값이 임계값 이상이면 강조 표시("**값**")를 붙인다(고액 항목 강조용).
pub fn emphasize_if_over(value: i64, threshold_krw: i64) -> String {
    if value >= threshold_krw {
        format!("**{value}**")
    } else {
        value.to_string()
    }
}

/// 문자열 목록을 번호를 매겨("1. ", "2. " ...) 여러 줄 텍스트로 만든다.
pub fn numbered_list(items: &[String]) -> String {
    items.iter().enumerate().map(|(i, s)| format!("{}. {s}", i + 1)).collect::<Vec<_>>().join("\n")
}

/// 두 값 중 절대값이 더 큰 쪽을 반환한다(등락폭 비교 등에 쓰인다).
pub fn larger_magnitude(a: i64, b: i64) -> i64 {
    if b.abs() > a.abs() {
        b
    } else {
        a
    }
}

/// 백분율 값을 소수점 한 자리까지 포맷한다(정수 스케일 값을 받아
/// `value/scale`로 나눠 표시 — 예: `format_percent_1dp(125, 10)` -> "12.5%").
pub fn format_percent_1dp(value: i64, scale: i64) -> String {
    if scale == 0 {
        return "0.0%".to_string();
    }
    let tenths = (value * 10) / scale;
    format!("{}.{}%", tenths / 10, (tenths % 10).abs())
}
