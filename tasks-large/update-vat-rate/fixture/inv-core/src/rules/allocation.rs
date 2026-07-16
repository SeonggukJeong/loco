//! 창고 배분 규칙(자유 구현) — 재고를 여러 창고에 나누는 로직 모음.

use super::WarehouseGrade;

/// 배분 후보 창고 하나의 요약 정보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationCandidate {
    pub warehouse_code: String,
    pub grade: WarehouseGrade,
    pub available_capacity: u32,
    pub distance_km: u32,
}

/// 배분 결과 한 건.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationResult {
    pub warehouse_code: String,
    pub allocated_qty: u32,
}

/// 등급이 Central인 후보를 우선하고, 동률이면 거리 순으로 정렬한다.
pub fn rank_candidates(mut candidates: Vec<AllocationCandidate>) -> Vec<AllocationCandidate> {
    candidates.sort_by(|a, b| {
        grade_rank(&a.grade)
            .cmp(&grade_rank(&b.grade))
            .then_with(|| a.distance_km.cmp(&b.distance_km))
    });
    candidates
}

fn grade_rank(grade: &WarehouseGrade) -> u8 {
    match grade {
        WarehouseGrade::Central => 0,
        WarehouseGrade::Regional => 1,
        WarehouseGrade::Local => 2,
    }
}

/// 요청 수량을 순위가 높은 후보부터 그리디하게 채운다.
///
/// 각 창고는 자신의 `available_capacity`를 넘지 않는 범위에서 배분받는다.
/// 전량 배분에 실패하면(용량 부족) 부분 배분 결과를 그대로 반환한다.
pub fn allocate_greedy(candidates: &[AllocationCandidate], requested_qty: u32) -> Vec<AllocationResult> {
    let ranked = rank_candidates(candidates.to_vec());
    let mut remaining = requested_qty;
    let mut results = Vec::new();
    for candidate in ranked {
        if remaining == 0 {
            break;
        }
        let take = candidate.available_capacity.min(remaining);
        if take > 0 {
            results.push(AllocationResult { warehouse_code: candidate.warehouse_code.clone(), allocated_qty: take });
            remaining -= take;
        }
    }
    results
}

/// 그리디 배분 결과가 요청 수량을 전부 채웠는지 확인한다.
pub fn is_fully_allocated(results: &[AllocationResult], requested_qty: u32) -> bool {
    let total: u32 = results.iter().map(|r| r.allocated_qty).sum();
    total >= requested_qty
}

/// 배분 결과의 미충족 수량(부족분)을 계산한다.
pub fn shortfall(results: &[AllocationResult], requested_qty: u32) -> u32 {
    let total: u32 = results.iter().map(|r| r.allocated_qty).sum();
    requested_qty.saturating_sub(total)
}

/// 후보 목록 중 특정 지역 접두(예: "SEL")로 시작하는 창고만 걸러낸다.
pub fn candidates_in_region<'a>(candidates: &'a [AllocationCandidate], region_prefix: &str) -> Vec<&'a AllocationCandidate> {
    candidates.iter().filter(|c| c.warehouse_code.starts_with(region_prefix)).collect()
}

/// 후보 목록의 총 가용 용량.
pub fn total_available_capacity(candidates: &[AllocationCandidate]) -> u32 {
    candidates.iter().map(|c| c.available_capacity).sum()
}

/// 요청 수량이 후보들의 총 가용 용량으로 충족 가능한지 사전 점검한다.
pub fn can_satisfy(candidates: &[AllocationCandidate], requested_qty: u32) -> bool {
    total_available_capacity(candidates) >= requested_qty
}

/// 배분 결과를 창고 코드 오름차순으로 정렬한다(보고서 출력용 정규화).
pub fn sort_results_by_code(mut results: Vec<AllocationResult>) -> Vec<AllocationResult> {
    results.sort_by(|a, b| a.warehouse_code.cmp(&b.warehouse_code));
    results
}

/// 두 배분 결과 목록을 병합한다(같은 창고 코드는 수량을 합산).
pub fn merge_results(a: &[AllocationResult], b: &[AllocationResult]) -> Vec<AllocationResult> {
    let mut merged: Vec<AllocationResult> = a.to_vec();
    for item in b {
        if let Some(existing) = merged.iter_mut().find(|r| r.warehouse_code == item.warehouse_code) {
            existing.allocated_qty += item.allocated_qty;
        } else {
            merged.push(item.clone());
        }
    }
    merged
}

/// 배분 결과 중 가장 많이 배분받은 창고 코드를 찾는다(동률이면 코드 오름차순 첫 항목).
pub fn top_allocated(results: &[AllocationResult]) -> Option<String> {
    let sorted = sort_results_by_code(results.to_vec());
    sorted.into_iter().max_by_key(|r| r.allocated_qty).map(|r| r.warehouse_code)
}

/// 백오더(미충족 수요) 항목 한 건 — 배분에 실패한 수량을 추적한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackorderEntry {
    pub sku: String,
    pub shortfall_qty: u32,
    pub created_epoch: i64,
}

/// 그리디 배분 후 미충족분이 있으면 백오더 항목을 생성한다.
pub fn to_backorder(sku: &str, results: &[AllocationResult], requested_qty: u32, epoch: i64) -> Option<BackorderEntry> {
    let gap = shortfall(results, requested_qty);
    if gap == 0 {
        None
    } else {
        Some(BackorderEntry { sku: sku.to_string(), shortfall_qty: gap, created_epoch: epoch })
    }
}

/// 백오더 목록의 총 부족 수량.
pub fn total_backorder_qty(entries: &[BackorderEntry]) -> u32 {
    entries.iter().map(|e| e.shortfall_qty).sum()
}

/// 백오더 목록을 부족 수량 내림차순으로 정렬한다(긴급도 높은 순).
pub fn sort_backorders_by_severity(mut entries: Vec<BackorderEntry>) -> Vec<BackorderEntry> {
    entries.sort_by(|a, b| b.shortfall_qty.cmp(&a.shortfall_qty).then_with(|| a.sku.cmp(&b.sku)));
    entries
}

/// 2단계 배분: 1차로 등급 우선순위 그리디 배분을 시도하고, 남은 수량은
/// 거리 기준(가까운 순)으로 재시도한다. 두 단계 결과를 병합해 반환한다.
pub fn allocate_two_pass(candidates: &[AllocationCandidate], requested_qty: u32) -> Vec<AllocationResult> {
    let first_pass = allocate_greedy(candidates, requested_qty);
    let remaining = shortfall(&first_pass, requested_qty);
    if remaining == 0 {
        return first_pass;
    }
    let mut by_distance = candidates.to_vec();
    by_distance.sort_by(|a, b| a.distance_km.cmp(&b.distance_km));
    // 1차에서 이미 소진된 창고의 남은 용량만 고려한다.
    let mut remaining_capacity: Vec<AllocationCandidate> = by_distance
        .into_iter()
        .map(|mut c| {
            let already = first_pass.iter().find(|r| r.warehouse_code == c.warehouse_code).map(|r| r.allocated_qty).unwrap_or(0);
            c.available_capacity = c.available_capacity.saturating_sub(already);
            c
        })
        .collect();
    remaining_capacity.retain(|c| c.available_capacity > 0);
    let second_pass = allocate_greedy(&remaining_capacity, remaining);
    merge_results(&first_pass, &second_pass)
}

/// 특정 창고가 배분 후보 목록에 포함되어 있는지 검사한다.
pub fn contains_warehouse(candidates: &[AllocationCandidate], warehouse_code: &str) -> bool {
    candidates.iter().any(|c| c.warehouse_code == warehouse_code)
}

/// 후보 목록에서 특정 등급만 걸러낸다.
pub fn candidates_of_grade(candidates: &[AllocationCandidate], grade: &WarehouseGrade) -> Vec<AllocationCandidate> {
    candidates.iter().filter(|c| c.grade == *grade).cloned().collect()
}

/// 배분 결과 목록을 창고별 배분 비율(%, 총 배분량 대비)로 변환한다.
pub fn allocation_shares_percent(results: &[AllocationResult]) -> Vec<(String, u32)> {
    let total: u32 = results.iter().map(|r| r.allocated_qty).sum();
    if total == 0 {
        return Vec::new();
    }
    results
        .iter()
        .map(|r| (r.warehouse_code.clone(), r.allocated_qty.saturating_mul(100) / total))
        .collect()
}

/// 배분 결과가 특정 창고에 대해 최대 허용치(cap)를 넘지 않는지 검사한다.
pub fn respects_cap(results: &[AllocationResult], warehouse_code: &str, cap: u32) -> bool {
    results
        .iter()
        .find(|r| r.warehouse_code == warehouse_code)
        .map(|r| r.allocated_qty <= cap)
        .unwrap_or(true)
}

/// 배분 결과 목록에서 배분량이 0인 항목을 제거한다(정리용).
pub fn drop_zero_allocations(results: Vec<AllocationResult>) -> Vec<AllocationResult> {
    results.into_iter().filter(|r| r.allocated_qty > 0).collect()
}

/// 두 배분 계획을 비교해 차이가 있는 창고 코드 목록을 반환한다(변경 감사용).
pub fn diff_warehouse_codes(a: &[AllocationResult], b: &[AllocationResult]) -> Vec<String> {
    let mut codes: Vec<String> = Vec::new();
    for r in a {
        let other = b.iter().find(|x| x.warehouse_code == r.warehouse_code).map(|x| x.allocated_qty);
        if other != Some(r.allocated_qty) {
            codes.push(r.warehouse_code.clone());
        }
    }
    for r in b {
        if !a.iter().any(|x| x.warehouse_code == r.warehouse_code) {
            codes.push(r.warehouse_code.clone());
        }
    }
    codes.sort();
    codes.dedup();
    codes
}
