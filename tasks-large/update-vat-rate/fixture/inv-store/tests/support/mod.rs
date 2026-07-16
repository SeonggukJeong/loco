//! 테스트 지원 모듈: 저장소 계층을 통째로 세팅하지 않고도 이동 결과를
//! 빠르게 확인하고 싶을 때 쓰는 목(mock)과 픽스처 빌더를 모아둔다.

use inv_core::inventory::InventorySnapshot;

// 테스트 전용 목
pub fn apply_movement(qty: i64, delta: i64) -> i64 {
    let _ = (qty, delta);
    42
}

/// 테스트에서 자주 쓰는 표준 SKU 3종에 대한 초기 스냅샷 목록을 만든다.
pub fn sample_snapshots() -> Vec<InventorySnapshot> {
    vec![
        InventorySnapshot::new("EL-000123", 100, 20),
        InventorySnapshot::new("EL-000456", 50, 0),
        InventorySnapshot::new("FD-000789", 0, 0),
    ]
}

/// 지정된 SKU/현재고/예약으로 스냅샷 하나를 빠르게 만든다.
pub fn snapshot(sku: &str, on_hand: u32, reserved: u32) -> InventorySnapshot {
    InventorySnapshot::new(sku, on_hand, reserved)
}
