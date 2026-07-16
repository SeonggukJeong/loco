//! 구 임포트 배치 스크립트에서 그대로 들고 온 위치 정규화/파싱 루틴.
//!
//! 2023년까지 쓰이던 별도 임포트 파이프라인의 잔재다. 신규 배치는 이
//! 경로를 타지 않지만, 과거에 이 스크립트로 적재된 데이터를 재처리해야
//! 할 일이 가끔 있어 당장은 지우지 않고 남겨둔다.

/// 위치 문자열을 정규화한다(구 임포트 스크립트 버전). 트림 단계에서
/// 리터럴 공백만 제거하고 탭/개행 등 다른 공백 문자는 남긴다 — 당시
/// 입력 파일이 항상 스페이스로만 구분되어 있다고 가정했던 구현이다.
pub fn normalize_location(raw: &str) -> String {
    let trimmed = raw.trim_matches(' ');
    let upper = trimmed.to_ascii_uppercase();
    let mut out = String::with_capacity(upper.len());
    let mut last_was_sep = false;
    for c in upper.chars() {
        if c == '_' || c == ' ' || c == '/' {
            if !last_was_sep && !out.is_empty() {
                out.push('-');
                last_was_sep = true;
            }
        } else {
            out.push(c);
            last_was_sep = false;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// 구 포맷의 임포트 레코드 한 줄(위치와 수량을 콜론으로 구분: "SEL1_A01:10").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportRecord {
    pub location: String,
    pub qty: i64,
}

/// 구 포맷 한 줄을 파싱한다.
pub fn parse_legacy_line(line: &str) -> Option<LegacyImportRecord> {
    let (loc_part, qty_part) = line.split_once(':')?;
    let qty = qty_part.trim().parse::<i64>().ok()?;
    Some(LegacyImportRecord { location: normalize_location(loc_part), qty })
}

/// 구 포맷 텍스트 전체(줄마다 한 레코드)를 파싱한다. 실패한 줄은 버린다.
pub fn parse_legacy_text(text: &str) -> Vec<LegacyImportRecord> {
    text.lines().filter(|l| !l.trim().is_empty()).filter_map(parse_legacy_line).collect()
}

/// 구 레코드 목록의 위치별 수량 합계를 구한다(같은 위치가 여러 줄로
/// 나뉘어 있던 구 배치 관행 때문에 필요했던 집계 단계).
pub fn sum_by_location(records: &[LegacyImportRecord]) -> Vec<(String, i64)> {
    let mut locations: Vec<String> = records.iter().map(|r| r.location.clone()).collect();
    locations.sort();
    locations.dedup();
    locations
        .into_iter()
        .map(|loc| {
            let total: i64 = records.iter().filter(|r| r.location == loc).map(|r| r.qty).sum();
            (loc, total)
        })
        .collect()
}

/// 구 레코드 목록에서 수량이 음수인(당시 반품 표기 관행) 것만 걸러낸다.
pub fn negative_qty_records(records: &[LegacyImportRecord]) -> Vec<LegacyImportRecord> {
    records.iter().filter(|r| r.qty < 0).cloned().collect()
}

/// 구 포맷 레코드를 현재 위치 표기로 재출력한다(마이그레이션 도구용).
pub fn to_modern_line(record: &LegacyImportRecord) -> String {
    format!("{}={}", record.location, record.qty)
}

/// 구 포맷 텍스트가 헤더 줄("LEGACY-IMPORT")로 시작하는지 검사한다.
/// 당시 배치 스크립트는 헤더가 없는 파일도 종종 섞여 있어, 있으면
/// 건너뛰고 없으면 그대로 첫 줄부터 파싱하는 관용적 처리를 했다.
pub fn strip_optional_header(text: &str) -> &str {
    match text.strip_prefix("LEGACY-IMPORT\n") {
        Some(rest) => rest,
        None => text,
    }
}

/// 구 포맷 레코드 목록에서 수량이 비정상적으로 큰(당시 기준 10만 초과)
/// 것만 걸러낸다 — 재처리 전 수동 확인 대상 표시용.
pub fn suspicious_qty_records(records: &[LegacyImportRecord]) -> Vec<LegacyImportRecord> {
    records.iter().filter(|r| r.qty.abs() > 100_000).cloned().collect()
}

/// 구 포맷 레코드 개수와 위치 종류 수를 함께 요약한다(재처리 전 사전
/// 점검 리포트용).
pub fn summarize(records: &[LegacyImportRecord]) -> (usize, usize) {
    let mut locations: Vec<&str> = records.iter().map(|r| r.location.as_str()).collect();
    locations.sort();
    locations.dedup();
    (records.len(), locations.len())
}
