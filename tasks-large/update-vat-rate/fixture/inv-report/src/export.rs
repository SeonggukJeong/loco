//! 보고서 데이터를 텍스트로 직렬화하는 내보내기(export) 헬퍼.
//!
//! 실제 파일 쓰기는 이 크레이트의 책임이 아니다(그건 inv-cli의 몫이다) —
//! 여기서는 "보고서 데이터를 어떤 텍스트 형식으로 표현할지"만 다룬다.

/// CSV 한 필드를 이스케이프한다(쉼표/따옴표/개행이 있으면 큰따옴표로 감싼다).
pub fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// 여러 필드를 CSV 한 줄로 조립한다.
pub fn csv_row(fields: &[String]) -> String {
    fields.iter().map(|f| escape_csv_field(f)).collect::<Vec<_>>().join(",")
}

/// (라벨, 값) 목록을 2열 CSV 텍스트(헤더 포함)로 내보낸다.
pub fn export_key_value_csv(header: (&str, &str), rows: &[(String, i64)]) -> String {
    let mut out = csv_row(&[header.0.to_string(), header.1.to_string()]);
    out.push('\n');
    for (label, value) in rows {
        out.push_str(&csv_row(&[label.clone(), value.to_string()]));
        out.push('\n');
    }
    out
}

/// (라벨, 값) 목록을 탭 구분(TSV) 텍스트로 내보낸다.
pub fn export_tsv(rows: &[(String, i64)]) -> String {
    rows.iter().map(|(label, value)| format!("{label}\t{value}")).collect::<Vec<_>>().join("\n")
}

/// 값 목록을 JSON 배열 형태의 텍스트로 직렬화한다(외부 JSON 라이브러리
/// 없이 정수 배열만 다루는 최소 구현).
pub fn export_json_array(values: &[i64]) -> String {
    let joined = values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
    format!("[{joined}]")
}

/// (라벨, 값) 목록을 JSON 객체 형태의 텍스트로 직렬화한다(라벨은 그대로
/// 키로 쓰며 별도 이스케이프는 하지 않는다 — 내부 리포트 라벨만 다루는
/// 전제).
pub fn export_json_object(rows: &[(String, i64)]) -> String {
    let joined = rows.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect::<Vec<_>>().join(",");
    format!("{{{joined}}}")
}

/// 내보낼 텍스트의 줄 수를 센다(내보내기 전 크기 가늠용).
pub fn count_export_lines(text: &str) -> usize {
    text.lines().count()
}

/// CSV 텍스트 한 줄을 필드 목록으로 되돌린다(따옴표 처리 없는 단순
/// 파서 — 이 모듈이 만든 이스케이프 없는 출력만 되돌리는 용도).
pub fn split_simple_csv_line(line: &str) -> Vec<String> {
    line.split(',').map(|s| s.to_string()).collect()
}

/// 여러 CSV 텍스트 블록을 헤더를 한 번만 남기고 이어붙인다(여러 배치를
/// 하나의 파일로 합쳐 내보낼 때 쓴다).
pub fn concat_csv_blocks(blocks: &[String]) -> String {
    let mut out = String::new();
    for (i, block) in blocks.iter().enumerate() {
        let mut lines = block.lines();
        if i == 0 {
            if let Some(header) = lines.next() {
                out.push_str(header);
                out.push('\n');
            }
        } else {
            lines.next();
        }
        for line in lines {
            if !line.trim().is_empty() {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

/// 내보내기 형식 이름이 이 모듈이 지원하는 값인지 검사한다.
pub fn is_supported_format(format: &str) -> bool {
    matches!(format.to_ascii_lowercase().as_str(), "csv" | "tsv" | "json")
}

/// 파일 확장자로부터 내보내기 형식을 추정한다(알 수 없으면 "csv" 기본값).
pub fn format_from_extension(filename: &str) -> String {
    match filename.rsplit('.').next() {
        Some(ext) if is_supported_format(ext) => ext.to_ascii_lowercase(),
        _ => "csv".to_string(),
    }
}

/// (라벨, 값) 목록을 마크다운 표 텍스트로 내보낸다(헤더 + 구분선 + 행).
pub fn export_markdown_table(header: (&str, &str), rows: &[(String, i64)]) -> String {
    let mut out = format!("| {} | {} |\n| --- | --- |\n", header.0, header.1);
    for (label, value) in rows {
        out.push_str(&format!("| {label} | {value} |\n"));
    }
    out
}

/// 값 목록을 공백 구분 텍스트 한 줄로 내보낸다(로그 라인 등 아주 단순한
/// 포맷이 필요할 때 쓴다).
pub fn export_space_separated(values: &[i64]) -> String {
    values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ")
}

/// 내보낼 (라벨, 값) 목록에서 라벨에 CSV 이스케이프가 필요한 항목의
/// 개수를 센다(내보내기 전 사전 점검용).
pub fn count_needing_escape(rows: &[(String, i64)]) -> usize {
    rows.iter().filter(|(label, _)| escape_csv_field(label) != *label).count()
}

/// CSV 텍스트 전체(헤더 포함)의 데이터 행 수를 센다(헤더 제외).
pub fn count_data_rows(csv_text: &str) -> usize {
    csv_text.lines().skip(1).filter(|l| !l.trim().is_empty()).count()
}
