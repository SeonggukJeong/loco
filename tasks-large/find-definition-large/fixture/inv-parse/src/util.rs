//! 파싱 문맥에서만 쓰는 소소한 문자열 헬퍼.
//!
//! 크레이트 전역 문자열 유틸은 inv-core에도 있지만, 여기 있는 것들은
//! CSV/설정 텍스트를 다룰 때만 의미가 있는 것들이라(BOM 처리, 줄 개수
//! 세기 등) 이 크레이트에 별도로 둔다.

/// UTF-8 BOM(`EF BB BF`)이 붙어 있으면 제거한다.
pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// 줄바꿈을 전부 `\n`으로 통일한다(CRLF/CR 혼합 입력 대응).
pub fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// 텍스트가 `#`으로 시작하는 주석 줄인지 검사한다(앞 공백은 무시).
pub fn is_comment_line(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// 텍스트의 공백이 아닌(=의미 있는) 줄 수를 센다.
pub fn count_meaningful_lines(text: &str) -> usize {
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

/// 필드 양끝의 따옴표 한 겹만 제거한다(이미 없으면 그대로).
pub fn trim_one_quote_layer(field: &str) -> &str {
    field.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(field)
}

/// 문자열이 헤더 행처럼 보이는지 대략적으로 추정한다: 숫자로만 이루어진
/// 필드가 하나도 없으면 헤더일 가능성이 높다고 본다.
pub fn looks_like_header_line(line: &str) -> bool {
    line.split(',').all(|f| {
        let t = f.trim();
        !t.chars().all(|c| c.is_ascii_digit()) || t.is_empty()
    })
}

/// 안전한 부분 문자열을 반환한다(문자 경계를 벗어나지 않도록 char 단위로 자름).
pub fn safe_prefix(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// 텍스트를 빈 줄 기준으로 블록(레코드 묶음)으로 나눈다.
pub fn split_into_blocks(text: &str) -> Vec<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

/// 줄 목록에서 빈 줄과 주석 줄을 모두 제거한다.
pub fn strip_noise_lines(text: &str) -> Vec<&str> {
    text.lines().filter(|l| !l.trim().is_empty() && !is_comment_line(l)).collect()
}

/// 문자열 목록을 지정한 구분자로 합치되, 빈 문자열은 제외한다.
pub fn join_non_empty(items: &[String], sep: &str) -> String {
    items.iter().filter(|s| !s.trim().is_empty()).cloned().collect::<Vec<_>>().join(sep)
}

/// 문자열이 지정된 접두사 중 하나로 시작하는지 검사한다(로그 태그 판별 등).
pub fn starts_with_any(s: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|p| s.starts_with(p))
}

/// 텍스트의 첫 번째 비어있지 않은 줄을 반환한다(샘플링/미리보기용).
pub fn first_meaningful_line(text: &str) -> Option<&str> {
    text.lines().find(|l| !l.trim().is_empty())
}

/// 문자열 내 탭 문자를 지정한 개수의 공백으로 확장한다(고정폭 로그 출력용).
pub fn expand_tabs(s: &str, width: usize) -> String {
    s.replace('\t', &" ".repeat(width))
}
