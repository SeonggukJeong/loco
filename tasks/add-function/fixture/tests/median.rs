use add_function::median;

#[test]
fn odd_length() {
    assert_eq!(median(&[3, 1, 2]), 2.0);
}

#[test]
fn even_length() {
    assert_eq!(median(&[1, 2, 3, 4]), 2.5);
}

#[test]
fn single() {
    assert_eq!(median(&[5]), 5.0);
}

#[test]
fn unsorted_negative() {
    assert_eq!(median(&[-5, 10, 0]), 0.0);
}
