//! 저장소 조회 헬퍼(필터/검색).
//!
//! `memory::MemoryStore`가 CRUD 기본 연산을 제공한다면, 이 모듈은 그
//! 위에서 자주 쓰이는 조회 패턴(접두 검색, 범위 필터 등)을 모아둔다.

use inv_core::inventory::InventorySnapshot;

/// SKU 접두사로 스냅샷을 검색한다.
pub fn search_by_sku_prefix<'a>(snapshots: &'a [InventorySnapshot], prefix: &str) -> Vec<&'a InventorySnapshot> {
    snapshots.iter().filter(|s| s.sku.starts_with(prefix)).collect()
}

/// 가용 재고가 지정 범위 안인 스냅샷만 걸러낸다.
pub fn filter_by_available_range(
    snapshots: &[InventorySnapshot],
    min: u32,
    max: u32,
) -> Vec<InventorySnapshot> {
    snapshots.iter().filter(|s| s.available() >= min && s.available() <= max).cloned().collect()
}

/// 재고가 가장 많은 상위 N개 SKU를 뽑는다.
pub fn top_n_by_on_hand(snapshots: &[InventorySnapshot], n: usize) -> Vec<InventorySnapshot> {
    let mut sorted = snapshots.to_vec();
    sorted.sort_by(|a, b| b.on_hand.cmp(&a.on_hand));
    sorted.into_iter().take(n).collect()
}

/// 재고가 가장 적은 하위 N개 SKU를 뽑는다(품절 임박 후보).
pub fn bottom_n_by_available(snapshots: &[InventorySnapshot], n: usize) -> Vec<InventorySnapshot> {
    let mut sorted = snapshots.to_vec();
    sorted.sort_by(|a, b| a.available().cmp(&b.available()));
    sorted.into_iter().take(n).collect()
}

/// 특정 SKU 목록에 해당하는 스냅샷만 순서를 유지하며 추린다.
pub fn select_by_skus<'a>(snapshots: &'a [InventorySnapshot], skus: &[String]) -> Vec<&'a InventorySnapshot> {
    skus.iter().filter_map(|sku| snapshots.iter().find(|s| &s.sku == sku)).collect()
}

/// 스냅샷 목록에서 조건을 만족하는 첫 항목을 찾는다(제네릭 술어 버전).
pub fn find_first<F>(snapshots: &[InventorySnapshot], predicate: F) -> Option<InventorySnapshot>
where
    F: Fn(&InventorySnapshot) -> bool,
{
    snapshots.iter().find(|s| predicate(s)).cloned()
}

/// 예약 재고 비율(예약/현재고, %)이 지정 임계값 이상인 스냅샷을 찾는다
/// (과다 예약 경보 후보).
pub fn high_reservation_ratio(snapshots: &[InventorySnapshot], threshold_percent: u32) -> Vec<InventorySnapshot> {
    snapshots
        .iter()
        .filter(|s| {
            if s.on_hand == 0 {
                s.reserved > 0
            } else {
                (s.reserved.saturating_mul(100) / s.on_hand) >= threshold_percent
            }
        })
        .cloned()
        .collect()
}

/// 페이지네이션: 스냅샷 목록에서 지정 구간(offset, limit)만 잘라낸다.
pub fn page(snapshots: &[InventorySnapshot], offset: usize, limit: usize) -> Vec<InventorySnapshot> {
    snapshots.iter().skip(offset).take(limit).cloned().collect()
}

/// SKU 목록 중 저장소에 존재하지 않는(조회 실패) SKU만 걸러낸다.
pub fn missing_skus(snapshots: &[InventorySnapshot], requested: &[String]) -> Vec<String> {
    requested.iter().filter(|sku| !snapshots.iter().any(|s| &s.sku == *sku)).cloned().collect()
}

/// 스냅샷 목록의 총 재고 가치를 계산한다(단가 목록과 SKU로 매칭, 단가
/// 미상인 SKU는 0으로 취급).
pub fn total_value_krw(snapshots: &[InventorySnapshot], unit_prices: &[(String, i64)]) -> i64 {
    snapshots
        .iter()
        .map(|s| {
            let price = unit_prices.iter().find(|(sku, _)| sku == &s.sku).map(|(_, p)| *p).unwrap_or(0);
            s.on_hand as i64 * price
        })
        .sum()
}

/// 스냅샷 목록을 가용 재고 기준 오름차순으로 정렬한 새 목록을 반환한다
/// (원본은 변경하지 않음 — `MemoryStore::sort_by_available_desc`의 읽기
/// 전용/반대 방향 버전).
pub fn sorted_by_available_asc(snapshots: &[InventorySnapshot]) -> Vec<InventorySnapshot> {
    let mut sorted = snapshots.to_vec();
    sorted.sort_by(|a, b| a.available().cmp(&b.available()));
    sorted
}

/// 스냅샷 목록에서 현재고가 예약보다 적은(데이터 이상 후보) 것들을 찾는다.
pub fn inconsistent_snapshots(snapshots: &[InventorySnapshot]) -> Vec<InventorySnapshot> {
    snapshots.iter().filter(|s| s.on_hand < s.reserved).cloned().collect()
}

/// 두 스냅샷 목록에 공통으로 존재하는 SKU만 걸러낸다.
pub fn intersecting_skus(a: &[InventorySnapshot], b: &[InventorySnapshot]) -> Vec<String> {
    let mut common: Vec<String> =
        a.iter().filter(|sa| b.iter().any(|sb| sb.sku == sa.sku)).map(|s| s.sku.clone()).collect();
    common.sort();
    common.dedup();
    common
}

/// SKU 목록에서 특정 조건(가용 재고 0)을 만족하는 것의 비율(%)을 계산한다.
pub fn depleted_ratio_percent(snapshots: &[InventorySnapshot]) -> u32 {
    if snapshots.is_empty() {
        return 0;
    }
    let depleted = snapshots.iter().filter(|s| s.is_depleted()).count();
    (depleted * 100 / snapshots.len()) as u32
}

/// 스냅샷 목록을 SKU 접두(카테고리 코드 등) 기준으로 그룹핑한다(접두
/// 길이는 호출자가 지정, 2글자면 inv-core SKU 카테고리와 맞아떨어진다).
pub fn group_by_sku_prefix(snapshots: &[InventorySnapshot], prefix_len: usize) -> Vec<(String, usize)> {
    let mut prefixes: Vec<String> =
        snapshots.iter().map(|s| s.sku.chars().take(prefix_len).collect::<String>()).collect();
    prefixes.sort();
    prefixes.dedup();
    prefixes
        .into_iter()
        .map(|p| (p.clone(), snapshots.iter().filter(|s| s.sku.starts_with(&p)).count()))
        .collect()
}

/// 스냅샷 목록에서 가용 재고 합계가 가장 큰 접두 그룹을 찾는다.
pub fn largest_available_group(snapshots: &[InventorySnapshot], prefix_len: usize) -> Option<(String, u32)> {
    let mut prefixes: Vec<String> =
        snapshots.iter().map(|s| s.sku.chars().take(prefix_len).collect::<String>()).collect();
    prefixes.sort();
    prefixes.dedup();
    prefixes
        .into_iter()
        .map(|p| {
            let sum: u32 = snapshots.iter().filter(|s| s.sku.starts_with(&p)).map(|s| s.available()).sum();
            (p, sum)
        })
        .max_by_key(|(_, sum)| *sum)
}

/// 스냅샷 목록을 두 그룹(짝수/홀수 인덱스)으로 나눈다(A/B 배치 테스트나
/// 병렬 처리 분산용 — 단순 라운드로빈 분배).
pub fn round_robin_split(snapshots: &[InventorySnapshot]) -> (Vec<InventorySnapshot>, Vec<InventorySnapshot>) {
    let mut a = Vec::new();
    let mut b = Vec::new();
    for (i, s) in snapshots.iter().enumerate() {
        if i % 2 == 0 {
            a.push(s.clone());
        } else {
            b.push(s.clone());
        }
    }
    (a, b)
}
