use fix_failing_test::{max_csv, sum_csv};

#[test]
fn sums() {
    assert_eq!(sum_csv("1, 2,3"), 6);
}

#[test]
fn empty_sum() {
    assert_eq!(sum_csv(""), 0);
}

#[test]
fn max_of_list() {
    assert_eq!(max_csv("3, 9, 2"), Some(9));
}

#[test]
fn max_single() {
    assert_eq!(max_csv("7"), Some(7));
}

#[test]
fn max_empty() {
    assert_eq!(max_csv(""), None);
}
