#[test]
fn invoice_total_uses_12_percent() {
    assert_eq!(inv_report::invoice::invoice_total(100_000), 112_000);
}
#[test]
fn forecast_projection_uses_12_percent() {
    assert_eq!(inv_report::forecast::forecast_projection(200_000), 224_000);
}
