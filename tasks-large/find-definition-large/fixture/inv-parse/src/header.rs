//! CSV 헤더 행 파싱과 컬럼 이름 매핑.
//!
//! v1 포맷은 컬럼 순서가 고정(`csv::EXPECTED_COLUMNS`)이라 헤더 없이도
//! 파싱할 수 있지만, 배치 파일에 헤더가 실제로 붙어 오는 경우가 대부분
//! 이라 이름 기준으로 컬럼 인덱스를 찾는 유틸을 따로 둔다 — 향후 컬럼
//! 순서가 벤더마다 달라지는 상황에 대비한다.

use crate::csv::EXPECTED_COLUMNS;

/// v1 헤더의 정식 컬럼 이름(순서대로).
pub const CANONICAL_HEADER: [&str; 5] = ["sku", "warehouse_code", "qty", "unit_price_krw", "category"];

/// 헤더 한 줄을 컬럼 이름 목록으로 파싱한다(공백 제거, 소문자화).
pub fn parse_header_line(line: &str) -> Vec<String> {
    line.split(',').map(|f| f.trim().to_ascii_lowercase()).collect()
}

/// 파싱한 헤더에서 특정 컬럼 이름의 인덱스를 찾는다.
pub fn column_index(header: &[String], name: &str) -> Option<usize> {
    header.iter().position(|h| h == name)
}

/// 헤더가 정식 컬럼 이름을 모두 포함하는지(순서는 무관) 검사한다.
pub fn has_all_canonical_columns(header: &[String]) -> bool {
    CANONICAL_HEADER.iter().all(|name| header.iter().any(|h| h == name))
}

/// 헤더가 정식 순서와 정확히 일치하는지 검사한다.
pub fn is_canonical_order(header: &[String]) -> bool {
    header.len() == EXPECTED_COLUMNS && header.iter().zip(CANONICAL_HEADER.iter()).all(|(a, b)| a == b)
}

/// 헤더에서 누락된 정식 컬럼 이름 목록을 반환한다.
pub fn missing_columns(header: &[String]) -> Vec<&'static str> {
    CANONICAL_HEADER.iter().filter(|name| !header.iter().any(|h| h == *name)).copied().collect()
}

/// 헤더에 정식 컬럼이 아닌 이름(오타/추가 컬럼)이 있는지 검사해 목록으로 낸다.
pub fn unrecognized_columns(header: &[String]) -> Vec<String> {
    header.iter().filter(|h| !CANONICAL_HEADER.contains(&h.as_str())).cloned().collect()
}

/// 헤더를 기준으로 임의 순서 행을 정식 순서로 재배열한다. 컬럼이 하나라도
/// 빠져 있으면 `None`.
pub fn reorder_to_canonical(header: &[String], fields: &[String]) -> Option<Vec<String>> {
    if header.len() != fields.len() {
        return None;
    }
    let mut reordered = Vec::with_capacity(CANONICAL_HEADER.len());
    for name in CANONICAL_HEADER.iter() {
        let idx = column_index(header, name)?;
        reordered.push(fields.get(idx)?.clone());
    }
    Some(reordered)
}

/// 정식 헤더 행 문자열을 만든다(재출력용).
pub fn canonical_header_line() -> String {
    CANONICAL_HEADER.join(",")
}

/// 헤더 두 개(예: 이전 배치/이번 배치)를 비교해 순서가 같은지 검사한다.
pub fn same_order(a: &[String], b: &[String]) -> bool {
    a == b
}

/// 헤더 컬럼 수가 예상과 다른지 검사한다(형식 사전 점검용).
pub fn has_unexpected_column_count(header: &[String]) -> bool {
    header.len() != EXPECTED_COLUMNS
}

/// 헤더 컬럼 이름에 흔한 오타/변형(예: "sku_code" 대신 "skucode")이 있는지
/// 확인해, 정식 이름으로 교정할 수 있는 후보를 제안한다. 교정 불가능하면
/// 원래 이름을 그대로 둔다.
pub fn suggest_correction(name: &str) -> String {
    let compact = name.replace(['_', '-', ' '], "").to_ascii_lowercase();
    for canonical in CANONICAL_HEADER.iter() {
        if compact == canonical.replace('_', "") {
            return canonical.to_string();
        }
    }
    name.to_string()
}

/// 헤더 전체에 교정을 시도해 새 헤더 목록을 만든다.
pub fn auto_correct_header(header: &[String]) -> Vec<String> {
    header.iter().map(|h| suggest_correction(h)).collect()
}

/// 교정 후 헤더가 정식 컬럼을 모두 포함하게 되는지(즉, 교정이 헤더 문제를
/// 실제로 해결했는지) 검사한다.
pub fn correction_resolves_header(header: &[String]) -> bool {
    has_all_canonical_columns(&auto_correct_header(header))
}

/// 헤더에서 중복된 컬럼 이름이 있는지 검사한다.
pub fn has_duplicate_columns(header: &[String]) -> bool {
    let mut seen: Vec<&String> = Vec::new();
    for h in header {
        if seen.contains(&h) {
            return true;
        }
        seen.push(h);
    }
    false
}

/// 헤더에서 중복된 컬럼 이름 목록만 뽑아낸다.
pub fn duplicate_columns(header: &[String]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut dups: Vec<String> = Vec::new();
    for h in header {
        if seen.contains(h) {
            if !dups.contains(h) {
                dups.push(h.clone());
            }
        } else {
            seen.push(h.clone());
        }
    }
    dups
}

/// 헤더가 완전히 비어 있는지(컬럼이 하나도 없는지) 검사한다.
pub fn is_empty_header(header: &[String]) -> bool {
    header.is_empty()
}

/// 헤더 목록을 사람이 읽는 콤마 나열 문자열로 만든다(로그/에러 메시지용).
pub fn describe_header(header: &[String]) -> String {
    if header.is_empty() {
        "(빈 헤더)".to_string()
    } else {
        header.join(", ")
    }
}

/// 텍스트의 첫 줄이 실제로 헤더인지 아니면 이미 데이터 행인지 추정한다
/// (정식 컬럼 이름과 일치하는 필드가 하나라도 있으면 헤더로 본다).
pub fn first_line_is_probably_header(text: &str) -> bool {
    let Some(first) = text.lines().find(|l| !l.trim().is_empty()) else {
        return false;
    };
    let header = parse_header_line(first);
    header.iter().any(|h| CANONICAL_HEADER.contains(&h.as_str()))
}

/// 헤더 목록에서 정식 컬럼과 이름이 같지만 대소문자만 다른 항목을 찾는다.
pub fn case_mismatched_columns(raw_header: &[String]) -> Vec<String> {
    raw_header
        .iter()
        .filter(|h| {
            let lower = h.to_ascii_lowercase();
            CANONICAL_HEADER.contains(&lower.as_str()) && h.as_str() != lower
        })
        .cloned()
        .collect()
}

/// 두 헤더의 컬럼 순서가 다르더라도 같은 컬럼 집합을 갖는지 비교한다.
pub fn same_columns_different_order(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().all(|h| b.contains(h))
}

/// 헤더에서 공백만 있는(빈 컬럼명) 위치의 인덱스를 찾는다.
pub fn blank_column_indices(header: &[String]) -> Vec<usize> {
    header.iter().enumerate().filter(|(_, h)| h.trim().is_empty()).map(|(i, _)| i).collect()
}
