use implement_from_doc::rle;

#[test]
fn basic() {
    assert_eq!(rle("aaabbc"), "a3b2c1");
}

#[test]
fn empty() {
    assert_eq!(rle(""), "");
}

#[test]
fn no_repeats() {
    assert_eq!(rle("abc"), "a1b1c1");
}

#[test]
fn unicode() {
    assert_eq!(rle("가가가나"), "가3나1");
}

#[test]
fn long_run() {
    assert_eq!(rle(&"z".repeat(12)), "z12");
}
