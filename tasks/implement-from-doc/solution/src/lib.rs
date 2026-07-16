/// 런랭스 인코딩(RLE).
/// 연속으로 반복되는 문자를 `문자 + 반복횟수`로 축약한다.
/// 반복이 1회인 문자에도 횟수 1을 붙인다.
/// 예: "aaabbc" -> "a3b2c1", "" -> "".
/// 유니코드 문자 단위(char)로 처리한다.
pub fn rle(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    let Some(mut current) = chars.next() else {
        return out;
    };
    let mut count: usize = 1;
    for c in chars {
        if c == current {
            count += 1;
        } else {
            out.push(current);
            out.push_str(&count.to_string());
            current = c;
            count = 1;
        }
    }
    out.push(current);
    out.push_str(&count.to_string());
    out
}
