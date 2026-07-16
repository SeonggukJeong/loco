//! inv-parse 베이스 테스트.
//!
//! 금지 구역 준수: `DEFAULT_VAT_PERCENT`의 값이나 `parse_config`가 반환한
//! `vat_percent` 값은 이 파일에서 절대 단정하지 않는다(부가세율 변경
//! 과제의 판정 신호이기 때문 — 세율이 바뀐 뒤에도 이 파일의 테스트는
//! 그대로 통과해야 한다). 대신 구조 파싱(컬럼 분리, 필드 추출, 오류 종류
//! 판별)만 검증한다.

use inv_parse::config::parse_config;
use inv_parse::csv::{parse_row, split_csv_line, ParseError};
use inv_parse::date::parse_date;
use inv_parse::delimiter::detect_from_line;
use inv_parse::reader::{collect_valid_rows, count_data_rows};
use inv_parse::readers::read_batch;
use inv_parse::validate::is_fully_valid_row;

#[test]
fn parse_row_extracts_structural_fields() {
    let row = parse_row("EL-000123,SEL1,10,5000,ELEC").expect("should parse");
    assert_eq!(row.sku, "EL-000123");
    assert_eq!(row.warehouse_code, "SEL1");
    assert_eq!(row.qty, 10);
    assert_eq!(row.unit_price_krw, 5000);
    assert_eq!(row.category, "ELEC");
}

#[test]
fn parse_row_rejects_wrong_column_count() {
    let result = parse_row("EL-000123,SEL1,10");
    assert!(matches!(result, Err(ParseError::WrongColumnCount { expected: 5, actual: 3 })));
}

#[test]
fn parse_row_rejects_bad_sku() {
    let result = parse_row("bad-sku,SEL1,10,5000,ELEC");
    assert!(matches!(result, Err(ParseError::EmptySku)));
}

#[test]
fn split_csv_line_handles_quoted_comma() {
    let fields = split_csv_line("EL-000123,SEL1,10,5000,\"Home, Kitchen\"");
    assert_eq!(fields.len(), 5);
    assert_eq!(fields[4], "Home, Kitchen");
}

#[test]
fn split_csv_line_handles_escaped_quote() {
    let fields = split_csv_line("A,\"say \"\"hi\"\"\",B,C,D");
    assert_eq!(fields[1], "say \"hi\"");
}

#[test]
fn config_parses_known_keys() {
    let cfg = parse_config("warehouse_count=5\ncurrency=USD\n");
    assert_eq!(cfg.warehouse_count, 5);
    assert_eq!(cfg.currency, "USD");
}

#[test]
fn config_ignores_comments_and_unknown_keys() {
    let cfg = parse_config("# 이 줄은 주석\nwarehouse_count=3\nunknown_key=zzz\n");
    assert_eq!(cfg.warehouse_count, 3);
}

#[test]
fn reader_collects_valid_rows_and_skips_header() {
    let text = "sku,warehouse_code,qty,unit_price_krw,category\nEL-000123,SEL1,10,5000,ELEC\nEL-000456,BSN1,3,1000,FOOD\n";
    let rows = collect_valid_rows(text, true);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].sku, "EL-000123");
}

#[test]
fn reader_counts_data_rows_excluding_header() {
    let text = "header_line\nEL-000123,SEL1,10,5000,ELEC\n";
    assert_eq!(count_data_rows(text, true), 1);
}

#[test]
fn batch_reader_aggregates_multiple_files() {
    let a = "EL-000123,SEL1,10,5000,ELEC\n";
    let b = "EL-000456,BSN1,3,1000,FOOD\n";
    let result = read_batch(&[a, b], false);
    assert_eq!(result.file_count(), 2);
    assert_eq!(result.total_valid(), 2);
}

#[test]
fn validate_rejects_row_with_bad_warehouse_code() {
    let row = parse_row("EL-000123,sel1,10,5000,ELEC").expect("should parse structurally");
    assert!(!is_fully_valid_row(&row));
}

#[test]
fn date_parses_all_three_formats_to_same_value() {
    let a = parse_date("2024-11-03").unwrap();
    let b = parse_date("2024/11/03").unwrap();
    let c = parse_date("20241103").unwrap();
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn delimiter_detects_semicolon() {
    assert_eq!(detect_from_line("a;b;c;d"), ';');
}
