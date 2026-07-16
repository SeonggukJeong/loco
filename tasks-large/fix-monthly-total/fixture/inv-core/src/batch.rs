//! 배치(batch) 처리 보조 타입/헬퍼.
//!
//! 원장 라인이나 이동 요청을 일정 크기로 묶어 처리할 때 쓰는 순수 유틸이다.
//! "배치"라는 이름은 `config::DEFAULT_BATCH_SIZE`와 `rules::mod`의 배치
//! 관련 검사 함수들과도 관련 있지만, 여기서는 청크 분할 자체에 집중한다.

/// 배치 메타데이터.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchInfo {
    pub id: String,
    pub size: u32,
    pub created_epoch: i64,
}

impl BatchInfo {
    pub fn new(id: impl Into<String>, size: u32, created_epoch: i64) -> Self {
        BatchInfo { id: id.into(), size, created_epoch }
    }
}

/// 슬라이스를 지정된 크기의 청크로 나눈다(마지막 청크는 남는 만큼만).
pub fn chunk<T: Clone>(items: &[T], size: usize) -> Vec<Vec<T>> {
    if size == 0 {
        return vec![items.to_vec()];
    }
    items.chunks(size).map(|c| c.to_vec()).collect()
}

/// 총 항목 수를 배치 크기로 나눴을 때 필요한 배치 개수를 계산한다.
pub fn batch_count(total_items: u32, batch_size: u32) -> u32 {
    if batch_size == 0 {
        0
    } else {
        (total_items + batch_size - 1) / batch_size
    }
}

/// 배치 ID를 생성한다("BATCH-<순번>" 형식, 순번은 4자리 0-패딩).
pub fn generate_batch_id(sequence: u32) -> String {
    format!("BATCH-{sequence:04}")
}

/// 배치 크기가 정책상 허용 범위(1~10000)인지 검사한다.
pub fn is_valid_batch_size(size: u32) -> bool {
    size > 0 && size <= 10_000
}

/// 배치가 비어 있는지(size == 0) 판정한다.
pub fn is_empty_batch(info: &BatchInfo) -> bool {
    info.size == 0
}

/// 두 배치를 병합했을 때의 총 크기를 계산한다.
pub fn merged_size(a: &BatchInfo, b: &BatchInfo) -> u32 {
    a.size.saturating_add(b.size)
}

/// 배치 목록 중 특정 시각 이후 생성된 것만 걸러낸다.
pub fn created_after<'a>(batches: &'a [BatchInfo], epoch: i64) -> Vec<&'a BatchInfo> {
    batches.iter().filter(|b| b.created_epoch > epoch).collect()
}

/// 배치 목록의 총 항목 수 합계.
pub fn total_size(batches: &[BatchInfo]) -> u32 {
    batches.iter().map(|b| b.size).sum()
}

/// ID로 배치를 찾는다.
pub fn find_by_id<'a>(batches: &'a [BatchInfo], id: &str) -> Option<&'a BatchInfo> {
    batches.iter().find(|b| b.id == id)
}

/// 배치 목록을 생성 시각 오름차순으로 정렬한다.
pub fn sort_chronological(batches: &mut Vec<BatchInfo>) {
    batches.sort_by_key(|b| b.created_epoch);
}

/// 두 슬라이스를 지정된 크기로 나눠 짝지은 청크 쌍을 만든다(같은 인덱스끼리
/// 병렬 처리할 때 쓰는 헬퍼 — 길이가 다르면 짧은 쪽 기준으로 자른다).
pub fn zip_chunks<T: Clone, U: Clone>(a: &[T], b: &[U], size: usize) -> Vec<(Vec<T>, Vec<U>)> {
    let a_chunks = chunk(a, size);
    let b_chunks = chunk(b, size);
    a_chunks.into_iter().zip(b_chunks).collect()
}

/// 배치 크기 목록의 평균을 계산한다(빈 목록이면 0.0).
pub fn average_batch_size(batches: &[BatchInfo]) -> f64 {
    if batches.is_empty() {
        return 0.0;
    }
    total_size(batches) as f64 / batches.len() as f64
}

/// 가장 큰 배치를 찾는다(동률이면 먼저 등장한 것).
pub fn largest_batch(batches: &[BatchInfo]) -> Option<&BatchInfo> {
    batches.iter().max_by_key(|b| b.size)
}

/// 배치 목록 중 지정 크기 이상인 것만 걸러낸다("대형 배치" 필터).
pub fn large_batches(batches: &[BatchInfo], min_size: u32) -> Vec<BatchInfo> {
    batches.iter().filter(|b| b.size >= min_size).cloned().collect()
}

/// 배치 ID가 예상 접두("BATCH-")로 시작하는지 검사한다.
pub fn has_expected_prefix(id: &str) -> bool {
    id.starts_with("BATCH-")
}
