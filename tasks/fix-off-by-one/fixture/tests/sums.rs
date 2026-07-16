use fix_off_by_one::sum_upto;

#[test]
fn sums_inclusive() {
    assert_eq!(sum_upto(5), 15);
}

#[test]
fn one() {
    assert_eq!(sum_upto(1), 1);
}

// M6 §3: 기존 zero(0→0)는 배타 범위 버그((1..n))에서도 통과하는 비변별 케이스라
// 교체 — two는 버그 상태에서 1을 반환해 실패한다 (변별)
#[test]
fn two() {
    assert_eq!(sum_upto(2), 3);
}
