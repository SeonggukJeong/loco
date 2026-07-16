//! SKU -> 위치 목록 보조 인덱스.
//!
//! `memory::MemoryStore`는 SKU 하나당 스냅샷 하나만 보관하지만(창고 구분
//! 없이 전체 합계), 실제로는 "이 SKU가 어느 위치들에 흩어져 있는지"를
//! 자주 물어봐야 한다. 이 모듈은 그 보조 인덱스를 별도 자료구조로 둔다
//! (메인 스냅샷과 동기화는 호출자의 책임 — 인덱스 자체는 순수 자료구조다).

/// SKU별 보유 위치 목록을 보관하는 인덱스.
#[derive(Debug, Clone, Default)]
pub struct LocationIndex {
    entries: Vec<(String, Vec<String>)>,
}

impl LocationIndex {
    pub fn new() -> Self {
        LocationIndex::default()
    }

    /// SKU에 위치를 추가한다(이미 있으면 중복 추가하지 않음).
    pub fn add(&mut self, sku: &str, location: &str) {
        if let Some((_, locs)) = self.entries.iter_mut().find(|(s, _)| s == sku) {
            if !locs.iter().any(|l| l == location) {
                locs.push(location.to_string());
            }
        } else {
            self.entries.push((sku.to_string(), vec![location.to_string()]));
        }
    }

    /// SKU에서 위치를 제거한다.
    pub fn remove(&mut self, sku: &str, location: &str) {
        if let Some((_, locs)) = self.entries.iter_mut().find(|(s, _)| s == sku) {
            locs.retain(|l| l != location);
        }
    }

    /// SKU가 보유한 위치 목록을 조회한다.
    pub fn locations_for(&self, sku: &str) -> Vec<String> {
        self.entries.iter().find(|(s, _)| s == sku).map(|(_, locs)| locs.clone()).unwrap_or_default()
    }

    /// SKU가 여러 위치에 분산되어 있는지(2곳 이상) 검사한다.
    pub fn is_scattered(&self, sku: &str) -> bool {
        self.locations_for(sku).len() > 1
    }

    /// 인덱스에 등록된 전체 SKU 개수.
    pub fn sku_count(&self) -> usize {
        self.entries.len()
    }

    /// 특정 위치를 보유한 SKU 목록을 역조회한다(느린 선형 탐색 — 인덱스
    /// 규모가 크지 않다는 전제).
    pub fn skus_at_location(&self, location: &str) -> Vec<String> {
        self.entries.iter().filter(|(_, locs)| locs.iter().any(|l| l == location)).map(|(s, _)| s.clone()).collect()
    }

    /// 위치가 하나도 없는(비어버린) SKU 항목을 정리한다.
    pub fn prune_empty(&mut self) {
        self.entries.retain(|(_, locs)| !locs.is_empty());
    }

    /// 등록된 모든 SKU 목록(정렬됨).
    pub fn all_skus(&self) -> Vec<String> {
        let mut skus: Vec<String> = self.entries.iter().map(|(s, _)| s.clone()).collect();
        skus.sort();
        skus
    }
}

/// 여러 (SKU, 위치) 쌍으로부터 인덱스를 한 번에 만든다.
pub fn build_index(pairs: &[(String, String)]) -> LocationIndex {
    let mut index = LocationIndex::new();
    for (sku, loc) in pairs {
        index.add(sku, loc);
    }
    index
}

/// 인덱스에서 여러 위치에 분산된 SKU 개수를 센다(재배치 후보 파악용).
pub fn scattered_count(index: &LocationIndex) -> usize {
    index.all_skus().iter().filter(|sku| index.is_scattered(sku)).count()
}

/// 인덱스에서 특정 SKU를 완전히 제거한다(모든 위치 매핑 삭제).
pub fn remove_sku(index: &mut LocationIndex, sku: &str) {
    for loc in index.locations_for(sku) {
        index.remove(sku, &loc);
    }
    index.prune_empty();
}

/// 두 인덱스를 병합한다(같은 SKU는 위치 목록을 합친다).
pub fn merge(a: &LocationIndex, b: &LocationIndex) -> LocationIndex {
    let mut merged = a.clone();
    for sku in b.all_skus() {
        for loc in b.locations_for(&sku) {
            merged.add(&sku, &loc);
        }
    }
    merged
}

/// 인덱스에 등록된 전체 (SKU, 위치) 쌍 개수를 센다.
pub fn total_mappings(index: &LocationIndex) -> usize {
    index.all_skus().iter().map(|sku| index.locations_for(sku).len()).sum()
}

/// 등록된 SKU가 없는(빈) 인덱스인지 검사한다.
pub fn is_empty(index: &LocationIndex) -> bool {
    index.sku_count() == 0
}
