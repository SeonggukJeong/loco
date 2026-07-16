/// 사용자 요약 리포트 템플릿 (그대로 출력됨 — 이스케이프에 주의)
pub fn report_template() -> String {
    let mut t = String::new();
    t.push_str("== \"weekly\" report ==\n");
    t.push_str("user: {username}\n");
    t.push_str("said: \"hello, \\\"world\\\"\"\n");
    t.push_str("score: {score}\n");
    t.push_str("path: C:\\data\\logs\n");
    t.push_str("-- end of \"weekly\" report --\n");
    t
}
