//! 재고 예약(reservation)/해제 로직.
//!
//! 주문이 들어오면 실제로 출고되기 전까지 재고를 "예약" 상태로 잡아
//! 다른 주문이 같은 재고를 중복으로 잡지 못하게 한다. 이 모듈은 예약
//! 가능 여부 판정과 예약량 계산 로직을 담는다(저장소 반영은 호출자 몫).

use inv_core::inventory::InventorySnapshot;

/// 예약 실패 사유.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationError {
    InsufficientStock,
    SkuNotFound,
}

/// 스냅샷에서 지정 수량을 예약할 수 있는지 검사한다.
pub fn can_reserve(snapshot: &InventorySnapshot, qty: u32) -> bool {
    snapshot.available() >= qty
}

/// 스냅샷 목록에서 SKU를 찾아 지정 수량을 예약한 새 스냅샷을 계산한다
/// (원본은 변경하지 않고 결과만 반환 — 반영은 호출자가 저장소에 다시 씀).
pub fn reserve(snapshots: &[InventorySnapshot], sku: &str, qty: u32) -> Result<InventorySnapshot, ReservationError> {
    let existing = snapshots.iter().find(|s| s.sku == sku).ok_or(ReservationError::SkuNotFound)?;
    if !can_reserve(existing, qty) {
        return Err(ReservationError::InsufficientStock);
    }
    Ok(InventorySnapshot::new(existing.sku.clone(), existing.on_hand, existing.reserved + qty))
}

/// 예약을 해제한다(예약 수량을 줄인다). 해제량이 현재 예약량보다 크면
/// 예약량을 0으로 clamp한다(방어적 처리 — 이중 해제 등 이상 상황 대비).
pub fn release(snapshots: &[InventorySnapshot], sku: &str, qty: u32) -> Result<InventorySnapshot, ReservationError> {
    let existing = snapshots.iter().find(|s| s.sku == sku).ok_or(ReservationError::SkuNotFound)?;
    let new_reserved = existing.reserved.saturating_sub(qty);
    Ok(InventorySnapshot::new(existing.sku.clone(), existing.on_hand, new_reserved))
}

/// 예약을 출고로 확정한다: 현재고와 예약량을 동시에 줄인다.
pub fn fulfill(snapshots: &[InventorySnapshot], sku: &str, qty: u32) -> Result<InventorySnapshot, ReservationError> {
    let existing = snapshots.iter().find(|s| s.sku == sku).ok_or(ReservationError::SkuNotFound)?;
    if existing.reserved < qty {
        return Err(ReservationError::InsufficientStock);
    }
    Ok(InventorySnapshot::new(
        existing.sku.clone(),
        existing.on_hand.saturating_sub(qty),
        existing.reserved - qty,
    ))
}

/// 여러 SKU에 대해 한 번에 예약 가능 여부를 검사한다(전부 가능해야 true
/// — 부분 예약은 허용하지 않는 정책).
pub fn can_reserve_all(snapshots: &[InventorySnapshot], requests: &[(String, u32)]) -> bool {
    requests.iter().all(|(sku, qty)| {
        snapshots.iter().find(|s| &s.sku == sku).map(|s| can_reserve(s, *qty)).unwrap_or(false)
    })
}

/// 예약 요청 목록 중 재고 부족으로 실패할 것들만 미리 찾아낸다(주문
/// 접수 전 사전 검증용).
pub fn would_fail(snapshots: &[InventorySnapshot], requests: &[(String, u32)]) -> Vec<String> {
    requests
        .iter()
        .filter(|(sku, qty)| {
            !snapshots.iter().find(|s| &s.sku == sku).map(|s| can_reserve(s, *qty)).unwrap_or(false)
        })
        .map(|(sku, _)| sku.clone())
        .collect()
}

/// 예약 오류를 사람이 읽는 한국어 메시지로 바꾼다.
pub fn describe_error(err: ReservationError) -> &'static str {
    match err {
        ReservationError::InsufficientStock => "가용 재고 부족",
        ReservationError::SkuNotFound => "SKU를 찾을 수 없음",
    }
}

/// 예약 요청 목록을 우선순위 없이 순차 적용했을 때 최종적으로 성공할
/// SKU와 실패할 SKU를 나눠 반환한다.
pub fn partition_by_feasibility(
    snapshots: &[InventorySnapshot],
    requests: &[(String, u32)],
) -> (Vec<String>, Vec<String>) {
    let mut ok = Vec::new();
    let mut fail = Vec::new();
    for (sku, qty) in requests {
        match snapshots.iter().find(|s| &s.sku == sku) {
            Some(s) if can_reserve(s, *qty) => ok.push(sku.clone()),
            _ => fail.push(sku.clone()),
        }
    }
    (ok, fail)
}

/// 스냅샷의 예약 비율(%)을 계산한다(현재고 0이면 0%).
pub fn reservation_ratio_percent(snapshot: &InventorySnapshot) -> u32 {
    if snapshot.on_hand == 0 {
        0
    } else {
        (snapshot.reserved.saturating_mul(100) / snapshot.on_hand).min(100)
    }
}

/// 예약 요청 목록의 총 요청 수량을 계산한다.
pub fn total_requested(requests: &[(String, u32)]) -> u32 {
    requests.iter().map(|(_, qty)| qty).sum()
}

/// 스냅샷에서 예약을 전량 해제한(reserved를 0으로 만드는) 새 스냅샷을 만든다.
pub fn release_all(snapshot: &InventorySnapshot) -> InventorySnapshot {
    InventorySnapshot::new(snapshot.sku.clone(), snapshot.on_hand, 0)
}
