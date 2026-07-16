//! inv-report 베이스 테스트.
//!
//! 정책에 따라 바뀌는 파생값(세율 등)은 여기서 구체값을 단정하지 않는다 —
//! 그 계산 로직 자신의 테스트가 값 검증을 맡고, 여기서는 리포트 조립·요약·
//! 포맷팅 같은 구조적 동작만 확인한다. v1 `calc_total`은 안정된 경로라
//! 합계값을 그대로 단정한다.

use inv_core::ledger::{LedgerLine, LineKind};
use inv_report::comparison::Comparison;
use inv_report::export::csv_row;
use inv_report::forecast::growth_rate_percent;
use inv_report::invoice::{invoice_number, is_valid_invoice_number};
use inv_report::period::PeriodRange;
use inv_report::report::build_report;
use inv_report::reporting::render_line;
use inv_report::summary::summarize;
use inv_report::totals::{calc_total, total_sales_only};
use inv_report::trend::is_monotonic_increasing;
use inv_report::util::format_krw_with_commas;

fn sample_lines() -> Vec<LedgerLine> {
    vec![
        LedgerLine::new("EL-000123", LineKind::Sale, 50_000, 0),
        LedgerLine::new("EL-000123", LineKind::Refund, 10_000, 0),
        LedgerLine::new("FD-000456", LineKind::Adjustment, 0, 2_000),
    ]
}

#[test]
fn calc_total_v1_sums_sale_minus_refund_plus_adjustment() {
    // v1 calc_total의 합계값 단정은 명시적으로 허용된다.
    let lines = sample_lines();
    assert_eq!(calc_total(&lines), 50_000 - 10_000 + 2_000);
}

#[test]
fn calc_total_v1_empty_ledger_is_zero() {
    assert_eq!(calc_total(&[]), 0);
}

#[test]
fn total_sales_only_ignores_refund_and_adjustment() {
    let lines = sample_lines();
    assert_eq!(total_sales_only(&lines), 50_000);
}

#[test]
fn build_report_net_matches_v1_total() {
    let lines = sample_lines();
    let report = build_report(&lines);
    assert_eq!(report.net_krw, calc_total(&lines));
    assert_eq!(report.line_count, 3);
}

#[test]
fn summarize_counts_lines_by_kind() {
    let lines = sample_lines();
    let summary = summarize(&lines);
    assert_eq!(summary.line_count, 3);
    assert_eq!(summary.sale_count, 1);
    assert_eq!(summary.refund_count, 1);
    assert_eq!(summary.adjustment_count, 1);
    assert_eq!(summary.sku_count, 2);
}

#[test]
fn comparison_computes_delta_and_percent() {
    let cmp = Comparison::new(1000, 1200);
    assert_eq!(cmp.delta(), 200);
    assert_eq!(cmp.delta_percent(), 20);
    assert!(cmp.improved());
}

#[test]
fn period_range_contains_and_overlaps() {
    let a = PeriodRange::new(0, 100);
    let b = PeriodRange::new(50, 150);
    assert!(a.contains(50));
    assert!(!a.contains(100));
    assert!(a.overlaps(&b));
}

#[test]
fn trend_monotonic_increasing_detects_flat_and_rising() {
    assert!(is_monotonic_increasing(&[1, 1, 2, 3]));
    assert!(!is_monotonic_increasing(&[3, 2, 1]));
}

#[test]
fn forecast_growth_rate_percent_from_previous_and_current() {
    // growth_rate_percent는 세율과 무관한 일반 증감률 계산이다.
    assert_eq!(growth_rate_percent(1000, 1100), 10);
    assert_eq!(growth_rate_percent(0, 500), 0);
}

#[test]
fn invoice_number_format_round_trips() {
    let number = invoice_number(42);
    assert_eq!(number, "INV-000042");
    assert!(is_valid_invoice_number(&number));
    assert!(!is_valid_invoice_number("BAD-000042"));
}

#[test]
fn reporting_render_line_pads_with_dots() {
    let line = render_line("합계", 1000, 20);
    assert!(line.starts_with("합계"));
    assert!(line.ends_with("1000"));
}

#[test]
fn export_csv_row_escapes_comma_field() {
    let row = csv_row(&["a,b".to_string(), "c".to_string()]);
    assert_eq!(row, "\"a,b\",c");
}

#[test]
fn util_format_krw_with_commas_inserts_separators() {
    assert_eq!(format_krw_with_commas(1_234_000), "1,234,000");
    assert_eq!(format_krw_with_commas(-1_234), "-1,234");
}
