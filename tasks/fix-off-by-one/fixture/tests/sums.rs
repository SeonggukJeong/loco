use fix_off_by_one::sum_upto;

#[test]
fn sums_inclusive() {
    assert_eq!(sum_upto(5), 15);
}

#[test]
fn one() {
    assert_eq!(sum_upto(1), 1);
}

#[test]
fn zero() {
    assert_eq!(sum_upto(0), 0);
}
