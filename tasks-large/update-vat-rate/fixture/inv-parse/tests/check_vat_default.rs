#[test]
fn default_config_vat_is_12() {
    assert_eq!(inv_parse::config::parse_config("").vat_percent, 12);
}
