//! SKU/항목 순위 매기기.
//!
//! 이미 집계된 (이름, 값) 쌍 목록을 받아 상위/하위 랭킹을 뽑는 함수를
//! 모아둔다. 값의 출처(합계 버전, 단위 등)는 신경 쓰지 않는다.

/// 순위가 매겨진 항목 한 건.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedItem {
    pub rank: usize,
    pub name: String,
    pub value: i64,
}

/// (이름, 값) 목록을 값 내림차순으로 정렬해 순위를 매긴다(1부터 시작,
/// 동률은 이름 오름차순으로 2차 정렬).
pub fn rank_desc(items: &[(String, i64)]) -> Vec<RankedItem> {
    let mut sorted = items.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted
        .into_iter()
        .enumerate()
        .map(|(i, (name, value))| RankedItem { rank: i + 1, name, value })
        .collect()
}

/// (이름, 값) 목록을 값 오름차순으로 정렬해 순위를 매긴다(하위 랭킹용).
pub fn rank_asc(items: &[(String, i64)]) -> Vec<RankedItem> {
    let mut sorted = items.to_vec();
    sorted.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    sorted
        .into_iter()
        .enumerate()
        .map(|(i, (name, value))| RankedItem { rank: i + 1, name, value })
        .collect()
}

/// 값 기준 상위 N개 항목만 뽑는다.
pub fn top_n(items: &[(String, i64)], n: usize) -> Vec<RankedItem> {
    rank_desc(items).into_iter().take(n).collect()
}

/// 값 기준 하위 N개 항목만 뽑는다.
pub fn bottom_n(items: &[(String, i64)], n: usize) -> Vec<RankedItem> {
    rank_asc(items).into_iter().take(n).collect()
}

/// 특정 이름의 순위를 찾는다(없으면 `None`).
pub fn rank_of(ranked: &[RankedItem], name: &str) -> Option<usize> {
    ranked.iter().find(|item| item.name == name).map(|item| item.rank)
}

/// 순위 목록을 사람이 읽는 텍스트(줄바꿈 구분)로 포맷한다.
pub fn format_ranking(ranked: &[RankedItem]) -> String {
    ranked.iter().map(|item| format!("{}. {} ({}원)", item.rank, item.name, item.value)).collect::<Vec<_>>().join("\n")
}

/// 순위 목록에서 상위 percentile(%) 안에 드는 항목만 걸러낸다.
///
/// 예: `percentile = 10`이면 상위 10%에 해당하는 항목만 반환한다.
pub fn top_percentile(items: &[(String, i64)], percentile: u32) -> Vec<RankedItem> {
    let ranked = rank_desc(items);
    let cutoff = ((ranked.len() * percentile as usize) / 100).max(1);
    ranked.into_iter().take(cutoff).collect()
}

/// 두 랭킹(예: 이번 달/지난 달)을 비교해 순위가 오른 항목의 이름 목록을 찾는다.
pub fn risers(current: &[RankedItem], previous: &[RankedItem]) -> Vec<String> {
    current
        .iter()
        .filter_map(|c| {
            previous.iter().find(|p| p.name == c.name).and_then(|p| if c.rank < p.rank { Some(c.name.clone()) } else { None })
        })
        .collect()
}

/// 두 랭킹을 비교해 순위가 내려간 항목의 이름 목록을 찾는다.
pub fn fallers(current: &[RankedItem], previous: &[RankedItem]) -> Vec<String> {
    current
        .iter()
        .filter_map(|c| {
            previous.iter().find(|p| p.name == c.name).and_then(|p| if c.rank > p.rank { Some(c.name.clone()) } else { None })
        })
        .collect()
}

/// 값이 모두 동일한(동률 랭킹인) 항목이 있는지 검사한다.
pub fn has_ties(items: &[(String, i64)]) -> bool {
    let mut values: Vec<i64> = items.iter().map(|(_, v)| *v).collect();
    values.sort();
    let before = values.len();
    values.dedup();
    values.len() != before
}

/// 순위 목록에서 특정 순위 구간(양 끝 포함)에 속한 항목만 걸러낸다.
pub fn in_rank_range(ranked: &[RankedItem], min_rank: usize, max_rank: usize) -> Vec<RankedItem> {
    ranked.iter().filter(|item| item.rank >= min_rank && item.rank <= max_rank).cloned().collect()
}

/// 순위 목록의 값 합계를 구한다.
pub fn total_value(ranked: &[RankedItem]) -> i64 {
    ranked.iter().map(|item| item.value).sum()
}

/// 순위 목록에서 상위 N개가 전체 값에서 차지하는 비율(%)을 계산한다
/// (집중도 지표 — 예: 상위 20% SKU가 매출의 몇 %를 차지하는지).
pub fn top_n_share_percent(items: &[(String, i64)], n: usize) -> u32 {
    let total: i64 = items.iter().map(|(_, v)| v).sum();
    if total <= 0 {
        return 0;
    }
    let top_total: i64 = top_n(items, n).iter().map(|r| r.value).sum();
    ((top_total.saturating_mul(100)) / total).clamp(0, 100) as u32
}

/// 순위 목록에서 값이 0 이하인 항목의 개수를 센다(비정상/누락 데이터 지표).
pub fn non_positive_count(items: &[(String, i64)]) -> usize {
    items.iter().filter(|(_, v)| *v <= 0).count()
}

/// 두 시점의 랭킹을 비교해 이번에 새로 순위표에 등장한 이름 목록을 찾는다.
pub fn newcomers(current: &[RankedItem], previous: &[RankedItem]) -> Vec<String> {
    current.iter().filter(|c| !previous.iter().any(|p| p.name == c.name)).map(|c| c.name.clone()).collect()
}

/// 두 시점의 랭킹을 비교해 이번에 순위표에서 사라진 이름 목록을 찾는다.
pub fn dropouts(current: &[RankedItem], previous: &[RankedItem]) -> Vec<String> {
    previous.iter().filter(|p| !current.iter().any(|c| c.name == p.name)).map(|p| p.name.clone()).collect()
}
