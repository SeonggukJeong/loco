//! 배분·이동·유효성 규칙 모음 (코어 비즈니스 로직).
//!
//! 원래는 pricing/allocation처럼 서브모듈로 나눠 관리할 계획이었으나, 새
//! 규칙이 필요할 때마다 여기 계속 덧붙여지며 리팩터링이 미뤄진 채
//! 누적되었다 — 사내에서 실제로 자주 벌어지는 패턴이다.

pub mod pricing;
pub mod allocation;

// ---------------------------------------------------------------------
// 섹션 1: 수량/비율 유효성 규칙
// ---------------------------------------------------------------------

/// 요청 수량이 0보다 큰지 검사한다.
///
/// 0 이하 수량은 시스템상 무의미한 요청이므로 항상 거부한다.
pub fn is_valid_qty(qty: i64) -> bool {
    qty > 0
}

/// 백분율 값이 0~100 범위인지 검사한다.
///
/// 할인율/세율 등 퍼센트 필드 전반에 재사용되는 공용 검사다.
pub fn is_valid_percent(p: u32) -> bool {
    p <= 100
}

/// 리드타임(발주부터 입고까지 일수)이 현실적인 범위인지 검사한다.
///
/// 0일은 즉시 입고를 뜻하므로 유효하지 않고, 365일을 넘는 리드타임은
/// 데이터 입력 오류로 간주한다.
pub fn is_valid_lead_time(days: u32) -> bool {
    days > 0 && days <= 365
}

/// 요청 수량이 배치 크기의 배수인지 검사한다.
///
/// 예: 박스 단위로만 출고 가능한 품목의 경우 배치 크기가 박스당 수량이다.
pub fn is_valid_batch_multiple(qty: u32, batch_size: u32) -> bool {
    batch_size != 0 && qty % batch_size == 0
}

/// 실측값이 기대값의 허용 오차 범위 안에 있는지 검사한다.
///
/// 재고 실사(cycle count)에서 전산 재고와 실사 재고 차이를 판정할 때 쓴다.
pub fn is_within_tolerance(expected: i64, actual: i64, tolerance: i64) -> bool {
    (expected - actual).abs() <= tolerance
}

/// 우선순위 레벨이 1~5 범위인지 검사한다(1이 가장 높은 우선순위).
pub fn is_valid_priority(level: u8) -> bool {
    (1..=5).contains(&level)
}

/// 등급 코드 문자열이 알려진 등급("A"/"B"/"C")인지 검사한다.
///
/// `WarehouseGrade`와는 별개의, 상품 등급 코드다 — 이름이
/// 비슷해 혼동하기 쉬우니 주의.
pub fn is_valid_grade_code(code: &str) -> bool {
    matches!(code, "A" | "B" | "C")
}

/// 반품 사유 코드가 알려진 값인지 검사한다.
pub fn is_valid_return_reason(code: &str) -> bool {
    matches!(code, "DEFECT" | "WRONG_ITEM" | "CUSTOMER_CHANGE" | "OVERSTOCK")
}

/// 재고 이동 유형 코드가 알려진 값인지 검사한다.
pub fn is_valid_movement_type(kind: &str) -> bool {
    matches!(kind, "IN" | "OUT" | "TRANSFER" | "ADJUST")
}

/// 순환 실사(cycle count) 주기가 정책상 허용된 값인지 검사한다.
///
/// 7일/14일/30일 주기만 허용한다 — 그 외 주기는 운영팀 승인이 별도로
/// 필요하므로 이 함수의 검사 대상이 아니다.
pub fn is_valid_cycle_count_interval(days: u32) -> bool {
    days == 7 || days == 14 || days == 30
}

/// 단위 환산 계수가 합리적인 범위(1~10000)인지 검사한다.
pub fn is_valid_uom_conversion_factor(factor: u32) -> bool {
    factor > 0 && factor <= 10_000
}

/// 단위 코드 문자열이 알려진 값("EA"/"BOX"/"PLT")인지 검사한다.
pub fn is_valid_uom_str(code: &str) -> bool {
    matches!(code, "EA" | "BOX" | "PLT")
}

/// 발주 접수 가능 시간(6시~22시)인지 검사한다.
///
/// 야간 시간대는 수동 승인 없이는 발주를 접수하지 않는 정책을 반영한다.
pub fn is_valid_order_window(hour: u32) -> bool {
    (6..=22).contains(&hour)
}

// ---------------------------------------------------------------------
// 섹션 2: 재고 상태 판정 규칙
// ---------------------------------------------------------------------

/// 현재고가 최소 재고 기준보다 낮은지 판정한다.
pub fn is_below_min_stock(on_hand: u32, min_stock: u32) -> bool {
    on_hand < min_stock
}

/// 현재고가 최대 재고 기준을 초과했는지 판정한다.
pub fn is_above_max_stock(on_hand: u32, max_stock: u32) -> bool {
    on_hand > max_stock
}

/// 재고가 완전히 소진되었는지(품절) 판정한다.
pub fn is_stockout(on_hand: u32) -> bool {
    on_hand == 0
}

/// 과잉 재고 여부를 판정한다. 최대 재고 기준에 여유 버퍼(%)를 더한 값을
/// 넘으면 과잉으로 본다.
pub fn is_overstock(on_hand: u32, max_stock: u32, buffer_pct: u32) -> bool {
    let buffer = max_stock.saturating_mul(100 + buffer_pct) / 100;
    on_hand > buffer
}

/// 최근 30일 판매량 대비 현재고가 지나치게 많은지(회전율 저조) 판정한다.
///
/// 대략적인 휴리스틱이다: 30일 판매량의 10배를 넘는 재고를 저회전으로 본다.
pub fn is_low_turnover(sold_last_30d: u32, on_hand: u32) -> bool {
    on_hand > 0 && sold_last_30d.saturating_mul(10) < on_hand
}

/// 최근 90일간 판매가 전혀 없었는지(데드스톡 후보) 판정한다.
pub fn is_dead_stock(sold_last_90d: u32) -> bool {
    sold_last_90d == 0
}

/// 일 평균 판매량 기준으로 현재고가 며칠분인지 계산한다.
///
/// 일 평균이 0이면 "무한대"를 뜻하는 `u32::MAX`를 반환한다(0으로 나누기 방지).
pub fn compute_days_of_supply(on_hand: u32, daily_avg: u32) -> u32 {
    if daily_avg == 0 {
        u32::MAX
    } else {
        on_hand / daily_avg
    }
}

/// 안전 재고량을 계산한다: 일평균 × 리드타임 × 서비스 팩터(%) / 100.
///
/// 서비스 팩터는 보통 100~150 범위(서비스 수준이 높을수록 큰 값)로 쓰인다.
pub fn compute_safety_stock(daily_avg: u32, lead_days: u32, service_factor: u32) -> u32 {
    daily_avg.saturating_mul(lead_days).saturating_mul(service_factor) / 100
}

/// 재주문점(reorder point)을 계산한다: 리드타임 중 예상 소비 + 안전 재고.
pub fn compute_reorder_point(daily_avg: u32, lead_days: u32, safety_stock: u32) -> u32 {
    daily_avg.saturating_mul(lead_days).saturating_add(safety_stock)
}

/// 미출고 수요(백오더) 수량을 계산한다: 수요가 현재고를 초과하는 만큼.
pub fn compute_backorder_qty(demand: u32, on_hand: u32) -> u32 {
    demand.saturating_sub(on_hand)
}

/// 목표 재고 수준까지 보충해야 할 수량을 계산한다(입고 예정분 차감).
pub fn compute_replenishment_qty(target: u32, on_hand: u32, on_order: u32) -> u32 {
    target.saturating_sub(on_hand).saturating_sub(on_order)
}

/// 현재고가 재주문점 이하로 내려와 긴급 보충이 필요한지 판정한다.
pub fn is_restock_urgent(on_hand: u32, reorder_point: u32) -> bool {
    on_hand <= reorder_point
}

/// 판매 속도(일 평균) 대비 리드타임이 너무 길어 품절 위험이 있는지 판정한다.
///
/// 현재고가 "리드타임 동안의 예상 소비량"보다 적으면 위험으로 본다.
pub fn is_stockout_risk_during_lead_time(on_hand: u32, daily_avg: u32, lead_days: u32) -> bool {
    on_hand < daily_avg.saturating_mul(lead_days)
}

// ---------------------------------------------------------------------
// 섹션 3: 이동/배분 규칙
// ---------------------------------------------------------------------

/// 가용 재고가 요청 수량을 충족하는지 검사한다.
pub fn can_allocate_to_warehouse(available: u32, requested: u32) -> bool {
    available >= requested
}

/// 수량을 [min, max] 구간으로 자른다(i64 버전).
pub fn clamp_quantity(qty: i64, min: i64, max: i64) -> i64 {
    qty.clamp(min, max)
}

/// 수량을 케이스(박스) 단위로 올림한다.
///
/// 케이스 크기가 0이면(단위 미설정) 원래 수량을 그대로 반환한다.
pub fn round_to_case_size(qty: u32, case_size: u32) -> u32 {
    if case_size == 0 {
        qty
    } else {
        ((qty + case_size - 1) / case_size) * case_size
    }
}

/// 총 수량을 창고 수만큼 균등 배분한다. 나머지는 앞쪽 창고부터 1개씩
/// 더 배분한다(총합이 정확히 `total`이 되도록 보장).
pub fn split_allocation_even(total: u32, warehouse_count: u32) -> Vec<u32> {
    if warehouse_count == 0 {
        return Vec::new();
    }
    let base = total / warehouse_count;
    let remainder = total % warehouse_count;
    (0..warehouse_count)
        .map(|i| if i < remainder { base + 1 } else { base })
        .collect()
}

/// 총 수량을 비율(ratio) 목록에 따라 배분한다. 비율 합이 0이면 전부 0을
/// 반환한다(0으로 나누기 방지).
pub fn allocate_by_ratio(total: u32, ratios: &[u32]) -> Vec<u32> {
    let sum: u32 = ratios.iter().sum();
    if sum == 0 {
        return vec![0; ratios.len()];
    }
    ratios.iter().map(|r| total.saturating_mul(*r) / sum).collect()
}

/// 이동 출발지/도착지 쌍이 유효한지 검사한다(둘 다 비어있지 않고 서로 달라야 함).
pub fn is_valid_transfer_pair(from_code: &str, to_code: &str) -> bool {
    !from_code.is_empty() && !to_code.is_empty() && from_code != to_code
}

/// 이동 수량이 승인 임계값 이상이라 수동 승인이 필요한지 판정한다.
pub fn movement_requires_approval(qty: i64, threshold: i64) -> bool {
    qty.abs() >= threshold
}

/// 우선순위 레벨이 유효 범위(1~5)인지 검사한다(`is_valid_priority`와 동일
/// 로직이지만 이동 규칙 문맥에서 별도로 쓰인다).
pub fn is_valid_priority_level(level: u8) -> bool {
    (1..=5).contains(&level)
}

/// 공급일수와 긴급 여부로 피킹 우선순위(1이 최우선)를 계산한다.
pub fn compute_pick_priority(days_of_supply: u32, is_urgent: bool) -> u8 {
    if is_urgent {
        1
    } else if days_of_supply < 3 {
        2
    } else if days_of_supply < 7 {
        3
    } else {
        4
    }
}

/// 배분 후보의 점수를 계산한다: 재고 비율은 가점, 거리(km)는 감점.
///
/// 점수가 높을수록 우선 배분 대상이다. 이 함수는 배분 알고리즘의 세부
/// 구현에 쓰이며, `allocation.rs`의 등급 기반 랭킹과는 별도 경로다.
pub fn compute_allocation_score(distance_km: u32, stock_ratio: u32) -> i64 {
    (stock_ratio as i64) * 100 - (distance_km as i64)
}

/// 같은 지역이 아니면 교차 지역 이동 한도(cross_region_limit) 이하만
/// 허용하는지 검사한다.
pub fn is_transfer_allowed_cross_region(same_region: bool, qty: u32, cross_region_limit: u32) -> bool {
    same_region || qty <= cross_region_limit
}

/// 창고 용량 초과 없이 입고분을 수용할 수 있는지 검사한다.
pub fn validate_warehouse_capacity(current_load: u32, capacity: u32, incoming: u32) -> bool {
    current_load.saturating_add(incoming) <= capacity
}

/// 창고 가동률(%)을 계산한다: 현재 적재량 / 용량 × 100.
pub fn compute_utilization_ratio(current_load: u32, capacity: u32) -> u32 {
    if capacity == 0 {
        0
    } else {
        current_load.saturating_mul(100) / capacity
    }
}

/// 등급 전환("Local"→"Regional" 등)이 정책상 허용되는 경로인지 검사한다.
///
/// 등급은 한 단계씩만 승격/강등 가능하다(Local과 Central 사이의 직접
/// 전환은 허용하지 않는다).
pub fn is_valid_grade_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("Local", "Regional") | ("Regional", "Central") | ("Regional", "Local") | ("Central", "Regional")
    )
}

/// 배분 결과 목록 중 특정 창고 코드에 배분된 수량 합계를 구한다.
pub fn allocated_qty_for(pairs: &[(String, u32)], warehouse_code: &str) -> u32 {
    pairs.iter().filter(|(code, _)| code == warehouse_code).map(|(_, qty)| *qty).sum()
}

// ---------------------------------------------------------------------
// 섹션 4: 공급망/운영 규칙
// ---------------------------------------------------------------------

/// 공급업체 리드타임이 정책상 허용 상한 이내인지 검사한다.
pub fn validate_supplier_lead_time(days: u32, max_allowed: u32) -> bool {
    days <= max_allowed
}

/// 거래 금액이 수동 검토 임계값 이상이라 검토가 필요한지 판정한다.
pub fn is_manual_review_required(amount_krw: i64, review_threshold: i64) -> bool {
    amount_krw >= review_threshold
}

/// 일일 이동 건수가 허용 상한을 넘지 않는지 검사한다(과도한 이동 방지).
pub fn validate_max_daily_movements(count: u32, max_allowed: u32) -> bool {
    count <= max_allowed
}

/// 카테고리 코드가 위험물(hazmat) 제한 대상인지 검사한다.
pub fn is_hazmat_restricted(category_code: &str) -> bool {
    matches!(category_code, "HZ1" | "HZ2" | "HZ3")
}

/// 유통기한까지 남은 일수가 최소 진열 가능 일수 이상인지 검사한다.
///
/// 음수 `days_until_expiry`는 이미 만료된 상태를 뜻한다.
pub fn validate_expiry_window(days_until_expiry: i64, min_shelf_days: i64) -> bool {
    days_until_expiry >= min_shelf_days
}

/// 월(1~12) 값으로 성수기(11~12월) 여부를 판정한다.
pub fn is_peak_season_rule(month: u32) -> bool {
    matches!(month, 11 | 12)
}

/// 실사 수량과 전산 수량의 차이가 허용 오차 비율 이내인지 검사한다.
///
/// 전산 수량이 0인 경우 실사 수량도 0이어야 통과한다(0으로 나누기 방지 경로).
pub fn validate_count_variance_tolerance(counted: i64, recorded: i64, tolerance_pct: u32) -> bool {
    if recorded == 0 {
        counted == 0
    } else {
        let diff = (counted - recorded).abs();
        let allowed = (recorded.abs() * tolerance_pct as i64) / 100;
        diff <= allowed
    }
}

/// 두 시각(유닉스 epoch 초) 사이의 경과 일수를 계산한다(음수는 0으로 clamp).
pub fn compute_days_since(epoch_then: i64, epoch_now: i64) -> i64 {
    (epoch_now - epoch_then).max(0) / 86_400
}

/// 반품 처리 시 원 거래 대비 반품 수량이 초과되지 않았는지 검사한다.
pub fn is_valid_return_qty(original_qty: u32, return_qty: u32) -> bool {
    return_qty > 0 && return_qty <= original_qty
}

/// 발주 승인 체인에서 다음 승인자가 필요한지(금액 기준 다단계 승인) 판정한다.
///
/// 1차 임계값을 넘으면 팀장 승인, 2차 임계값을 넘으면 추가로 본부장 승인이
/// 필요하다는 식의 정책을 반영한다.
pub fn required_approval_level(amount_krw: i64, tier1_threshold: i64, tier2_threshold: i64) -> u8 {
    if amount_krw >= tier2_threshold {
        2
    } else if amount_krw >= tier1_threshold {
        1
    } else {
        0
    }
}

/// 창고 간 이동 시 파렛트 단위로 온전히 나누어 떨어지는지 검사한다.
pub fn is_full_pallet_multiple(qty: u32, pallet_size: u32) -> bool {
    pallet_size != 0 && qty % pallet_size == 0
}

/// 최소/최대 주문 수량 범위 안에 있는지 한 번에 검사하는 편의 함수.
pub fn is_within_order_bounds(qty: u32, min_order: u32, max_order: u32) -> bool {
    qty >= min_order && qty <= max_order
}

/// 두 창고 코드가 같은 배송권역(zone)에 속하는지, 3자 접두 비교로 판정한다.
pub fn is_same_delivery_zone(a: &str, b: &str) -> bool {
    a.len() >= 3 && b.len() >= 3 && a[..3] == b[..3]
}

// ---------------------------------------------------------------------
// 섹션 5: 알림/경보 임계값 규칙
// ---------------------------------------------------------------------

/// 공급일수가 0이면(당장 품절) 긴급(critical) 경보로 판정한다.
pub fn is_critical_alert(days_of_supply: u32) -> bool {
    days_of_supply == 0
}

/// 공급일수가 경고 기준일 이하이면 경고(warning) 경보로 판정한다.
pub fn is_warning_alert(days_of_supply: u32, warn_days: u32) -> bool {
    days_of_supply <= warn_days
}

/// 경보 등급을 계산한다: 2=긴급, 1=경고, 0=정상.
pub fn alert_level_for(days_of_supply: u32, warn_days: u32) -> u8 {
    if is_critical_alert(days_of_supply) {
        2
    } else if is_warning_alert(days_of_supply, warn_days) {
        1
    } else {
        0
    }
}

/// 직전 경보 발송 후 쿨다운 시간이 지나지 않았으면 중복 경보를 억제한다.
///
/// 동일한 상황에 대해 반복 알림이 쏟아지는 것을 막기 위한 규칙이다.
pub fn should_suppress_duplicate_alert(last_alert_epoch: i64, now_epoch: i64, cooldown_secs: i64) -> bool {
    (now_epoch - last_alert_epoch) < cooldown_secs
}

/// 측정 온도가 허용 범위를 벗어났는지(콜드체인 이탈) 판정한다.
pub fn is_temperature_excursion(min_c: f64, max_c: f64, reading_c: f64) -> bool {
    reading_c < min_c || reading_c > max_c
}

/// 측정 습도가 허용 범위를 벗어났는지 판정한다.
pub fn is_humidity_out_of_range(min_pct: f64, max_pct: f64, reading_pct: f64) -> bool {
    reading_pct < min_pct || reading_pct > max_pct
}

// ---------------------------------------------------------------------
// 섹션 6: 계절성/수요예측 보조 규칙
// ---------------------------------------------------------------------

/// 월(1~12)에 대응하는 계절 수요 배율을 반환한다.
///
/// 성수기(11~12월)는 1.4배, 비수기(1~2월)는 0.8배, 여름 성수기(6~8월)는
/// 1.1배, 그 외에는 1.0배(변동 없음)로 어림한다.
pub fn season_factor_for_month(month: u32) -> f64 {
    match month {
        11 | 12 => 1.4,
        1 | 2 => 0.8,
        6 | 7 | 8 => 1.1,
        _ => 1.0,
    }
}

/// 연말 성수기(11~12월) 여부를 판정한다.
pub fn is_holiday_surge_month(month: u32) -> bool {
    matches!(month, 11 | 12)
}

/// 기본 예측치에 계절 배율을 곱해 조정한다.
pub fn adjust_forecast_for_season(base_forecast: u32, month: u32) -> u32 {
    let factor = season_factor_for_month(month);
    ((base_forecast as f64) * factor) as u32
}

/// 지수평활 가중치(alpha)를 백분율 입력에서 0.0~1.0 범위로 변환한다.
pub fn smoothing_weight(alpha_pct: u32) -> f64 {
    (alpha_pct.min(100) as f64) / 100.0
}

/// 단순 지수평활법으로 새 평균값을 계산한다.
///
/// `alpha_pct`가 클수록 최신 값(`new_value`)의 비중이 커진다.
pub fn exponential_smooth(prev_avg: f64, new_value: f64, alpha_pct: u32) -> f64 {
    let alpha = smoothing_weight(alpha_pct);
    alpha * new_value + (1.0 - alpha) * prev_avg
}

// ---------------------------------------------------------------------
// 섹션 7: 감사/이력 관련 규칙
// ---------------------------------------------------------------------

/// 특정 액션이 감사 추적(audit trail) 기록 대상인지 판정한다.
pub fn requires_audit_trail(action: &str) -> bool {
    matches!(action, "DELETE" | "ADJUST" | "OVERRIDE")
}

/// 필드명이 민감 정보(원가, 계약 ID, 마진율 등)인지 판정한다.
pub fn is_sensitive_field(field_name: &str) -> bool {
    matches!(field_name, "cost_price" | "vendor_contract_id" | "margin_pct")
}

/// 역할(role)별로 잠기기 전 허용되는 최대 수정 횟수를 반환한다.
pub fn max_edits_before_lock(role: &str) -> u32 {
    match role {
        "ADMIN" => 999,
        "MANAGER" => 20,
        _ => 3,
    }
}

/// 레코드 생성 후 수정 허용 시간(window) 안인지 판정한다.
pub fn is_edit_within_window(created_epoch: i64, now_epoch: i64, window_secs: i64) -> bool {
    (now_epoch - created_epoch) <= window_secs
}

// ---------------------------------------------------------------------
// 섹션 8: 벤더/구매 관련 규칙
// ---------------------------------------------------------------------

/// 신뢰도 점수(0.0~1.0)가 우대 벤더 기준을 충족하는지 판정한다.
pub fn is_preferred_vendor(reliability_score: f64) -> bool {
    reliability_score >= 0.9
}

/// 신뢰도 점수와 리드타임으로 벤더 리스크 등급을 계산한다: 2=고위험,
/// 1=중위험, 0=저위험.
pub fn vendor_risk_level(reliability_score: f64, lead_time_days: u32) -> u8 {
    if reliability_score < 0.5 || lead_time_days > 60 {
        2
    } else if reliability_score < 0.8 || lead_time_days > 30 {
        1
    } else {
        0
    }
}

/// 지역별 무료 배송 최소 주문 금액을 반환한다.
pub fn min_order_value_for_free_shipping(region: &str) -> i64 {
    match region {
        "SEL" => 30_000,
        "BSN" => 40_000,
        _ => 50_000,
    }
}

/// 주문 금액과 지역으로 무료 배송 대상인지 판정한다.
pub fn qualifies_for_free_shipping(order_value_krw: i64, region: &str) -> bool {
    order_value_krw >= min_order_value_for_free_shipping(region)
}

// ---------------------------------------------------------------------
// 섹션 9: 창고 배치/슬롯 규칙
// ---------------------------------------------------------------------

/// 슬롯 코드 포맷이 유효한지 검사한다(4자 이상, 첫 글자는 대문자 구역 코드).
pub fn is_valid_slot_code(code: &str) -> bool {
    code.len() >= 4 && code.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false)
}

/// 슬롯 코드에서 구역(zone) 문자를 뽑아낸다(첫 글자).
pub fn slot_zone(code: &str) -> String {
    if code.is_empty() {
        String::new()
    } else {
        code[..1].to_string()
    }
}

/// 구역이 대량(bulk) 적재 구역("B")인지 판정한다.
pub fn is_bulk_zone(zone: &str) -> bool {
    zone == "B"
}

/// 구역별 최대 적재 중량(kg)을 반환한다.
pub fn max_weight_for_zone(zone: &str) -> u32 {
    match zone {
        "B" => 2000,
        "P" => 500,
        _ => 100,
    }
}

/// 적재 중량이 해당 구역의 최대 허용치를 초과했는지 판정한다.
pub fn is_overweight_for_zone(weight_kg: u32, zone: &str) -> bool {
    weight_kg > max_weight_for_zone(zone)
}

// ---------------------------------------------------------------------
// 섹션 10: 반품/품질 검수 규칙
// ---------------------------------------------------------------------

/// 검수 결과 코드가 알려진 값("PASS"/"FAIL"/"HOLD")인지 검사한다.
pub fn is_valid_inspection_result(code: &str) -> bool {
    matches!(code, "PASS" | "FAIL" | "HOLD")
}

/// 검수 결과가 격리(quarantine) 대상인지 판정한다.
pub fn requires_quarantine(code: &str) -> bool {
    code == "FAIL"
}

/// 카테고리별 허용 최대 불량률(%)을 반환한다.
///
/// 위험물(HZ 접두)은 불량 허용치가 매우 낮게 잡혀 있다.
pub fn max_defect_rate_pct(category_code: &str) -> u32 {
    if category_code.starts_with("HZ") {
        1
    } else {
        5
    }
}

/// 표본 검수 결과가 허용 불량률 이내인지 판정한다.
///
/// 표본 크기가 0이면 판단 불가로 보고 `false`를 반환한다(0으로 나누기 방지).
pub fn is_within_defect_rate(defect_count: u32, sample_size: u32, category_code: &str) -> bool {
    if sample_size == 0 {
        return false;
    }
    let rate_pct = defect_count.saturating_mul(100) / sample_size;
    rate_pct <= max_defect_rate_pct(category_code)
}

/// 일련번호 구간이 유효한지 검사한다(시작 > 0, 끝 >= 시작).
pub fn is_valid_serial_range(start: u32, end: u32) -> bool {
    start > 0 && end >= start
}

/// 값이 [start, end] 구간(양 끝 포함)에 속하는지 검사한다.
pub fn count_in_range(value: u32, start: u32, end: u32) -> bool {
    value >= start && value <= end
}

/// 배치 ID 포맷이 유효한지 검사한다: `LOT-YYYY-####` (연도 4자리 + 일련 4자리).
pub fn is_valid_batch_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('-').collect();
    matches!(parts.as_slice(), ["LOT", year, seq] if year.len() == 4 && seq.len() == 4
        && year.chars().all(|c| c.is_ascii_digit())
        && seq.chars().all(|c| c.is_ascii_digit()))
}

/// 배치 ID에서 연도를 추출한다(포맷이 아니면 `None`).
pub fn batch_year_from_id(id: &str) -> Option<u32> {
    if !is_valid_batch_id(id) {
        return None;
    }
    id.split('-').nth(1).and_then(|y| y.parse::<u32>().ok())
}

/// 배치 생산 연도와 현재 연도, 보관 가능 연수로 만료 여부를 판정한다.
pub fn is_expired_batch(batch_year: u32, current_year: u32, shelf_years: u32) -> bool {
    current_year.saturating_sub(batch_year) >= shelf_years
}

/// 불량률(%)에 따른 권장 폐기/재작업 조치를 문자열로 반환한다.
pub fn recommended_disposal_action(defect_rate_pct: u32) -> &'static str {
    if defect_rate_pct >= 20 {
        "DISPOSE"
    } else if defect_rate_pct >= 5 {
        "REWORK"
    } else {
        "RELEASE"
    }
}

// ---------------------------------------------------------------------
// 섹션 11: 라벨/문서 규칙
// ---------------------------------------------------------------------

/// 출고 라벨에 바코드 인쇄가 필요한지 판정한다(위험물/고가품 예외 없이
/// 전 품목 공통 규칙 — 현재는 항상 true지만, 정책 변경 시 여기만 고치면
/// 되도록 별도 함수로 분리해 두었다).
pub fn requires_barcode_label(_category_code: &str) -> bool {
    true
}

/// 원산지 표기가 필수인 카테고리인지 판정한다.
pub fn requires_origin_label(category_code: &str) -> bool {
    matches!(category_code, "FD" | "CO" | "TX")
}

/// 두 문서 번호가 같은 회계연도에 속하는지, 앞 4자리(연도)로 비교한다.
pub fn is_same_fiscal_year(doc_no_a: &str, doc_no_b: &str) -> bool {
    doc_no_a.len() >= 4 && doc_no_b.len() >= 4 && doc_no_a[..4] == doc_no_b[..4]
}

/// 전표 번호 포맷이 유효한지 검사한다: 연도 4자리 + '-' + 일련 6자리.
pub fn is_valid_document_number(doc_no: &str) -> bool {
    let parts: Vec<&str> = doc_no.split('-').collect();
    matches!(parts.as_slice(), [year, seq] if year.len() == 4 && seq.len() == 6
        && year.chars().all(|c| c.is_ascii_digit())
        && seq.chars().all(|c| c.is_ascii_digit()))
}

/// 문서 보관 기간(년)이 지났는지 판정한다(세무 보관 의무 등에 대응).
pub fn is_past_retention_period(doc_year: u32, current_year: u32, retention_years: u32) -> bool {
    current_year.saturating_sub(doc_year) > retention_years
}

/// 라벨에 표기할 중량 단위 문자열을 정규화한다("kg"/"g"만 허용, 그 외는
/// 기본값 "kg"로 대체).
pub fn normalize_weight_unit(unit: &str) -> &'static str {
    match unit.to_ascii_lowercase().as_str() {
        "g" => "g",
        _ => "kg",
    }
}

/// 그램 단위 중량을 킬로그램으로 환산한다(소수점 이하는 버림).
pub fn grams_to_kg_floor(grams: u32) -> u32 {
    grams / 1000
}

/// 킬로그램 단위 중량을 그램으로 환산한다.
pub fn kg_to_grams(kg: u32) -> u32 {
    kg.saturating_mul(1000)
}

// ---------------------------------------------------------------------
// 섹션 12: 창고 등급 및 재고 임계값
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarehouseGrade { Central, Regional, Local }
pub fn restock_threshold(daily_avg: u32, lead_days: u32, grade: WarehouseGrade) -> u32 {
    let base = daily_avg.saturating_mul(lead_days);
    match grade {
        WarehouseGrade::Central => base.saturating_add(daily_avg.saturating_mul(7)),
        WarehouseGrade::Regional => base.saturating_add(daily_avg.saturating_mul(3)),
        WarehouseGrade::Local => base,
    }
}
