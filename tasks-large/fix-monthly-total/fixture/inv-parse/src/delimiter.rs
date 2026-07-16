//! CSV 구분자 자동 감지.
//!
//! 대부분의 배치는 쉼표(,) 구분이지만, 일부 유럽계 벤더 시스템은 세미콜론
//! (;)을, 일부 사내 레거시 배치 스크립트는 탭(\t)을 쓴다. 파일 확장자만
//! 보고는 구분할 수 없어, 샘플 줄에서 후보 구분자별 등장 횟수를 세어
//! 가장 그럴듯한 것을 고른다.

/// 감지 후보 구분자 목록(우선순위 없음 — 등장 횟수로만 판단).
pub const CANDIDATE_DELIMITERS: [char; 3] = [',', ';', '\t'];

/// 한 줄에서 특정 구분자가 몇 번 등장하는지 센다(따옴표 내부도 구분 없이
/// 단순히 문자 등장 횟수만 센다 — 빠른 1차 추정용).
pub fn count_delimiter(line: &str, delim: char) -> usize {
    line.chars().filter(|c| *c == delim).count()
}

/// 샘플 줄 하나에서 가장 유력한 구분자를 고른다. 아무 후보도 등장하지
/// 않으면 기본값(쉼표)을 반환한다.
pub fn detect_from_line(line: &str) -> char {
    CANDIDATE_DELIMITERS
        .iter()
        .copied()
        .max_by_key(|d| count_delimiter(line, *d))
        .filter(|d| count_delimiter(line, *d) > 0)
        .unwrap_or(',')
}

/// 텍스트의 첫 몇 줄을 보고 구분자를 감지한다(첫 줄만으로는 오탐 가능성이
/// 있어 여러 줄의 다수결을 취한다).
pub fn detect_from_sample(text: &str, sample_lines: usize) -> char {
    let mut votes: Vec<char> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()).take(sample_lines) {
        votes.push(detect_from_line(line));
    }
    most_common(&votes).unwrap_or(',')
}

fn most_common(items: &[char]) -> Option<char> {
    let mut best: Option<(char, usize)> = None;
    for &item in items {
        let count = items.iter().filter(|&&x| x == item).count();
        best = match best {
            Some((_, best_count)) if best_count >= count => best,
            _ => Some((item, count)),
        };
    }
    best.map(|(c, _)| c)
}

/// 텍스트 전체가 감지된 구분자를 일관되게 쓰고 있는지 검사한다(줄마다
/// 등장 횟수가 동일해야 함 — 아니면 필드 수가 줄마다 달라져 파싱이
/// 어긋난다).
pub fn is_consistent(text: &str, delim: char) -> bool {
    let mut counts = text.lines().filter(|l| !l.trim().is_empty()).map(|l| count_delimiter(l, delim));
    match counts.next() {
        None => true,
        Some(first) => counts.all(|c| c == first),
    }
}

/// 구분자가 다른 줄의 번호(1부터 시작) 목록을 반환한다(일관성 위반 위치
/// 파악용).
pub fn inconsistent_line_numbers(text: &str, delim: char) -> Vec<usize> {
    let lines: Vec<&str> = text.lines().collect();
    let expected = lines
        .iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| count_delimiter(l, delim));
    let Some(expected) = expected else {
        return Vec::new();
    };
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && count_delimiter(l, delim) != expected)
        .map(|(i, _)| i + 1)
        .collect()
}

/// 구분자를 사람이 읽을 수 있는 이름으로 바꾼다(로그 출력용).
pub fn delimiter_name(delim: char) -> &'static str {
    match delim {
        ',' => "쉼표",
        ';' => "세미콜론",
        '\t' => "탭",
        _ => "알수없음",
    }
}

/// 감지된 구분자가 후보 목록에 속하는(알려진) 구분자인지 검사한다.
pub fn is_known_delimiter(delim: char) -> bool {
    CANDIDATE_DELIMITERS.contains(&delim)
}

/// 텍스트를 지정한 구분자로 다른 구분자(보통 쉼표)로 치환해 재작성한다.
/// 필드 안에 원래 목표 구분자가 있었다면 결과가 깨질 수 있어, 변환 전
/// `is_consistent`로 안전한지 먼저 확인하는 것을 권장한다.
pub fn rewrite_delimiter(text: &str, from: char, to: char) -> String {
    text.lines().map(|l| l.replace(from, &to.to_string())).collect::<Vec<_>>().join("\n")
}

/// 텍스트에서 후보 구분자별 총 등장 횟수를 모두 계산한다(진단 로그용).
pub fn tally_all_candidates(text: &str) -> Vec<(char, usize)> {
    CANDIDATE_DELIMITERS
        .iter()
        .map(|&d| (d, text.lines().map(|l| count_delimiter(l, d)).sum()))
        .collect()
}

/// 감지된 구분자가 실제로 그 줄에 한 번 이상 등장했는지(즉, 최소 2개
/// 필드로는 나뉘는지) 검사한다.
pub fn produces_multiple_fields(line: &str, delim: char) -> bool {
    count_delimiter(line, delim) > 0
}

/// 여러 샘플 파일에서 각각 감지한 구분자가 모두 같은지 검사한다(배치
/// 안에 형식이 다른 파일이 섞여 있지 않은지 확인).
pub fn all_same_delimiter(samples: &[&str]) -> bool {
    let mut detected = samples.iter().map(|s| detect_from_sample(s, 5));
    match detected.next() {
        None => true,
        Some(first) => detected.all(|d| d == first),
    }
}
