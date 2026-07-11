use rename_function::{price_total, receipt};

#[test]
fn renamed_function_is_exported() {
    assert_eq!(price_total(&[(2, 100), (1, 50)]), 250);
}

#[test]
fn callers_still_work() {
    assert_eq!(receipt::summary(&[(1, 100)]), "total: 100");
    assert_eq!(receipt::with_shipping(&[(1, 100)]), 600);
}
