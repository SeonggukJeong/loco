//! CSV 필드/행 단위 유효성 검사.
//!
//! SKU/창고 코드 포맷 검증은 inv-core가 이미 갖고 있으므로(재고 도메인
//! 전체가 공유하는 규칙이라 두 번 만들지 않는다) 여기서는 그 함수를 그대로
//! 재사용하고, CSV 파싱 문맥에서만 의미 있는 검사(빈 필드, 숫자 형식 등)를
//! 추가로 둔다.

use crate::csv::{ParseError, ParsedRow};
use inv_core::sku::is_valid_sku;
use inv_core::warehouse::is_valid_warehouse_code;

/// SKU 필드가 유효한지 검사한다(inv-core의 SKU 포맷 규칙을 그대로 쓴다).
pub fn is_valid_sku_field(s: &str) -> bool {
    !s.is_empty() && is_valid_sku(s)
}

/// 창고 코드 필드가 유효한지 검사한다. CSV 배치 초기 도입 단계에서는
/// 창고 코드가 비어 있는 행도 흔해, 빈 문자열은 "미지정"으로 보고
/// 유효하다고 판정한다(엄격 검증은 상위 계층의 몫).
pub fn is_valid_warehouse_field(s: &str) -> bool {
    s.is_empty() || is_valid_warehouse_code(s)
}

/// 수량 필드 문자열이 파싱 가능한 형태인지 검사한다(부호 포함 숫자만 허용).
pub fn is_valid_qty_field(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    let start = if bytes[0] == b'-' { 1 } else { 0 };
    start < bytes.len() && bytes[start..].iter().all(|b| b.is_ascii_digit())
}

/// 단가 필드 문자열이 파싱 가능한 형태인지 검사한다(음수 불가).
pub fn is_valid_price_field(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// 카테고리 필드가 알려진(사내에서 실제 쓰는) 코드인지 느슨하게 검사한다.
///
/// 완전한 화이트리스트는 아니다 — 대문자 영숫자 2~8자만 형태를 검사한다.
pub fn looks_like_category_code(s: &str) -> bool {
    (2..=8).contains(&s.len()) && s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// 파싱된 행이 창고 코드까지 포함해 완전히 유효한지 검사한다(구조 파싱
/// 통과 후의 2차 검증 — 세율/합계와는 무관하다).
pub fn is_fully_valid_row(row: &ParsedRow) -> bool {
    is_valid_sku_field(&row.sku) && is_valid_warehouse_field(&row.warehouse_code) && row.qty >= 0
}

/// 행 목록 중 완전히 유효한 것만 걸러낸다.
pub fn filter_fully_valid(rows: &[ParsedRow]) -> Vec<ParsedRow> {
    rows.iter().filter(|r| is_fully_valid_row(r)).cloned().collect()
}

/// 행 목록 중 창고 코드가 비어 있는(미지정) 것만 걸러낸다.
pub fn rows_missing_warehouse(rows: &[ParsedRow]) -> Vec<ParsedRow> {
    rows.iter().filter(|r| r.warehouse_code.is_empty()).cloned().collect()
}

/// 파싱 오류 목록을 종류별로 센다: (컬럼수 불일치, SKU 오류, 수량 오류, 단가 오류).
pub fn count_error_kinds(errors: &[(usize, ParseError)]) -> (usize, usize, usize, usize) {
    let mut wrong_cols = 0;
    let mut sku = 0;
    let mut qty = 0;
    let mut price = 0;
    for (_, err) in errors {
        match err {
            ParseError::WrongColumnCount { .. } => wrong_cols += 1,
            ParseError::EmptySku => sku += 1,
            ParseError::InvalidQty(_) => qty += 1,
            ParseError::InvalidPrice(_) => price += 1,
        }
    }
    (wrong_cols, sku, qty, price)
}

/// 오류 발생 행 번호 목록만 뽑아낸다(로그 출력용).
pub fn error_line_numbers(errors: &[(usize, ParseError)]) -> Vec<usize> {
    errors.iter().map(|(n, _)| *n).collect()
}

/// 배치 안에서 같은 SKU가 같은 창고 코드로 몇 번이나 중복 등장했는지 센다.
///
/// 중복 자체가 오류는 아니지만(하루 여러 번 입고될 수 있다), 지나치게
/// 많으면 배치 파일이 잘못 이어붙여졌을 가능성을 의심할 신호가 된다.
pub fn duplicate_sku_warehouse_count(rows: &[ParsedRow], sku: &str, warehouse_code: &str) -> usize {
    rows.iter().filter(|r| r.sku == sku && r.warehouse_code == warehouse_code).count()
}

/// 행 목록에서 수량이 비정상적으로 큰(입력 오류 의심) 행만 걸러낸다.
pub fn suspiciously_large_qty_rows(rows: &[ParsedRow], threshold: i64) -> Vec<ParsedRow> {
    rows.iter().filter(|r| r.qty.abs() > threshold).cloned().collect()
}

/// 배치 전체의 오류율(%)을 계산한다(총 행 수가 0이면 0).
pub fn error_rate_percent(valid_count: usize, error_count: usize) -> u32 {
    let total = valid_count + error_count;
    if total == 0 {
        0
    } else {
        (error_count * 100 / total) as u32
    }
}

/// 오류율이 허용 상한을 넘어 배치 자체를 반려해야 하는지 판정한다.
pub fn should_reject_batch(valid_count: usize, error_count: usize, max_error_rate_percent: u32) -> bool {
    error_rate_percent(valid_count, error_count) > max_error_rate_percent
}

/// 카테고리 필드가 아예 비어 있는 행만 걸러낸다(필수는 아니지만 리포트
/// 품질 관리 차원에서 비율을 추적한다).
pub fn rows_missing_category(rows: &[ParsedRow]) -> Vec<ParsedRow> {
    rows.iter().filter(|r| r.category.trim().is_empty()).cloned().collect()
}

/// SKU 필드 값 중 앞뒤 공백이 있었던(트림 전 원본과 트림 후가 다른) 행이
/// 있는지 검사한다 — 입력 데이터 위생 상태를 가늠하는 지표다.
pub fn had_whitespace_padding(raw_field: &str) -> bool {
    raw_field != raw_field.trim()
}

/// 행 목록에서 SKU 형식은 유효하지만 창고 코드가 무효한 행만 걸러낸다
/// (SKU는 정상인데 창고만 잘못 찍힌 흔한 오타 패턴).
pub fn rows_with_bad_warehouse_only(rows: &[ParsedRow]) -> Vec<ParsedRow> {
    rows.iter()
        .filter(|r| is_valid_sku_field(&r.sku) && !is_valid_warehouse_field(&r.warehouse_code))
        .cloned()
        .collect()
}

/// 두 행 목록(예: 오늘 배치와 어제 배치)에서 공통으로 등장하는 SKU 집합을 구한다.
pub fn common_skus(a: &[ParsedRow], b: &[ParsedRow]) -> Vec<String> {
    let a_skus: Vec<&str> = a.iter().map(|r| r.sku.as_str()).collect();
    let mut common: Vec<String> = b
        .iter()
        .map(|r| r.sku.as_str())
        .filter(|s| a_skus.contains(s))
        .map(|s| s.to_string())
        .collect();
    common.sort();
    common.dedup();
    common
}

/// 오류 종류별 개수를 사람이 읽을 수 있는 요약 문자열로 만든다.
pub fn summarize_error_kinds(errors: &[(usize, ParseError)]) -> String {
    let (wrong_cols, sku, qty, price) = count_error_kinds(errors);
    format!("컬럼수 {wrong_cols}건, SKU {sku}건, 수량 {qty}건, 단가 {price}건")
}

/// 행 목록이 하나의 카테고리로만 이루어져 있는지 검사한다(단일 카테고리
/// 배치인지 여부 — 혼합 배치와 다른 처리 경로를 타는 상위 로직이 참조).
pub fn is_single_category_batch(rows: &[ParsedRow]) -> bool {
    if rows.is_empty() {
        return true;
    }
    let first = &rows[0].category;
    rows.iter().all(|r| &r.category == first)
}

/// 행 목록에서 SKU 포맷은 유효하지만 카테고리 코드 형식이 이상한 행만 걸러낸다.
pub fn rows_with_bad_category_only(rows: &[ParsedRow]) -> Vec<ParsedRow> {
    rows.iter()
        .filter(|r| is_valid_sku_field(&r.sku) && !looks_like_category_code(&r.category))
        .cloned()
        .collect()
}

/// 오류 목록 중 특정 종류(컬럼 수 불일치)만 골라낸다.
pub fn wrong_column_count_errors(errors: &[(usize, ParseError)]) -> Vec<(usize, usize, usize)> {
    errors
        .iter()
        .filter_map(|(n, e)| match e {
            ParseError::WrongColumnCount { expected, actual } => Some((*n, *expected, *actual)),
            _ => None,
        })
        .collect()
}

/// 유효/무효 행 카운트를 받아 배치를 3단계(양호/주의/반려)로 등급화한다.
pub fn batch_grade(valid_count: usize, error_count: usize) -> &'static str {
    let rate = error_rate_percent(valid_count, error_count);
    if rate == 0 {
        "양호"
    } else if rate <= 5 {
        "주의"
    } else {
        "반려"
    }
}

/// 행 목록에서 SKU가 정확히 일치하는 두 행이 있는지(완전 중복 여부) 검사한다.
pub fn has_exact_duplicate(rows: &[ParsedRow]) -> bool {
    for (i, a) in rows.iter().enumerate() {
        for b in rows.iter().skip(i + 1) {
            if a == b {
                return true;
            }
        }
    }
    false
}
