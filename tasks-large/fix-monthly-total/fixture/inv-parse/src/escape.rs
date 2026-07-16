//! CSV 필드 이스케이프/언이스케이프(왕복 직렬화용).
//!
//! `csv` 모듈의 `split_csv_line`이 읽기(디코딩) 방향을 담당한다면, 이
//! 모듈은 반대 방향(인코딩) — 파싱한 값을 다시 CSV 행으로 써야 할 때
//! (예: 정제된 데이터를 재출력하는 배치) 필요한 이스케이프 규칙을 담는다.

/// 필드에 이스케이프(따옴표 감싸기)가 필요한지 검사한다.
pub fn needs_escaping(field: &str) -> bool {
    field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r')
}

/// 필드 하나를 CSV 규칙에 맞게 이스케이프한다. 필요 없으면 그대로 반환.
pub fn escape_field(field: &str) -> String {
    if !needs_escaping(field) {
        return field.to_string();
    }
    let doubled = field.replace('"', "\"\"");
    format!("\"{doubled}\"")
}

/// 필드 목록을 하나의 CSV 행 문자열로 합친다.
pub fn escape_row(fields: &[String]) -> String {
    fields.iter().map(|f| escape_field(f)).collect::<Vec<_>>().join(",")
}

/// 여러 행을 CSV 텍스트(각 행 끝에 개행)로 합친다.
pub fn escape_rows(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for row in rows {
        out.push_str(&escape_row(row));
        out.push('\n');
    }
    out
}

/// 이스케이프된 필드에서 감싸는 따옴표와 이중 따옴표 이스케이프를 되돌린다.
///
/// `csv::split_csv_line`은 이미 이 처리를 인라인으로 하지만, 필드 하나를
/// 독립적으로 되돌려야 하는 경우(예: 재파싱 없이 필드 하나만 정제할 때)를
/// 위해 별도 함수로도 제공한다.
pub fn unescape_field(field: &str) -> String {
    let trimmed = field.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        let inner = &trimmed[1..trimmed.len() - 1];
        inner.replace("\"\"", "\"")
    } else {
        trimmed.to_string()
    }
}

/// 필드가 이미 이스케이프(따옴표로 감싸짐)된 형태인지 검사한다.
pub fn is_already_escaped(field: &str) -> bool {
    let t = field.trim();
    t.len() >= 2 && t.starts_with('"') && t.ends_with('"')
}

/// 필드 목록 중 이스케이프가 필요한 것의 개수를 센다(품질 점검용).
pub fn count_needing_escape(fields: &[String]) -> usize {
    fields.iter().filter(|f| needs_escaping(f)).count()
}

/// 필드 안의 개행 문자를 공백으로 치환한다(한 줄 로그 출력 등, 이스케이프
/// 대신 아예 값을 단순화해야 하는 상황용).
pub fn flatten_newlines(field: &str) -> String {
    field.replace(['\n', '\r'], " ")
}

/// 필드 목록을 세미콜론 구분 형식으로 이스케이프해 합친다(일부 유럽계
/// 벤더 시스템에 재출력할 때 쓰는 대안 포맷).
pub fn escape_row_semicolon(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| if f.contains(';') || f.contains('"') { escape_field(f) } else { f.clone() })
        .collect::<Vec<_>>()
        .join(";")
}

/// 필드가 이스케이프 후에도 원래 길이보다 늘어났는지(따옴표가 실제로
/// 추가됐는지) 검사한다.
pub fn escaping_changes_length(field: &str) -> bool {
    escape_field(field).len() != field.len()
}

/// 여러 필드 중 가장 긴 이스케이프 결과의 길이를 구한다(출력 폭 계산 등에 사용).
pub fn max_escaped_len(fields: &[String]) -> usize {
    fields.iter().map(|f| escape_field(f).len()).max().unwrap_or(0)
}

/// CSV 텍스트 하나를 다시 필드 행렬로 되돌린다(왕복 테스트/검증용 — 내부
/// 적으로는 `csv::split_csv_line`을 그대로 재사용한다).
pub fn parse_escaped_text(text: &str) -> Vec<Vec<String>> {
    text.lines().filter(|l| !l.is_empty()).map(crate::csv::split_csv_line).collect()
}

/// 이스케이프-역이스케이프 왕복이 원래 값을 보존하는지 검사한다(자기 자신
/// 테스트용 헬퍼).
pub fn round_trip_preserves(field: &str) -> bool {
    let escaped = escape_field(field);
    unescape_field(&escaped) == field
}
