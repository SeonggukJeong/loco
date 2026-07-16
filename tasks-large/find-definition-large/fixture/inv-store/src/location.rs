//! 창고 위치 코드 정규화.
//!
//! 입고/출고 배치는 위치 필드에 대문자/소문자, 밑줄/공백/슬래시 등
//! 표기가 뒤섞인 채 들어온다("sel1_a01", "SEL1 A01", "sel1/a01" 모두 같은
//! 위치를 가리킨다). 저장소에 반영하기 전 이 함수로 정규화해 키 비교가
//! 안정적으로 동작하도록 한다.

/// 위치 문자열을 정규화한다: 앞뒤 공백 제거 → 대문자화 → 구분자(공백/
/// 밑줄/슬래시)를 하이픈 하나로 통일(연속 구분자는 하나로 합치고, 끝에
/// 남는 하이픈은 제거).
pub fn normalize_location(raw: &str) -> String {
    let trimmed = raw.trim();
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

/// 정규화된 위치 문자열이 형식상 유효한지(빈 문자열이 아니고, 하이픈으로
/// 시작/끝나지 않는지) 검사한다.
pub fn is_valid_normalized(location: &str) -> bool {
    !location.is_empty() && !location.starts_with('-') && !location.ends_with('-')
}

/// 두 위치 문자열이 정규화 후 같은 위치를 가리키는지 비교한다.
pub fn same_location(a: &str, b: &str) -> bool {
    normalize_location(a) == normalize_location(b)
}

/// 정규화된 위치에서 첫 번째 세그먼트(보통 창고 코드)만 뽑아낸다.
pub fn location_prefix(location: &str) -> String {
    let normalized = normalize_location(location);
    normalized.split('-').next().unwrap_or("").to_string()
}

/// 정규화된 위치의 세그먼트 개수를 센다(예: "SEL1-A01-03" -> 3).
pub fn segment_count(location: &str) -> usize {
    let normalized = normalize_location(location);
    if normalized.is_empty() {
        0
    } else {
        normalized.split('-').count()
    }
}

/// 위치 문자열 목록을 정규화해 중복 없이 정렬된 목록으로 만든다.
pub fn normalize_and_dedup(locations: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = locations.iter().map(|l| normalize_location(l)).collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

/// 위치 문자열이 같은 창고 접두(첫 세그먼트)를 공유하는지 비교한다.
pub fn same_warehouse_prefix(a: &str, b: &str) -> bool {
    let pa = location_prefix(a);
    !pa.is_empty() && pa == location_prefix(b)
}

/// 위치 문자열 목록에서 특정 창고 접두를 가진 것만 걸러낸다.
pub fn filter_by_prefix(locations: &[String], prefix: &str) -> Vec<String> {
    let target = prefix.trim().to_ascii_uppercase();
    locations.iter().filter(|l| location_prefix(l) == target).cloned().collect()
}

/// 정규화된 위치에서 마지막 세그먼트(보통 슬롯 번호)만 뽑아낸다.
pub fn location_suffix(location: &str) -> String {
    let normalized = normalize_location(location);
    normalized.split('-').next_back().unwrap_or("").to_string()
}

/// 위치 문자열이 최소 세그먼트 수(예: 창고+구역+슬롯 3단계) 요건을
/// 만족하는지 검사한다.
pub fn has_min_segments(location: &str, min_segments: usize) -> bool {
    segment_count(location) >= min_segments
}

/// 위치 목록을 정규화 후 사전순으로 정렬한다.
pub fn sort_normalized(locations: &mut Vec<String>) {
    for l in locations.iter_mut() {
        *l = normalize_location(l);
    }
    locations.sort();
}

/// 두 위치 문자열 목록이 정규화 후 같은 집합을 이루는지 비교한다.
pub fn same_location_set(a: &[String], b: &[String]) -> bool {
    let mut na = normalize_and_dedup(a);
    let mut nb = normalize_and_dedup(b);
    na.sort();
    nb.sort();
    na == nb
}
