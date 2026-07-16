//! `ingest` 서브커맨드: CSV 텍스트를 파싱해 결과를 요약한다.
//!
//! 실제 배치는 파일 경로를 받아 디스크에서 텍스트를 읽어오지만, 이
//! 서브커맨드는 텍스트를 인자로 직접 받는다 — 파일 I/O는 이 크레이트가
//! 담당하는 유일한 경계지만, 텍스트 자체를 다루는 파싱/집계 로직은
//! inv-parse에 위임한다.

use inv_parse::csv::{parse_all_rows, row_subtotal_krw, ParsedRow};

/// 텍스트를 파싱해 결과 요약 문자열을 만든다.
pub fn execute(text: &str) -> String {
    let (rows, errors) = parse_all_rows(text);
    let subtotal = total_subtotal(&rows);
    format!(
        "파싱 완료: 유효 {}행, 오류 {}행, 소계 합계 {}원",
        rows.len(),
        errors.len(),
        subtotal
    )
}

/// 파싱된 행 목록의 소계 합계를 구한다.
fn total_subtotal(rows: &[ParsedRow]) -> i64 {
    rows.iter().map(row_subtotal_krw).sum()
}

/// 텍스트를 파싱해 오류 행 번호 목록만 뽑아낸다(문제 있는 배치를 빠르게
/// 짚어낼 때 쓴다).
pub fn error_line_numbers(text: &str) -> Vec<usize> {
    let (_, errors) = parse_all_rows(text);
    errors.iter().map(|(line_no, _)| *line_no).collect()
}

/// 텍스트가 파싱 가능한 데이터 행을 하나도 포함하지 않는지(완전히 빈
/// 배치인지) 검사한다.
pub fn is_empty_batch(text: &str) -> bool {
    let (rows, errors) = parse_all_rows(text);
    rows.is_empty() && errors.is_empty()
}

/// 텍스트를 파싱해 오류율(%)을 계산한다(전체 시도한 행 대비 오류 비율).
pub fn error_rate_percent(text: &str) -> u32 {
    let (rows, errors) = parse_all_rows(text);
    let total = rows.len() + errors.len();
    if total == 0 {
        0
    } else {
        ((errors.len() * 100) / total) as u32
    }
}

/// 파싱 결과 요약을 실행 결과 문자열에서 다시 파싱해 유효 행 수만
/// 뽑아낸다(라우팅 테스트에서 출력 형식을 검증할 때 쓰는 보조 함수).
pub fn extract_valid_count(output: &str) -> Option<usize> {
    let marker = "유효 ";
    let start = output.find(marker)? + marker.len();
    let rest = &output[start..];
    let end = rest.find('행')?;
    rest[..end].parse::<usize>().ok()
}

/// 파싱 결과 요약 문자열에서 오류 행 수만 뽑아낸다.
pub fn extract_error_count(output: &str) -> Option<usize> {
    let marker = "오류 ";
    let start = output.find(marker)? + marker.len();
    let rest = &output[start..];
    let end = rest.find('행')?;
    rest[..end].parse::<usize>().ok()
}

/// 텍스트를 파싱해 창고 코드별 유효 행 수를 (창고코드, 개수) 목록으로
/// 계산한다.
pub fn count_by_warehouse(text: &str) -> Vec<(String, usize)> {
    let (rows, _) = parse_all_rows(text);
    inv_parse::csv::count_by_category(&rows.iter().map(|r| ParsedRow { category: r.warehouse_code.clone(), ..r.clone() }).collect::<Vec<_>>())
}

/// 텍스트를 파싱해 카테고리별 유효 행 수를 (카테고리, 개수) 목록으로 낸다.
pub fn count_by_category(text: &str) -> Vec<(String, usize)> {
    let (rows, _) = parse_all_rows(text);
    inv_parse::csv::count_by_category(&rows)
}

/// 텍스트를 파싱해 평균 단가를 계산한다(파싱된 유효 행만 대상).
pub fn average_unit_price(text: &str) -> i64 {
    let (rows, _) = parse_all_rows(text);
    inv_parse::csv::average_unit_price(&rows)
}

/// 텍스트를 파싱해 SKU가 비어 있는 등, 구조적으로 온전하지 않은 유효
/// 행이 있는지 검사한다(파싱은 성공했지만 값이 의심스러운 행 탐지).
pub fn has_structurally_unsound_rows(text: &str) -> bool {
    let (rows, _) = parse_all_rows(text);
    rows.iter().any(|r| !inv_parse::csv::is_structurally_sound(r))
}

/// 텍스트를 파싱해 결과를 두 줄 요약(유효/오류를 각각 별도 줄)으로 만든다
/// (한 줄 요약 `execute`와 달리 표 형태 출력에 쓰인다).
pub fn execute_multiline(text: &str) -> String {
    let (rows, errors) = parse_all_rows(text);
    format!("유효 행: {}\n오류 행: {}", rows.len(), errors.len())
}

/// 텍스트를 파싱해 오류 메시지를 사람이 읽는 여러 줄 텍스트로 나열한다.
pub fn describe_errors(text: &str) -> String {
    let (_, errors) = parse_all_rows(text);
    if errors.is_empty() {
        return "오류 없음".to_string();
    }
    errors
        .iter()
        .map(|(line_no, e)| format!("{line_no}번째 행: {}", inv_parse::csv::describe_error(e)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 텍스트를 파싱해 SKU가 중복 등장하는 행이 있는지(같은 SKU+창고 조합이
/// 여러 번 나오는지) 검사한다.
pub fn has_duplicate_rows(text: &str) -> bool {
    let (rows, _) = parse_all_rows(text);
    let deduped = inv_parse::csv::dedup_by_sku_warehouse(&rows);
    deduped.len() != rows.len()
}

/// 텍스트를 파싱해 소계가 지정 임계값 이상인 고액 행만 걸러 요약한다.
pub fn high_value_row_report(text: &str, threshold_krw: i64) -> String {
    let (rows, _) = parse_all_rows(text);
    let high_value = inv_parse::csv::rows_above_subtotal(&rows, threshold_krw);
    format!("고액 행({}원 이상): {}건", threshold_krw, high_value.len())
}

/// 여러 텍스트 배치를 순서대로 파싱해 각각의 유효/오류 건수를 모은다.
pub fn execute_batch(texts: &[String]) -> Vec<String> {
    texts.iter().map(|t| execute(t)).collect()
}

/// 텍스트가 단일 창고 배치인지(모든 행이 같은 창고 코드인지) 판정한다.
pub fn is_single_warehouse(text: &str) -> bool {
    let (rows, _) = parse_all_rows(text);
    inv_parse::csv::is_single_warehouse_batch(&rows)
}
