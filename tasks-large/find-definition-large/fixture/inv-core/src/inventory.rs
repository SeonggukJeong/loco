//! 재고 스냅샷 타입.
//!
//! `restock_threshold`/`WarehouseGrade`는 재고 도메인에서 자주 함께
//! 쓰이므로 편의상 여기서 재수출(re-export)해 둔다.
pub use crate::rules::{restock_threshold, WarehouseGrade};

/// 특정 SKU의 특정 시점 재고 스냅샷.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventorySnapshot {
    pub sku: String,
    pub on_hand: u32,
    pub reserved: u32,
}

impl InventorySnapshot {
    pub fn new(sku: impl Into<String>, on_hand: u32, reserved: u32) -> Self {
        InventorySnapshot { sku: sku.into(), on_hand, reserved }
    }

    /// 예약분을 제외한 가용 재고.
    pub fn available(&self) -> u32 {
        self.on_hand.saturating_sub(self.reserved)
    }

    /// 가용 재고가 0인지(즉시 판매 불가) 여부.
    pub fn is_depleted(&self) -> bool {
        self.available() == 0
    }
}

/// 스냅샷 목록에서 특정 SKU를 찾는다.
pub fn find_snapshot<'a>(snapshots: &'a [InventorySnapshot], sku: &str) -> Option<&'a InventorySnapshot> {
    snapshots.iter().find(|s| s.sku == sku)
}

/// 전체 스냅샷의 가용 재고 합계.
pub fn total_available(snapshots: &[InventorySnapshot]) -> u32 {
    snapshots.iter().map(|s| s.available()).sum()
}

/// 가용 재고가 0인 SKU만 걸러낸다.
pub fn depleted_skus(snapshots: &[InventorySnapshot]) -> Vec<String> {
    snapshots.iter().filter(|s| s.is_depleted()).map(|s| s.sku.clone()).collect()
}

/// 스냅샷 목록을 가용 재고 내림차순으로 정렬한다.
pub fn sort_by_available_desc(snapshots: &mut Vec<InventorySnapshot>) {
    snapshots.sort_by(|a, b| b.available().cmp(&a.available()));
}

/// 두 스냅샷 목록을 SKU 기준으로 병합한다(같은 SKU는 수량을 합산).
pub fn merge_snapshots(a: &[InventorySnapshot], b: &[InventorySnapshot]) -> Vec<InventorySnapshot> {
    let mut merged: Vec<InventorySnapshot> = a.to_vec();
    for item in b {
        if let Some(existing) = merged.iter_mut().find(|s| s.sku == item.sku) {
            existing.on_hand += item.on_hand;
            existing.reserved += item.reserved;
        } else {
            merged.push(item.clone());
        }
    }
    merged
}

/// 스냅샷 목록의 예약 재고 합계를 계산한다.
pub fn total_reserved(snapshots: &[InventorySnapshot]) -> u32 {
    snapshots.iter().map(|s| s.reserved).sum()
}

/// 예약 재고가 현재고를 초과한(데이터 이상) 스냅샷만 걸러낸다.
pub fn overreserved_snapshots(snapshots: &[InventorySnapshot]) -> Vec<InventorySnapshot> {
    snapshots.iter().filter(|s| s.reserved > s.on_hand).cloned().collect()
}

/// 특정 SKU의 재고를 조정한다(delta는 음수 가능). 존재하지 않으면 false.
pub fn adjust_on_hand(snapshots: &mut [InventorySnapshot], sku: &str, delta: i64) -> bool {
    if let Some(s) = snapshots.iter_mut().find(|s| s.sku == sku) {
        let new_value = (s.on_hand as i64 + delta).max(0);
        s.on_hand = new_value as u32;
        true
    } else {
        false
    }
}
