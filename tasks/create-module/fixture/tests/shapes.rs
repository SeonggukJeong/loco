use create_module::shapes::perimeter;

#[test]
fn rectangle_perimeter() {
    assert_eq!(perimeter(3, 4), 14);
}

#[test]
fn square() {
    assert_eq!(perimeter(5, 5), 20);
}
