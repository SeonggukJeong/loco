#[test]
fn apply_tax_uses_12_percent() {
    assert_eq!(inv_core::rules::pricing::apply_tax(10_000), 11_200);
}
