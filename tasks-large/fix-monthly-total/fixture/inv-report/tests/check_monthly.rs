//! 과제 판정: 월간 정산 합계의 부호 정상성.
use inv_core::ledger::{LedgerLine, LineKind};
use inv_report::monthly::monthly_total;

fn line(kind: LineKind, amount: i64) -> LedgerLine {
    LedgerLine { sku: "SKU-1".into(), kind, amount_krw: amount, adj_krw: 0 }
}

#[test]
fn monthly_total_with_refund_is_positive_net() {
    let lines = vec![
        line(LineKind::Sale, 120_000),
        line(LineKind::Sale, 80_000),
        line(LineKind::Refund, 30_000),
    ];
    assert_eq!(monthly_total(&lines), 170_000);
}

#[test]
fn monthly_total_sales_only_equals_sum() {
    let lines = vec![line(LineKind::Sale, 50_000), line(LineKind::Sale, 70_000)];
    assert_eq!(monthly_total(&lines), 120_000);
}
