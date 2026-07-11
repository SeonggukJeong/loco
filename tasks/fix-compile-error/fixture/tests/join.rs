use fix_compile_error::join_upper;

#[test]
fn joins_and_uppercases() {
    assert_eq!(join_upper(&["ab", "cd"]), "AB CD");
}

#[test]
fn empty() {
    assert_eq!(join_upper(&[]), "");
}
