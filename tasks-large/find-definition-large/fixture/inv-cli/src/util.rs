//! CLI 출력을 다룰 때 쓰는 소소한 텍스트 헬퍼 모음.
//!
//! 인자 목록 정리, 출력 줄 접두사 붙이기, 간단한 표 정렬 등 커맨드
//! 출력을 조립할 때마다 필요한 자잘한 함수들을 모아둔다.

/// 인자 목록에서 `--`로 시작하는 플래그만 걸러낸다.
pub fn extract_flags(args: &[String]) -> Vec<String> {
    args.iter().filter(|a| a.starts_with("--")).cloned().collect()
}

/// 인자 목록에서 플래그가 아닌(위치 인자) 항목만 걸러낸다.
pub fn extract_positionals(args: &[String]) -> Vec<String> {
    args.iter().filter(|a| !a.starts_with('-')).cloned().collect()
}

/// 출력 텍스트의 각 줄 앞에 접두사를 붙인다(하위 커맨드 출력을 상위
/// 컨텍스트에 표시할 때 들여쓰기 용도로 쓴다).
pub fn prefix_lines(text: &str, prefix: &str) -> String {
    text.lines().map(|l| format!("{prefix}{l}")).collect::<Vec<_>>().join("\n")
}

/// 여러 출력 블록을 빈 줄로 구분해 하나로 합친다.
pub fn join_blocks(blocks: &[String]) -> String {
    blocks.iter().filter(|b| !b.trim().is_empty()).cloned().collect::<Vec<_>>().join("\n\n")
}

/// 문자열이 특정 플래그(`--name` 또는 `--name=value`)를 포함하는지 검사한다.
pub fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name || a.starts_with(&format!("{name}=")))
}

/// 오류 메시지를 사용자에게 보여줄 최종 문자열로 포맷한다("오류: ..." 접두).
pub fn format_error_message(message: &str) -> String {
    format!("오류: {message}")
}

/// 문자열 목록을 지정한 너비로 표 형태처럼 오른쪽 정렬해 나열한다.
pub fn align_right(items: &[String], width: usize) -> Vec<String> {
    items
        .iter()
        .map(|s| {
            if s.chars().count() >= width {
                s.clone()
            } else {
                format!("{}{}", " ".repeat(width - s.chars().count()), s)
            }
        })
        .collect()
}

/// 빈 문자열이거나 공백만 있는 인자를 걸러낸 새 목록을 만든다(사용자가
/// 실수로 빈 인자를 넘긴 경우 방어).
pub fn drop_blank_args(args: &[String]) -> Vec<String> {
    args.iter().filter(|a| !a.trim().is_empty()).cloned().collect()
}

/// 인자 개수가 최소 요구치를 만족하는지 검사한다.
pub fn has_min_args(args: &[String], min: usize) -> bool {
    args.len() >= min
}

/// 문자열 목록을 지정한 너비로 왼쪽 정렬한다(라벨 컬럼용, `align_right`의
/// 반대).
pub fn align_left(items: &[String], width: usize) -> Vec<String> {
    items
        .iter()
        .map(|s| {
            if s.chars().count() >= width {
                s.clone()
            } else {
                format!("{}{}", s, " ".repeat(width - s.chars().count()))
            }
        })
        .collect()
}

/// 여러 줄 텍스트에서 지정 개수만큼 앞부분만 남기고 나머지는 생략
/// 표시("... N줄 더 있음")로 바꾼다(긴 출력을 터미널에 요약할 때 쓴다).
pub fn truncate_output_lines(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        return text.to_string();
    }
    let shown: Vec<&str> = lines.iter().take(max_lines).copied().collect();
    format!("{}\n... {}줄 더 있음", shown.join("\n"), lines.len() - max_lines)
}

/// 인자 목록에서 중복된 값을 제거한다(순서 유지, 첫 등장만 남김).
pub fn dedup_preserve_order(args: &[String]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for a in args {
        if !seen.contains(a) {
            seen.push(a.clone());
        }
    }
    seen
}

/// 문자열이 서브커맨드 이름으로 쓰기에 유효한 형태인지(영문 소문자와
/// 하이픈만) 검사한다(사용자 입력 사전 점검용).
pub fn is_valid_command_token(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

/// 여러 줄 텍스트의 각 줄에 1부터 시작하는 번호를 붙인다(출력 디버깅용).
pub fn number_lines(text: &str) -> String {
    text.lines().enumerate().map(|(i, l)| format!("{}: {l}", i + 1)).collect::<Vec<_>>().join("\n")
}

/// 문자열을 소문자로 바꾸고 앞뒤 공백을 제거한 정규화 버전을 만든다
/// (커맨드/플래그 이름 비교 전 정규화용).
pub fn normalize_token(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

/// 인자 목록 중 특정 접두사로 시작하는 항목의 개수를 센다.
pub fn count_with_prefix(args: &[String], prefix: &str) -> usize {
    args.iter().filter(|a| a.starts_with(prefix)).count()
}
