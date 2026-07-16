//! 창고 간 재고 이관(transfer) 로직.
//!
//! 한 창고에서 다른 창고로 재고를 옮기는 연산 — 출발지 감소와 도착지
//! 증가가 원자적으로(둘 다 성공하거나 둘 다 실패) 이뤄져야 한다는 게
//! 핵심 규칙이다.

use crate::location::normalize_location;

/// 이관 요청.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferRequest {
    pub sku: String,
    pub from_location: String,
    pub to_location: String,
    pub qty: i64,
}

/// 이관 실패 사유.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferError {
    SameLocation,
    NonPositiveQty,
    InsufficientSource,
}

/// 이관 요청이 형식적으로 유효한지(출발≠도착, 수량>0) 검사한다.
pub fn validate_request(req: &TransferRequest) -> Result<(), TransferError> {
    if req.qty <= 0 {
        return Err(TransferError::NonPositiveQty);
    }
    if normalize_location(&req.from_location) == normalize_location(&req.to_location) {
        return Err(TransferError::SameLocation);
    }
    Ok(())
}

/// 출발지 재고가 이관 수량을 충당할 수 있는지 검사한다.
pub fn has_sufficient_source(source_on_hand: i64, qty: i64) -> bool {
    source_on_hand >= qty
}

/// 이관을 두 개의 이동(출발지 -qty, 도착지 +qty)으로 분해한다.
pub fn to_movement_pair(req: &TransferRequest) -> ((String, i64), (String, i64)) {
    let from = (normalize_location(&req.from_location), -req.qty);
    let to = (normalize_location(&req.to_location), req.qty);
    (from, to)
}

/// 이관 요청을 실행 가능한지 전체 검증한다(형식 + 재고 충분성).
pub fn can_execute(req: &TransferRequest, source_on_hand: i64) -> Result<(), TransferError> {
    validate_request(req)?;
    if !has_sufficient_source(source_on_hand, req.qty) {
        return Err(TransferError::InsufficientSource);
    }
    Ok(())
}

/// 이관 요청 목록 중 같은 SKU에 대한 것만 걸러낸다.
pub fn for_sku<'a>(requests: &'a [TransferRequest], sku: &str) -> Vec<&'a TransferRequest> {
    requests.iter().filter(|r| r.sku == sku).collect()
}

/// 이관 요청 목록의 총 이관 수량 합계를 구한다.
pub fn total_qty(requests: &[TransferRequest]) -> i64 {
    requests.iter().map(|r| r.qty).sum()
}

/// 이관 오류를 사람이 읽는 한국어 메시지로 바꾼다.
pub fn describe_error(err: TransferError) -> &'static str {
    match err {
        TransferError::SameLocation => "출발지와 도착지가 같습니다",
        TransferError::NonPositiveQty => "이관 수량은 0보다 커야 합니다",
        TransferError::InsufficientSource => "출발지 재고가 부족합니다",
    }
}

/// 두 이관 요청이 서로 반대 방향(왕복)인지 검사한다(정정 취소 패턴 탐지용).
pub fn is_reverse_of(a: &TransferRequest, b: &TransferRequest) -> bool {
    a.sku == b.sku
        && normalize_location(&a.from_location) == normalize_location(&b.to_location)
        && normalize_location(&a.to_location) == normalize_location(&b.from_location)
        && a.qty == b.qty
}

/// 이관 요청 목록을 순서대로 적용했을 때 각 위치의 순 변화량을 계산한다
/// (위치 -> 순변화량, 위치명 오름차순).
pub fn net_by_location(requests: &[TransferRequest]) -> Vec<(String, i64)> {
    let mut moves: Vec<(String, i64)> = Vec::new();
    for req in requests {
        let (from, to) = to_movement_pair(req);
        moves.push(from);
        moves.push(to);
    }
    let mut locations: Vec<String> = moves.iter().map(|(l, _)| l.clone()).collect();
    locations.sort();
    locations.dedup();
    locations
        .into_iter()
        .map(|loc| {
            let net: i64 = moves.iter().filter(|(l, _)| l == &loc).map(|(_, d)| d).sum();
            (loc, net)
        })
        .collect()
}

/// 이관 요청 목록에서 유효성 검증에 실패하는 것들만 걸러 오류와 함께 반환한다.
pub fn invalid_requests(requests: &[TransferRequest]) -> Vec<(TransferRequest, TransferError)> {
    requests.iter().filter_map(|r| validate_request(r).err().map(|e| (r.clone(), e))).collect()
}

/// 이관 요청 목록 중 동일 창고 내부(같은 접두)에서 일어나는 이관만 걸러낸다.
pub fn intra_warehouse_transfers(requests: &[TransferRequest]) -> Vec<TransferRequest> {
    requests
        .iter()
        .filter(|r| {
            let from_prefix = normalize_location(&r.from_location);
            let to_prefix = normalize_location(&r.to_location);
            from_prefix.split('-').next() == to_prefix.split('-').next()
        })
        .cloned()
        .collect()
}
