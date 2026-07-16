//! 문자열/숫자 공용 헬퍼.
//!
//! 함정4: 이 파일과 같은 이름(`util.rs`)의 파일이 다른 inv-* 크레이트에도
//! 존재한다(각 크레이트가 자기 도메인에 맞는 헬퍼를 따로 둔다 — 서로
//! import하지 않는다). 파일명만으로 "그 util.rs"를 찾으려 하면 크레이트
//! 접두 없이는 어느 크레이트의 것인지 특정할 수 없다.

/// 앞뒤 공백을 제거하고, 빈 문자열이면 `None`을 반환한다.
pub fn non_blank(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// 문자열이 공백만으로 이루어졌는지(빈 문자열 포함) 검사한다.
pub fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

/// 정수 파싱, 실패 시 기본값을 반환한다.
pub fn parse_i64_or(s: &str, default: i64) -> i64 {
    s.trim().parse::<i64>().unwrap_or(default)
}

/// 부호 없는 정수 파싱, 실패 시 기본값을 반환한다.
pub fn parse_u32_or(s: &str, default: u32) -> u32 {
    s.trim().parse::<u32>().unwrap_or(default)
}

/// 값을 [min, max] 구간으로 자른다(i64 버전).
pub fn clamp_i64(value: i64, min: i64, max: i64) -> i64 {
    value.clamp(min, max)
}

/// 문자열을 지정된 길이로 자르고, 잘렸으면 말줄임표를 붙인다.
pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{truncated}...")
    }
}

/// 여러 공백 문자를 하나로 압축한다("a   b" -> "a b").
pub fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 문자열이 숫자로만 이루어졌는지 검사한다(부호 없음).
pub fn is_digits_only(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// 카멜케이스 문자열을 스네이크케이스로 변환한다("skuCode" -> "sku_code").
pub fn camel_to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// 두 문자열을 대소문자 무시하고 비교한다.
pub fn eq_ignore_case(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// 리스트를 지정된 구분자로 조인하되, 빈 항목은 걸러낸다.
pub fn join_non_empty(items: &[String], sep: &str) -> String {
    items.iter().filter(|s| !s.trim().is_empty()).cloned().collect::<Vec<_>>().join(sep)
}

/// 백분율(0~100 범위로 clamp)을 문자열로 포맷한다("42%").
pub fn format_percent(value: i64) -> String {
    format!("{}%", value.clamp(0, 100))
}

/// 문자열이 주어진 접두사 중 하나로 시작하는지 검사한다.
pub fn starts_with_any(s: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|p| s.starts_with(p))
}

/// 리스트에서 중복을 제거하되 원래 등장 순서를 유지한다(정렬하지 않음).
pub fn dedup_preserve_order(items: &[String]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for item in items {
        if !seen.contains(item) {
            seen.push(item.clone());
        }
    }
    seen
}

/// 두 슬라이스가 순서 무관하게 같은 원소 집합을 갖는지 비교한다.
pub fn same_elements(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut sa = a.to_vec();
    let mut sb = b.to_vec();
    sa.sort();
    sb.sort();
    sa == sb
}

/// 문자열을 지정한 너비로 오른쪽 정렬(왼쪽 패딩)한다(보고서 표 출력용).
pub fn pad_left(s: &str, width: usize, pad_char: char) -> String {
    if s.chars().count() >= width {
        s.to_string()
    } else {
        let padding: String = std::iter::repeat(pad_char).take(width - s.chars().count()).collect();
        format!("{padding}{s}")
    }
}

/// 문자열을 지정한 너비로 왼쪽 정렬(오른쪽 패딩)한다.
pub fn pad_right(s: &str, width: usize, pad_char: char) -> String {
    if s.chars().count() >= width {
        s.to_string()
    } else {
        let padding: String = std::iter::repeat(pad_char).take(width - s.chars().count()).collect();
        format!("{s}{padding}")
    }
}

/// 정수를 부호 포함 문자열로 포맷한다(양수에도 "+" 표시, 보고서 증감 표시용).
pub fn format_signed(value: i64) -> String {
    if value > 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

/// 문자열 목록에서 최대 길이를 구한다(표 컬럼 너비 계산에 사용).
pub fn max_len(items: &[String]) -> usize {
    items.iter().map(|s| s.chars().count()).max().unwrap_or(0)
}
