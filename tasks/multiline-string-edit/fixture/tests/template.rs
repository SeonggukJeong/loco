use multiline_string_edit::report_template;

#[test]
fn template_matches_spec_exactly() {
    let expected = "== \"weekly\" report ==\nuser: {username}\nsaid: \"hello, \\\"world\\\"\"\nscore: {score}\npath: C:\\data\\logs\n-- end of \"weekly\" report --\n";
    assert_eq!(report_template(), expected);
}
