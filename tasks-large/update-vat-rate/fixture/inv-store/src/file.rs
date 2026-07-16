//! 파일 영속화 포맷의 인코딩/디코딩.
//!
//! 실제 디스크 I/O(열기/쓰기/플러시)는 이 크레이트의 책임이 아니다(그건
//! inv-cli가 한다) — 여기서는 "저장소 상태를 텍스트 한 줄짜리 로그
//! 포맷으로 어떻게 표현할지"만 다룬다. 그래야 파일 시스템 없이도 포맷
//! 로직을 단위 테스트할 수 있다.

use inv_core::inventory::InventorySnapshot;

/// 로그 포맷 버전 헤더 줄(첫 줄에 위치).
pub const FORMAT_HEADER: &str = "INV-STORE-LOG v1";

/// 인코딩/디코딩 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    MissingHeader,
    WrongFieldCount { line_no: usize, expected: usize, actual: usize },
    BadNumber { line_no: usize, field: String },
}

/// 스냅샷 하나를 로그 한 줄로 인코딩한다: `sku|on_hand|reserved`.
pub fn encode_snapshot(snapshot: &InventorySnapshot) -> String {
    format!("{}|{}|{}", snapshot.sku, snapshot.on_hand, snapshot.reserved)
}

/// 로그 한 줄을 스냅샷으로 디코딩한다.
pub fn decode_snapshot(line: &str, line_no: usize) -> Result<InventorySnapshot, FormatError> {
    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() != 3 {
        return Err(FormatError::WrongFieldCount { line_no, expected: 3, actual: fields.len() });
    }
    let sku = fields[0].to_string();
    let on_hand = fields[1]
        .parse::<u32>()
        .map_err(|_| FormatError::BadNumber { line_no, field: fields[1].to_string() })?;
    let reserved = fields[2]
        .parse::<u32>()
        .map_err(|_| FormatError::BadNumber { line_no, field: fields[2].to_string() })?;
    Ok(InventorySnapshot::new(sku, on_hand, reserved))
}

/// 스냅샷 목록 전체를 헤더 포함 로그 텍스트로 인코딩한다.
pub fn encode_log(snapshots: &[InventorySnapshot]) -> String {
    let mut out = String::new();
    out.push_str(FORMAT_HEADER);
    out.push('\n');
    for s in snapshots {
        out.push_str(&encode_snapshot(s));
        out.push('\n');
    }
    out
}

/// 로그 텍스트 전체를 디코딩한다. 헤더가 없으면 오류.
pub fn decode_log(text: &str) -> Result<Vec<InventorySnapshot>, FormatError> {
    let mut lines = text.lines();
    match lines.next() {
        Some(h) if h.trim() == FORMAT_HEADER => {}
        _ => return Err(FormatError::MissingHeader),
    }
    let mut snapshots = Vec::new();
    for (idx, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        snapshots.push(decode_snapshot(line, idx + 2)?);
    }
    Ok(snapshots)
}

/// 로그 텍스트가 유효한 헤더로 시작하는지만 빠르게 검사한다(전체 디코딩
/// 없이 형식 사전 점검용).
pub fn has_valid_header(text: &str) -> bool {
    text.lines().next().map(|h| h.trim() == FORMAT_HEADER).unwrap_or(false)
}

/// 하나의 저장소 상태 변화를 나타내는 로그 엔트리(추가 전용 로그의 한 항목).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub sku: String,
    pub delta: i64,
    pub seq: u64,
}

/// 로그 엔트리 하나를 인코딩한다: `seq|sku|delta`.
pub fn encode_entry(entry: &LogEntry) -> String {
    format!("{}|{}|{}", entry.seq, entry.sku, entry.delta)
}

/// 로그 엔트리 한 줄을 디코딩한다.
pub fn decode_entry(line: &str, line_no: usize) -> Result<LogEntry, FormatError> {
    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() != 3 {
        return Err(FormatError::WrongFieldCount { line_no, expected: 3, actual: fields.len() });
    }
    let seq = fields[0].parse::<u64>().map_err(|_| FormatError::BadNumber { line_no, field: fields[0].to_string() })?;
    let sku = fields[1].to_string();
    let delta =
        fields[2].parse::<i64>().map_err(|_| FormatError::BadNumber { line_no, field: fields[2].to_string() })?;
    Ok(LogEntry { sku, delta, seq })
}

/// 추가 전용 로그(엔트리 목록)를 초기 스냅샷 목록에 순서대로 재생(replay)
/// 해 최종 상태를 계산한다.
pub fn replay(initial: &[InventorySnapshot], entries: &[LogEntry]) -> Vec<InventorySnapshot> {
    let mut result = initial.to_vec();
    for entry in entries {
        if let Some(s) = result.iter_mut().find(|s| s.sku == entry.sku) {
            let new_value = (s.on_hand as i64 + entry.delta).max(0);
            s.on_hand = new_value as u32;
        } else if entry.delta > 0 {
            result.push(InventorySnapshot::new(entry.sku.clone(), entry.delta as u32, 0));
        }
    }
    result
}

/// 시퀀스 번호가 순서대로(중복/역행 없이) 증가하는지 검사한다(로그 무결성
/// 점검용).
pub fn seq_is_monotonic(entries: &[LogEntry]) -> bool {
    entries.windows(2).all(|w| w[1].seq > w[0].seq)
}

/// 로그 엔트리 목록 중 연속된 중복(같은 seq)이 있는지 검사한다.
pub fn has_duplicate_seq(entries: &[LogEntry]) -> bool {
    let mut seqs: Vec<u64> = entries.iter().map(|e| e.seq).collect();
    seqs.sort();
    seqs.windows(2).any(|w| w[0] == w[1])
}

/// 같은 SKU에 대한 연속 엔트리를 하나로 압축한다(로그 컴팩션 — 파일 크기
/// 절감용).
pub fn compact(entries: &[LogEntry]) -> Vec<LogEntry> {
    let mut skus: Vec<String> = entries.iter().map(|e| e.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus.into_iter()
        .enumerate()
        .map(|(i, sku)| {
            let net: i64 = entries.iter().filter(|e| e.sku == sku).map(|e| e.delta).sum();
            LogEntry { sku, delta: net, seq: i as u64 }
        })
        .collect()
}

/// 로그 텍스트(엔트리 형식, 헤더 포함)를 인코딩한다.
pub fn encode_entry_log(entries: &[LogEntry]) -> String {
    let mut out = String::new();
    out.push_str(FORMAT_HEADER);
    out.push('\n');
    for e in entries {
        out.push_str(&encode_entry(e));
        out.push('\n');
    }
    out
}

/// 로그 텍스트(엔트리 형식)를 디코딩한다.
pub fn decode_entry_log(text: &str) -> Result<Vec<LogEntry>, FormatError> {
    let mut lines = text.lines();
    match lines.next() {
        Some(h) if h.trim() == FORMAT_HEADER => {}
        _ => return Err(FormatError::MissingHeader),
    }
    let mut entries = Vec::new();
    for (idx, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        entries.push(decode_entry(line, idx + 2)?);
    }
    Ok(entries)
}

/// 오류를 사람이 읽는 한국어 메시지로 바꾼다.
pub fn describe_error(err: &FormatError) -> String {
    match err {
        FormatError::MissingHeader => format!("헤더가 없거나 형식이 다릅니다(예상: \"{FORMAT_HEADER}\")"),
        FormatError::WrongFieldCount { line_no, expected, actual } => {
            format!("{line_no}번째 줄: 필드 수 불일치(예상 {expected}개, 실제 {actual}개)")
        }
        FormatError::BadNumber { line_no, field } => format!("{line_no}번째 줄: 숫자 파싱 실패('{field}')"),
    }
}

/// 스냅샷 목록을 인코딩했다가 다시 디코딩했을 때 원본과 같은지 검사한다
/// (포맷 왕복 무결성 자기 검증용).
pub fn round_trip_preserves_snapshots(snapshots: &[InventorySnapshot]) -> bool {
    match decode_log(&encode_log(snapshots)) {
        Ok(decoded) => decoded == snapshots,
        Err(_) => false,
    }
}

/// 로그 텍스트의 데이터 줄(헤더 제외) 개수를 센다(빈 줄은 제외).
pub fn count_data_lines(text: &str) -> usize {
    text.lines().skip(1).filter(|l| !l.trim().is_empty()).count()
}

/// 두 로그 텍스트를 이어붙인다(뒤 텍스트의 헤더 줄은 제거하고 데이터
/// 줄만 이어붙인다 — 여러 배치를 하나의 파일로 합칠 때 사용).
pub fn concat_logs(a: &str, b: &str) -> String {
    let mut out = a.trim_end().to_string();
    out.push('\n');
    for line in b.lines().skip(1) {
        if !line.trim().is_empty() {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// 로그 엔트리 목록의 총 순변화량(delta 합)을 계산한다.
pub fn total_entry_delta(entries: &[LogEntry]) -> i64 {
    entries.iter().map(|e| e.delta).sum()
}

/// 스냅샷 로그 텍스트에서 특정 SKU 한 줄만 찾아 디코딩한다(전체를 다
/// 파싱하지 않고 필요한 것만 뽑는 가벼운 조회 — 대용량 로그 미리보기용).
pub fn find_snapshot_line(text: &str, sku: &str) -> Option<InventorySnapshot> {
    text.lines().skip(1).find_map(|line| {
        let fields: Vec<&str> = line.split('|').collect();
        if fields.first() == Some(&sku) {
            decode_snapshot(line, 0).ok()
        } else {
            None
        }
    })
}

/// 스냅샷 로그 텍스트에서 헤더를 제외한 줄 수와 실제로 디코딩에 성공한
/// 줄 수를 함께 반환한다(손상된 로그 파일 진단용).
pub fn diagnose(text: &str) -> (usize, usize) {
    let data_lines: Vec<&str> = text.lines().skip(1).filter(|l| !l.trim().is_empty()).collect();
    let ok_count = data_lines.iter().enumerate().filter(|(i, l)| decode_snapshot(l, *i).is_ok()).count();
    (data_lines.len(), ok_count)
}

/// 엔트리 로그를 SKU 기준으로 필터링해 새 로그 텍스트를 만든다.
pub fn filter_entry_log_by_sku(entries: &[LogEntry], sku: &str) -> Vec<LogEntry> {
    entries.iter().filter(|e| e.sku == sku).cloned().collect()
}

/// 로그 엔트리 목록에서 가장 큰(절대값 기준) delta를 가진 엔트리를 찾는다.
pub fn largest_delta_entry(entries: &[LogEntry]) -> Option<&LogEntry> {
    entries.iter().max_by_key(|e| e.delta.abs())
}

/// 스냅샷 목록을 SKU 오름차순으로 정렬한 뒤 인코딩한다(파일 출력 시
/// 항상 같은 순서를 보장해 diff 도구로 비교하기 쉽게 만든다).
pub fn encode_log_sorted(snapshots: &[InventorySnapshot]) -> String {
    let mut sorted = snapshots.to_vec();
    sorted.sort_by(|a, b| a.sku.cmp(&b.sku));
    encode_log(&sorted)
}
