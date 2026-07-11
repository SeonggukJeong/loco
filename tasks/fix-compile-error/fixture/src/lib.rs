/// 단어들을 대문자로 바꿔 공백 하나로 잇는다
pub fn join_upper(words: &[&str]) -> String {
    let result = String::new();
    for w in words {
        result.push_str(&w.to_uppercase());
        result.push(' ');
    }
    result.trim_end().to_string()
}
