//! 옛 보고서 출력 계층.
//!
//! `report` 모듈이 만들어지기 전에 쓰이던 고정폭 텍스트 출력 포맷이다.
//! 값 계산은 전혀 하지 않고, 이미 계산된 숫자를 받아 옛 사내 배치가
//! 기대하는 문자열 모양으로만 바꾼다 — 그래서 합계 로직(v1/v2 어느 쪽도)에
//! 직접 의존하지 않는다. 일부 구형 리포트 뷰어가 아직 이 포맷을 파싱하고
//! 있어 당분간 유지한다.

/// 고정폭 텍스트 한 줄로 라벨/값 쌍을 렌더링한다("라벨........값" 형태).
pub fn render_line(label: &str, value: i64, width: usize) -> String {
    let value_str = value.to_string();
    let dots = width.saturating_sub(label.chars().count() + value_str.chars().count());
    format!("{label}{}{value_str}", ".".repeat(dots.max(1)))
}

/// 여러 (라벨, 값) 쌍을 고정폭 텍스트 블록으로 렌더링한다.
pub fn render_block(rows: &[(&str, i64)], width: usize) -> String {
    rows.iter().map(|(label, value)| render_line(label, *value, width)).collect::<Vec<_>>().join("\n")
}

/// 옛 포맷의 구분선(등호 반복)을 만든다.
pub fn separator(width: usize) -> String {
    "=".repeat(width)
}

/// 제목 + 구분선을 포함한 옛 포맷 헤더를 만든다.
pub fn render_header(title: &str, width: usize) -> String {
    format!("{title}\n{}", separator(width))
}

/// 옛 포맷이 기대하는 통화 접미(" KRW")를 값 문자열에 붙인다.
pub fn with_currency_suffix(value: i64) -> String {
    format!("{value} KRW")
}

/// 옛 포맷의 퍼센트 표기(소수점 없이 정수 %)를 만든다.
pub fn render_percent(value: i64) -> String {
    format!("{value}%")
}

/// 옛 포맷 보고서 전체(헤더 + 본문 블록 + 구분선)를 조립한다. 본문에
/// 들어갈 값은 호출자가 이미 계산해 넘긴다 — 이 함수는 합계를 계산하지
/// 않는다.
pub fn compose_legacy_report(title: &str, rows: &[(&str, i64)]) -> String {
    let width = 40;
    format!("{}\n{}\n{}", render_header(title, width), render_block(rows, width), separator(width))
}

/// 옛 포맷 필드 구분자(파이프)로 값 목록을 이어붙인다(파싱 스크립트 호환).
pub fn join_pipe_delimited(values: &[i64]) -> String {
    values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("|")
}

/// 파이프 구분 텍스트를 다시 값 목록으로 파싱한다(왕복 검증용).
pub fn parse_pipe_delimited(text: &str) -> Vec<i64> {
    text.split('|').filter_map(|s| s.trim().parse::<i64>().ok()).collect()
}

/// 옛 포맷에서 음수 값을 괄호로 감싸는 회계 표기 관례를 적용한다.
pub fn accounting_format(value: i64) -> String {
    if value < 0 {
        format!("({})", value.abs())
    } else {
        value.to_string()
    }
}

/// 여러 값에 회계 표기를 일괄 적용한다.
pub fn accounting_format_all(values: &[i64]) -> Vec<String> {
    values.iter().map(|v| accounting_format(*v)).collect()
}

/// 옛 포맷 줄이 유효한 라벨-값 형태인지(콘텐츠가 비어 있지 않은지) 검사한다.
pub fn is_well_formed_line(line: &str) -> bool {
    !line.trim().is_empty() && line.chars().any(|c| c.is_ascii_digit())
}

/// 옛 포맷 텍스트 블록에서 형식이 맞지 않는 줄 수를 센다(마이그레이션 전
/// 사전 점검용).
pub fn count_malformed_lines(text: &str) -> usize {
    text.lines().filter(|l| !l.trim().is_empty() && !is_well_formed_line(l)).count()
}

/// 옛 포맷의 들여쓰기(공백 2칸) 접두를 붙인다(중첩 항목 표기용).
pub fn indent(line: &str, level: usize) -> String {
    format!("{}{}", "  ".repeat(level), line)
}

/// 여러 줄에 같은 들여쓰기 레벨을 일괄 적용한다.
pub fn indent_all(lines: &[String], level: usize) -> Vec<String> {
    lines.iter().map(|l| indent(l, level)).collect()
}

/// 옛 포맷의 각주(footnote) 표기를 만든다("* 내용" 형태).
pub fn footnote(text: &str) -> String {
    format!("* {text}")
}

/// 옛 포맷 보고서에 각주 여러 개를 덧붙인다.
pub fn append_footnotes(report_text: &str, notes: &[String]) -> String {
    if notes.is_empty() {
        return report_text.to_string();
    }
    let footnotes: Vec<String> = notes.iter().map(|n| footnote(n)).collect();
    format!("{report_text}\n\n{}", footnotes.join("\n"))
}

/// 값 목록을 옛 포맷의 회계 표기로 렌더링한 뒤 고정폭으로 맞춘다.
pub fn accounting_column(values: &[i64], width: usize) -> Vec<String> {
    accounting_format_all(values).iter().map(|s| pad_left_dots(s, width)).collect()
}

/// 문자열을 지정한 너비만큼 오른쪽 정렬한다(점이 아니라 공백으로 채운다 —
/// `accounting_column`처럼 값 컬럼을 나란히 맞출 때 쓴다).
fn pad_left_dots(s: &str, width: usize) -> String {
    if s.chars().count() >= width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(width - s.chars().count()), s)
    }
}

/// 옛 포맷 텍스트에서 각주로 시작하는 줄만 걸러낸다.
pub fn extract_footnotes(text: &str) -> Vec<String> {
    text.lines().filter(|l| l.trim_start().starts_with('*')).map(|l| l.to_string()).collect()
}
