//! 재고 이동(입고/출고/조정) 적용 로직.
//!
//! 저장소에 반영되기 전의 "수량 계산" 단계만 담당한다 — 실제로 저장소를
//! 갱신하는 코드는 `memory`/`file` 모듈에 있다. 이렇게 분리해 두면 계산
//! 로직만 독립적으로 테스트하기 쉽다.

/// 현재 수량(`qty`)에 이동량(`delta`, 음수면 출고)을 적용한 새 수량을
/// 계산한다. 음수로 내려가지 않도록 0에서 clamp한다(재고가 실제로
/// 마이너스가 될 수는 없다는 도메인 규칙).
pub fn apply_movement(qty: i64, delta: i64) -> i64 {
    (qty + delta).max(0)
}

/// 이동 유형(입고/출고/이관/조정).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementKind {
    Inbound,
    Outbound,
    Transfer,
    Adjustment,
}

/// 이동 유형 코드 문자열을 파싱한다.
pub fn parse_movement_kind(code: &str) -> Option<MovementKind> {
    match code {
        "IN" => Some(MovementKind::Inbound),
        "OUT" => Some(MovementKind::Outbound),
        "TRANSFER" => Some(MovementKind::Transfer),
        "ADJUST" => Some(MovementKind::Adjustment),
        _ => None,
    }
}

/// 이동 유형에 맞는 부호로 delta를 정규화한다(입고는 항상 양수, 출고는
/// 항상 음수가 되도록 강제 — 호출자가 부호를 실수로 반대로 넣는 것을 방지).
pub fn normalize_delta(kind: MovementKind, magnitude: i64) -> i64 {
    let abs = magnitude.abs();
    match kind {
        MovementKind::Inbound => abs,
        MovementKind::Outbound => -abs,
        MovementKind::Transfer => -abs, // 이관은 출발 창고 기준으로는 출고와 동일
        MovementKind::Adjustment => magnitude, // 조정은 부호를 그대로 존중
    }
}

/// 하나의 이동 레코드.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovementRecord {
    pub sku: String,
    pub kind: MovementKind,
    pub delta: i64,
    pub reason: String,
}

impl MovementRecord {
    pub fn new(sku: impl Into<String>, kind: MovementKind, delta: i64, reason: impl Into<String>) -> Self {
        MovementRecord { sku: sku.into(), kind, delta, reason: reason.into() }
    }
}

/// 이동 레코드 목록을 순서대로 적용해 최종 수량을 계산한다.
pub fn apply_all(initial_qty: i64, records: &[MovementRecord]) -> i64 {
    records.iter().fold(initial_qty, |qty, r| apply_movement(qty, r.delta))
}

/// 이동이 수동 승인이 필요할 만큼 큰지(절대값 기준) 판정한다.
pub fn requires_manual_approval(delta: i64, threshold: i64) -> bool {
    delta.abs() >= threshold
}

/// 이동 레코드 목록에서 특정 종류만 걸러낸다.
pub fn filter_by_kind(records: &[MovementRecord], kind: MovementKind) -> Vec<MovementRecord> {
    records.iter().filter(|r| r.kind == kind).cloned().collect()
}

/// 이동 레코드 목록의 순 변화량(입고-출고 합)을 계산한다.
pub fn net_delta(records: &[MovementRecord]) -> i64 {
    records.iter().map(|r| r.delta).sum()
}

/// 이동 레코드 목록 중 특정 SKU에 대한 것만 걸러낸다.
pub fn for_sku<'a>(records: &'a [MovementRecord], sku: &str) -> Vec<&'a MovementRecord> {
    records.iter().filter(|r| r.sku == sku).collect()
}

/// 이동이 재고를 완전히 소진시키는지(적용 후 결과가 0인지) 미리 검사한다.
pub fn would_deplete(qty: i64, delta: i64) -> bool {
    apply_movement(qty, delta) == 0
}

/// 이동이 실제로 요청한 만큼 전부 반영됐는지(clamp로 잘리지 않았는지) 검사한다.
pub fn was_clamped(qty: i64, delta: i64) -> bool {
    qty + delta < 0
}

/// 이동 사유 코드가 알려진 값인지 검사한다.
pub fn is_known_reason(reason: &str) -> bool {
    matches!(reason, "PURCHASE" | "SALE" | "RETURN" | "DAMAGE" | "COUNT_CORRECTION" | "TRANSFER")
}

/// 여러 이동을 하나로 합쳐(같은 SKU라면) 순 변화량 레코드로 압축한다.
pub fn compress_by_sku(records: &[MovementRecord]) -> Vec<(String, i64)> {
    let mut skus: Vec<String> = records.iter().map(|r| r.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus.into_iter()
        .map(|sku| {
            let net: i64 = records.iter().filter(|r| r.sku == sku).map(|r| r.delta).sum();
            (sku, net)
        })
        .collect()
}

/// 이동 유형을 사람이 읽는 한국어 이름으로 바꾼다.
pub fn kind_label(kind: MovementKind) -> &'static str {
    match kind {
        MovementKind::Inbound => "입고",
        MovementKind::Outbound => "출고",
        MovementKind::Transfer => "이관",
        MovementKind::Adjustment => "조정",
    }
}

/// 이동 레코드 목록을 시간순이 아니라 유형별로 그룹핑한다(입고/출고/이관/
/// 조정 순으로 고정된 순서).
pub fn group_by_kind(records: &[MovementRecord]) -> Vec<(MovementKind, Vec<MovementRecord>)> {
    let kinds = [MovementKind::Inbound, MovementKind::Outbound, MovementKind::Transfer, MovementKind::Adjustment];
    kinds.into_iter().map(|k| (k, filter_by_kind(records, k))).collect()
}

/// 이동 레코드 목록의 사유(reason)별 건수를 센다(사유명 -> 건수, 이름
/// 오름차순).
pub fn count_by_reason(records: &[MovementRecord]) -> Vec<(String, usize)> {
    let mut reasons: Vec<String> = records.iter().map(|r| r.reason.clone()).collect();
    reasons.sort();
    reasons.dedup();
    reasons.into_iter().map(|r| (r.clone(), records.iter().filter(|rec| rec.reason == r).count())).collect()
}

/// 이동 레코드가 재고를 늘리는 방향인지(delta > 0) 검사한다.
pub fn is_increase(record: &MovementRecord) -> bool {
    record.delta > 0
}

/// 초기 수량에 이동 레코드를 하나씩 적용하며 중간 수량 이력을 모두 기록한다
/// (재고 추이 그래프용 — 각 단계의 결과값을 순서대로 담은 목록).
pub fn running_history(initial_qty: i64, records: &[MovementRecord]) -> Vec<i64> {
    let mut qty = initial_qty;
    let mut history = Vec::with_capacity(records.len());
    for r in records {
        qty = apply_movement(qty, r.delta);
        history.push(qty);
    }
    history
}

/// 이동 레코드 목록 중 재고를 완전히 소진시킨(적용 후 0이 된) 첫 레코드의
/// 인덱스를 찾는다.
pub fn first_depleting_index(initial_qty: i64, records: &[MovementRecord]) -> Option<usize> {
    running_history(initial_qty, records).iter().position(|&q| q == 0)
}

/// 이동 레코드 목록을 시간 역순(가장 최근이 먼저)이라고 가정하고 뒤집는다
/// (로그가 최신순으로 쌓이는 저장소에서 재생 순서로 되돌릴 때 사용).
pub fn reversed(records: &[MovementRecord]) -> Vec<MovementRecord> {
    let mut out = records.to_vec();
    out.reverse();
    out
}

/// 이동 레코드 목록에서 절대값이 가장 큰 이동 하나를 찾는다.
pub fn largest_movement(records: &[MovementRecord]) -> Option<&MovementRecord> {
    records.iter().max_by_key(|r| r.delta.abs())
}

/// 이동 레코드 목록을 SKU, 시간 순서(입력 순서 유지) 기준으로 그룹핑한다
/// (SKU -> 레코드 목록, SKU 오름차순).
pub fn group_by_sku(records: &[MovementRecord]) -> Vec<(String, Vec<MovementRecord>)> {
    let mut skus: Vec<String> = records.iter().map(|r| r.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus.into_iter().map(|sku| (sku.clone(), for_sku(records, &sku).into_iter().cloned().collect())).collect()
}

/// 이동 레코드 목록이 특정 SKU에 대해 순증가(모든 delta가 0 이상)만
/// 있었는지 검사한다.
pub fn is_monotonic_increase_for_sku(records: &[MovementRecord], sku: &str) -> bool {
    for_sku(records, sku).iter().all(|r| r.delta >= 0)
}

/// 이동 유형별 순 변화량 합계를 계산한다(유형 -> 합계, 고정된 4종 순서).
pub fn net_delta_by_kind(records: &[MovementRecord]) -> Vec<(MovementKind, i64)> {
    let kinds = [MovementKind::Inbound, MovementKind::Outbound, MovementKind::Transfer, MovementKind::Adjustment];
    kinds.into_iter().map(|k| (k, filter_by_kind(records, k).iter().map(|r| r.delta).sum())).collect()
}
