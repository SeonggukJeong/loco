//! `movement` 서브커맨드: 특정 SKU에 재고 이동(delta)을 적용해 결과를
//! 출력한다.
//!
//! 이 서브커맨드도 샘플 저장소를 기준으로 동작한다(영속화된 저장소를
//! 읽고 쓰는 것은 후속 작업). 이동 자체의 수량 계산은 inv-store에
//! 위임한다.

use inv_core::inventory::InventorySnapshot;
use inv_store::memory::MemoryStore;
use inv_store::movement::{apply_movement, requires_manual_approval};

/// 이동 커맨드에서 쓰는 샘플 저장소를 만든다(단일 SKU, 초기 재고 50).
fn sample_store() -> MemoryStore {
    let mut store = MemoryStore::new();
    store.upsert(InventorySnapshot::new("EL-000123", 50, 0));
    store
}

/// SKU에 이동량(delta)을 적용하고 결과를 출력 텍스트로 만든다. 저장소에
/// 없는 SKU면 새 스냅샷을 0에서 시작해 만든다.
pub fn execute(sku: &str, delta: i64) -> String {
    let mut store = sample_store();
    if !store.contains(sku) {
        store.upsert(InventorySnapshot::new(sku, 0, 0));
    }
    let before = store.get(sku).map(|s| s.on_hand).unwrap_or(0);
    store.apply_delta(sku, delta);
    let after = store.get(sku).map(|s| s.on_hand).unwrap_or(0);
    let approval_note = approval_note(delta);
    format!("{sku}: {before} -> {after} (delta {delta}){approval_note}")
}

/// 이동량이 수동 승인 임계값(1000) 이상이면 안내 문구를 덧붙인다.
fn approval_note(delta: i64) -> String {
    if requires_manual_approval(delta, 1000) {
        " [수동 승인 필요]".to_string()
    } else {
        String::new()
    }
}

/// 이동 결과가 재고 소진(0으로 클램프)을 유발했는지 텍스트에서 판정한다
/// (라우팅 테스트 보조용 — 순수 문자열 검사).
pub fn output_shows_depletion(output: &str) -> bool {
    output.contains("-> 0 ")
}

/// 여러 (SKU, delta) 이동을 순서대로 적용한 뒤 각 결과를 한 줄씩 모은다.
pub fn execute_batch(movements: &[(String, i64)]) -> String {
    movements.iter().map(|(sku, delta)| execute(sku, *delta)).collect::<Vec<_>>().join("\n")
}

/// 이동량 자체를 저장소 없이 미리 계산해 결과 수량만 보고 싶을 때 쓰는
/// 순수 계산 경로(승인 안내 없이 숫자만).
pub fn preview_result(current_qty: i64, delta: i64) -> i64 {
    apply_movement(current_qty, delta)
}

/// 이동 실행 결과 문자열에서 "이전 -> 이후" 수량 쌍을 파싱해 되돌린다
/// (라우팅 테스트/디버그 보조용).
pub fn parse_before_after(output: &str) -> Option<(u32, u32)> {
    let arrow_pos = output.find(" -> ")?;
    let before_start = output[..arrow_pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
    let before = output[before_start..arrow_pos].parse::<u32>().ok()?;
    let after_start = arrow_pos + 4;
    let rest = &output[after_start..];
    let after_end = rest.find(' ').unwrap_or(rest.len());
    let after = rest[..after_end].parse::<u32>().ok()?;
    Some((before, after))
}

/// 이동량이 양수(입고성)인지 검사한다(출력 텍스트에 의존하지 않는 순수
/// 판정 헬퍼).
pub fn is_inbound(delta: i64) -> bool {
    delta > 0
}

/// 이동량이 음수(출고성)인지 검사한다.
pub fn is_outbound(delta: i64) -> bool {
    delta < 0
}

/// 여러 이동량의 순 변화량(합계)을 계산한다(배치 실행 전 사전 확인용).
pub fn net_delta(deltas: &[i64]) -> i64 {
    deltas.iter().sum()
}

/// 이동량 목록 중 수동 승인이 필요한(임계값 이상인) 것의 개수를 센다.
pub fn approval_required_count(deltas: &[i64], threshold: i64) -> usize {
    deltas.iter().filter(|d| requires_manual_approval(**d, threshold)).count()
}

/// 이동 실행 결과 텍스트에서 SKU 이름만 뽑아낸다(콜론 앞부분).
pub fn extract_sku(output: &str) -> Option<&str> {
    output.split(':').next()
}

/// 배치 이동 실행 결과(여러 줄)의 줄 수를 센다(적용된 이동 건수 확인용).
pub fn count_batch_lines(output: &str) -> usize {
    output.lines().count()
}

/// 이동량 목록을 입고성/출고성으로 나눈다((입고 목록, 출고 목록)).
pub fn partition_by_direction(deltas: &[i64]) -> (Vec<i64>, Vec<i64>) {
    let inbound: Vec<i64> = deltas.iter().copied().filter(|d| is_inbound(*d)).collect();
    let outbound: Vec<i64> = deltas.iter().copied().filter(|d| is_outbound(*d)).collect();
    (inbound, outbound)
}

/// 이동량 목록 중 절대값이 가장 큰 것을 찾는다(가장 영향이 큰 이동 확인용).
pub fn largest_magnitude(deltas: &[i64]) -> Option<i64> {
    deltas.iter().copied().max_by_key(|d| d.abs())
}

/// SKU별 이동량을 순서대로 적용했을 때 최종 재고를 미리 계산한다(저장소
/// 없이 순수 계산 — 초기 수량과 이동량 목록만 받는다).
pub fn preview_batch_result(initial_qty: i64, deltas: &[i64]) -> i64 {
    deltas.iter().fold(initial_qty, |qty, d| apply_movement(qty, *d))
}

/// 이동량이 저장소의 재고를 완전히 소진시키는지(적용 후 0이 되는지)
/// 미리 판정한다.
pub fn would_deplete(current_qty: i64, delta: i64) -> bool {
    apply_movement(current_qty, delta) == 0
}

/// 배치 실행 결과 텍스트에서 수동 승인이 필요하다고 표시된 줄의 개수를 센다.
pub fn count_approval_required_lines(output: &str) -> usize {
    output.lines().filter(|l| l.contains("수동 승인 필요")).count()
}

/// 이동 커맨드의 승인 임계값(현재 고정값)을 조회한다(향후 설정 가능화
/// 대비 — 지금은 상수로 고정되어 있다는 사실을 코드 한 곳에 남겨둔다).
pub const APPROVAL_THRESHOLD_KRW: i64 = 1000;
