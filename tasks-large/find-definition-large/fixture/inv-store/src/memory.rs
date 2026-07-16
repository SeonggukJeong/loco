//! 인메모리 재고 저장소.
//!
//! 재고 스냅샷(`inv_core::inventory::InventorySnapshot`)을 프로세스 메모리에
//! 보관하는 가장 단순한 저장소 구현이다. 테스트/개발 환경, 혹은 영속화가
//! 필요 없는 단발성 배치 작업에 쓰인다. 실 운영에서는 `file` 모듈의 포맷을
//! 거쳐 디스크에 반영한다(실제 파일 I/O는 inv-cli 계층의 몫).

use inv_core::inventory::{self, InventorySnapshot};

use crate::movement::apply_movement;

/// SKU 단위 재고 스냅샷을 보관하는 인메모리 저장소.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    snapshots: Vec<InventorySnapshot>,
}

impl MemoryStore {
    pub fn new() -> Self {
        MemoryStore { snapshots: Vec::new() }
    }

    /// 기존 스냅샷 목록으로부터 저장소를 만든다.
    pub fn from_snapshots(snapshots: Vec<InventorySnapshot>) -> Self {
        MemoryStore { snapshots }
    }

    /// 저장소를 스냅샷 목록으로 되돌린다(직렬화/이관용).
    pub fn into_snapshots(self) -> Vec<InventorySnapshot> {
        self.snapshots
    }

    /// SKU 스냅샷을 추가하거나(없으면) 갱신한다(있으면).
    pub fn upsert(&mut self, snapshot: InventorySnapshot) {
        if let Some(existing) = self.snapshots.iter_mut().find(|s| s.sku == snapshot.sku) {
            *existing = snapshot;
        } else {
            self.snapshots.push(snapshot);
        }
    }

    /// SKU로 스냅샷을 조회한다.
    pub fn get(&self, sku: &str) -> Option<&InventorySnapshot> {
        inventory::find_snapshot(&self.snapshots, sku)
    }

    /// SKU 스냅샷을 제거한다. 존재했으면 true.
    pub fn remove(&mut self, sku: &str) -> bool {
        let before = self.snapshots.len();
        self.snapshots.retain(|s| s.sku != sku);
        self.snapshots.len() != before
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// 저장소가 보유한 전체 스냅샷을 읽기 전용으로 반환한다.
    pub fn all(&self) -> &[InventorySnapshot] {
        &self.snapshots
    }

    /// 특정 SKU에 재고 이동량(delta)을 적용한다. SKU가 없으면 false.
    /// 실제 수량 계산은 `movement::apply_movement`를 그대로 쓴다.
    pub fn apply_delta(&mut self, sku: &str, delta: i64) -> bool {
        if let Some(s) = self.snapshots.iter_mut().find(|s| s.sku == sku) {
            s.on_hand = apply_movement(s.on_hand as i64, delta) as u32;
            true
        } else {
            false
        }
    }

    /// 여러 이동을 순서대로 적용하고, 성공한 이동 건수를 반환한다.
    pub fn apply_deltas(&mut self, deltas: &[(String, i64)]) -> usize {
        deltas.iter().filter(|(sku, delta)| self.apply_delta(sku, *delta)).count()
    }

    /// SKU 존재 여부.
    pub fn contains(&self, sku: &str) -> bool {
        self.get(sku).is_some()
    }

    /// 전체 가용 재고(현재고 - 예약).
    pub fn total_available(&self) -> u32 {
        inventory::total_available(&self.snapshots)
    }

    /// 가용 재고가 0인 SKU 목록.
    pub fn depleted_skus(&self) -> Vec<String> {
        inventory::depleted_skus(&self.snapshots)
    }

    /// 가용 재고 내림차순으로 정렬한다.
    pub fn sort_by_available_desc(&mut self) {
        inventory::sort_by_available_desc(&mut self.snapshots)
    }

    /// 다른 저장소의 스냅샷을 이 저장소에 병합한다(같은 SKU는 수량 합산).
    pub fn merge_from(&mut self, other: &MemoryStore) {
        self.snapshots = inventory::merge_snapshots(&self.snapshots, &other.snapshots);
    }

    /// 모든 스냅샷을 지운다.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// 가용 재고가 `min` 이상인 스냅샷만 남긴다(저회전 SKU 정리 등에 사용).
    pub fn retain_min_available(&mut self, min: u32) {
        self.snapshots.retain(|s| s.available() >= min);
    }

    /// 예약 재고가 현재고를 초과하는(데이터 이상) 스냅샷 목록.
    pub fn overreserved(&self) -> Vec<InventorySnapshot> {
        inventory::overreserved_snapshots(&self.snapshots)
    }

    /// 예약 재고 합계.
    pub fn total_reserved(&self) -> u32 {
        inventory::total_reserved(&self.snapshots)
    }
}

/// 빈 저장소에 SKU 목록을 기본 수량 0으로 초기화해 채운다(신규 SKU 등록
/// 배치의 첫 단계로 흔히 쓰인다).
pub fn seed_zero(skus: &[String]) -> MemoryStore {
    let mut store = MemoryStore::new();
    for sku in skus {
        store.upsert(InventorySnapshot::new(sku.clone(), 0, 0));
    }
    store
}

/// 두 저장소의 SKU 집합이 정확히 같은지 비교한다(마이그레이션 검증용).
pub fn same_sku_set(a: &MemoryStore, b: &MemoryStore) -> bool {
    let mut a_skus: Vec<&str> = a.all().iter().map(|s| s.sku.as_str()).collect();
    let mut b_skus: Vec<&str> = b.all().iter().map(|s| s.sku.as_str()).collect();
    a_skus.sort();
    b_skus.sort();
    a_skus == b_skus
}

impl MemoryStore {
    /// 저장소의 모든 스냅샷 중 지정 조건을 만족하는 것만 걸러 새 목록으로
    /// 반환한다(제네릭 술어 버전 — 필터 조합이 매번 다른 호출부를 위해).
    pub fn filter<F>(&self, predicate: F) -> Vec<InventorySnapshot>
    where
        F: Fn(&InventorySnapshot) -> bool,
    {
        self.snapshots.iter().filter(|s| predicate(s)).cloned().collect()
    }

    /// SKU 목록으로 여러 스냅샷을 한 번에 조회한다(존재하지 않는 SKU는
    /// 결과에서 제외).
    pub fn get_many(&self, skus: &[String]) -> Vec<InventorySnapshot> {
        skus.iter().filter_map(|sku| self.get(sku).cloned()).collect()
    }

    /// 저장소의 전체 예약 재고 합계 대비 전체 현재고 합계 비율(%)을 계산한다.
    pub fn overall_reservation_ratio_percent(&self) -> u32 {
        let total_on_hand: u32 = self.snapshots.iter().map(|s| s.on_hand).sum();
        if total_on_hand == 0 {
            0
        } else {
            (self.total_reserved().saturating_mul(100) / total_on_hand).min(100)
        }
    }

    /// 저장소를 SKU 오름차순으로 정렬한다(출력/보고서용 — 가용 재고 정렬과
    /// 별개의 안정적인 정렬 기준이 필요할 때 사용).
    pub fn sort_by_sku(&mut self) {
        self.snapshots.sort_by(|a, b| a.sku.cmp(&b.sku));
    }

    /// 저장소에서 특정 SKU 하나만 복제해 새 저장소를 만든다(부분 백업용).
    pub fn extract(&self, sku: &str) -> MemoryStore {
        let mut store = MemoryStore::new();
        if let Some(s) = self.get(sku) {
            store.upsert(s.clone());
        }
        store
    }
}

/// 저장소 두 개를 비교해 SKU 집합의 차집합(a에만 있는 SKU)을 구한다.
pub fn skus_only_in(a: &MemoryStore, b: &MemoryStore) -> Vec<String> {
    a.all().iter().map(|s| s.sku.clone()).filter(|sku| !b.contains(sku)).collect()
}

/// 여러 저장소를 순서대로 병합해 하나의 저장소로 합친다.
pub fn merge_all(stores: &[MemoryStore]) -> MemoryStore {
    let mut result = MemoryStore::new();
    for store in stores {
        result.merge_from(store);
    }
    result
}

/// 저장소에서 재고 가치를 계산한다(단가 목록 기반, 미상 SKU는 0원).
pub fn total_value_krw(store: &MemoryStore, unit_prices: &[(String, i64)]) -> i64 {
    store
        .all()
        .iter()
        .map(|s| {
            let price = unit_prices.iter().find(|(sku, _)| sku == &s.sku).map(|(_, p)| *p).unwrap_or(0);
            s.on_hand as i64 * price
        })
        .sum()
}

/// 저장소가 지정 SKU 목록을 전부 보유하고 있는지(하나라도 빠지면 false)
/// 검사한다.
pub fn has_all_skus(store: &MemoryStore, skus: &[String]) -> bool {
    skus.iter().all(|sku| store.contains(sku))
}

/// 저장소의 스냅샷 중 가용 재고가 가장 큰 것 하나를 찾는다.
pub fn most_available(store: &MemoryStore) -> Option<InventorySnapshot> {
    store.all().iter().max_by_key(|s| s.available()).cloned()
}

/// 저장소를 두 그룹(조건을 만족/불만족)으로 나눈다.
pub fn partition<F>(store: &MemoryStore, predicate: F) -> (MemoryStore, MemoryStore)
where
    F: Fn(&InventorySnapshot) -> bool,
{
    let mut matched = MemoryStore::new();
    let mut unmatched = MemoryStore::new();
    for s in store.all() {
        if predicate(s) {
            matched.upsert(s.clone());
        } else {
            unmatched.upsert(s.clone());
        }
    }
    (matched, unmatched)
}
