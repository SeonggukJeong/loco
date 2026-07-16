//! 창고 용량 점검.
//!
//! 저장소에 재고를 추가로 반영하기 전, 대상 창고가 물리적으로 그만큼을
//! 더 수용할 수 있는지 미리 확인하는 헬퍼. inv-core의 창고 용량 규칙을
//! 그대로 재사용한다.

use inv_core::rules::{compute_utilization_ratio, validate_warehouse_capacity};

/// 창고에 입고분을 반영해도 용량을 넘지 않는지 검사한다.
pub fn can_accept_inbound(current_load: u32, capacity: u32, incoming: u32) -> bool {
    validate_warehouse_capacity(current_load, capacity, incoming)
}

/// 여러 입고 요청을 순서대로 반영했을 때 어느 시점에 용량을 넘는지 찾는다.
/// 전부 수용 가능하면 `None`.
pub fn first_overflow_index(current_load: u32, capacity: u32, incoming_batches: &[u32]) -> Option<usize> {
    let mut load = current_load;
    for (i, &qty) in incoming_batches.iter().enumerate() {
        if !can_accept_inbound(load, capacity, qty) {
            return Some(i);
        }
        load = load.saturating_add(qty);
    }
    None
}

/// 여러 입고 요청을 모두 반영한 뒤의 가동률(%)을 계산한다.
pub fn utilization_after_batches(current_load: u32, capacity: u32, incoming_batches: &[u32]) -> u32 {
    let total_incoming: u32 = incoming_batches.iter().sum();
    compute_utilization_ratio(current_load.saturating_add(total_incoming), capacity)
}

/// 창고가 경고 수준 가동률(기본 90%)을 넘었는지 검사한다.
pub fn is_near_capacity(current_load: u32, capacity: u32, warn_threshold_percent: u32) -> bool {
    compute_utilization_ratio(current_load, capacity) >= warn_threshold_percent
}

/// 여러 창고의 (현재 적재량, 용량) 쌍 중 여유 공간이 가장 큰 창고의
/// 인덱스를 찾는다(신규 입고 배정 시 참고용).
pub fn most_available_warehouse(loads_and_capacities: &[(u32, u32)]) -> Option<usize> {
    loads_and_capacities
        .iter()
        .enumerate()
        .max_by_key(|(_, (load, capacity))| capacity.saturating_sub(*load))
        .map(|(i, _)| i)
}

/// 여러 창고 중 용량을 초과한(과적재) 창고의 인덱스 목록을 찾는다.
pub fn overloaded_warehouses(loads_and_capacities: &[(u32, u32)]) -> Vec<usize> {
    loads_and_capacities
        .iter()
        .enumerate()
        .filter(|(_, (load, capacity))| load > capacity)
        .map(|(i, _)| i)
        .collect()
}

/// 창고 목록의 평균 가동률(%)을 계산한다.
pub fn average_utilization(loads_and_capacities: &[(u32, u32)]) -> u32 {
    if loads_and_capacities.is_empty() {
        return 0;
    }
    let sum: u32 = loads_and_capacities.iter().map(|(load, cap)| compute_utilization_ratio(*load, *cap)).sum();
    sum / loads_and_capacities.len() as u32
}

/// 목표 가동률(%)을 맞추기 위해 추가로 수용 가능한 최대 수량을 계산한다.
/// 이미 목표를 넘겼으면 0.
pub fn headroom_for_target(current_load: u32, capacity: u32, target_percent: u32) -> u32 {
    let target_load = capacity.saturating_mul(target_percent) / 100;
    target_load.saturating_sub(current_load)
}

/// 여러 창고 중 지정 가동률 이상인 것들의 인덱스를 모은다(경고 대시보드용).
pub fn warehouses_above_threshold(loads_and_capacities: &[(u32, u32)], threshold_percent: u32) -> Vec<usize> {
    loads_and_capacities
        .iter()
        .enumerate()
        .filter(|(_, (load, cap))| compute_utilization_ratio(*load, *cap) >= threshold_percent)
        .map(|(i, _)| i)
        .collect()
}

/// 두 창고 상태(적재량, 용량)를 비교해 어느 쪽이 더 여유로운지(true면 a)
/// 판정한다.
pub fn a_has_more_headroom(a: (u32, u32), b: (u32, u32)) -> bool {
    let (a_load, a_cap) = a;
    let (b_load, b_cap) = b;
    a_cap.saturating_sub(a_load) > b_cap.saturating_sub(b_load)
}

/// 총 수요를 여러 창고에 여유 공간 비례로 배분한다. 총 여유 공간이
/// 0이면 전부 0을 반환한다(0으로 나누기 방지).
pub fn allocate_by_headroom(total_qty: u32, loads_and_capacities: &[(u32, u32)]) -> Vec<u32> {
    let headrooms: Vec<u32> = loads_and_capacities.iter().map(|(l, c)| c.saturating_sub(*l)).collect();
    let total_headroom: u32 = headrooms.iter().sum();
    if total_headroom == 0 {
        return vec![0; loads_and_capacities.len()];
    }
    headrooms.iter().map(|h| total_qty.saturating_mul(*h) / total_headroom).collect()
}

/// 창고가 완전히 가득 찼는지(가동률 100% 이상) 검사한다.
pub fn is_full(current_load: u32, capacity: u32) -> bool {
    current_load >= capacity
}
