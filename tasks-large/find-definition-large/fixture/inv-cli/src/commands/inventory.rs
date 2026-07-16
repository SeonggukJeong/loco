//! `inventory` 서브커맨드: 재고 현황을 조회해 출력한다.
//!
//! 이 서브커맨드 역시 온보딩/스모크 테스트용 샘플 스냅샷으로 동작한다.
//! 실제 배치에서는 저장소 파일(inv-store의 로그 포맷)을 읽어 적재하지만,
//! 그 연동은 이 커맨드의 범위 밖이다.

use inv_core::inventory::InventorySnapshot;
use inv_store::memory::MemoryStore;

/// 재고 현황 커맨드에서 쓰는 샘플 스냅샷 저장소를 만든다.
pub fn sample_store() -> MemoryStore {
    let mut store = MemoryStore::new();
    store.upsert(InventorySnapshot::new("EL-000123", 100, 20));
    store.upsert(InventorySnapshot::new("EL-000456", 50, 0));
    store.upsert(InventorySnapshot::new("FD-000789", 0, 0));
    store
}

/// 재고 현황을 조회해 출력 텍스트를 만든다. `sku_filter`가 있으면 해당
/// SKU 한 건만 표시하고, 없으면 저장소 전체를 나열한다.
pub fn execute(sku_filter: Option<&str>) -> String {
    let store = sample_store();
    match sku_filter {
        Some(sku) => match store.get(sku) {
            Some(snap) => format_snapshot(snap),
            None => format!("SKU를 찾을 수 없습니다: {sku}"),
        },
        None => format_all(&store),
    }
}

/// 스냅샷 하나를 한 줄로 포맷한다.
fn format_snapshot(snapshot: &InventorySnapshot) -> String {
    format!("{}: 현재고 {}, 예약 {}, 가용 {}", snapshot.sku, snapshot.on_hand, snapshot.reserved, snapshot.available())
}

/// 저장소 전체를 여러 줄 텍스트로 포맷한다(SKU 오름차순).
fn format_all(store: &MemoryStore) -> String {
    let mut sorted = store.all().to_vec();
    sorted.sort_by(|a, b| a.sku.cmp(&b.sku));
    let lines: Vec<String> = sorted.iter().map(format_snapshot).collect();
    if lines.is_empty() {
        "재고 데이터가 없습니다".to_string()
    } else {
        lines.join("\n")
    }
}

/// 재고 현황 출력에서 특정 SKU가 언급되었는지 검사한다(라우팅/스모크
/// 테스트 보조용).
pub fn output_mentions_sku(output: &str, sku: &str) -> bool {
    output.lines().any(|l| l.starts_with(sku))
}

/// 재고 현황 출력의 줄 수를 센다(빈 결과인지 여러 건인지 빠르게 확인).
pub fn count_output_lines(output: &str) -> usize {
    output.lines().count()
}

/// 샘플 저장소에서 가용 재고가 0인 SKU 목록을 조회한다(결품 알림용 헬퍼).
pub fn depleted_sku_report() -> String {
    let store = sample_store();
    let depleted = store.depleted_skus();
    if depleted.is_empty() {
        "결품 SKU가 없습니다".to_string()
    } else {
        format!("결품 SKU: {}", depleted.join(", "))
    }
}

/// 샘플 저장소의 전체 가용 재고 합계를 한 줄 텍스트로 만든다.
pub fn total_available_report() -> String {
    let store = sample_store();
    format!("전체 가용 재고: {}", store.total_available())
}

/// 샘플 저장소의 전체 예약 재고 합계를 한 줄 텍스트로 만든다.
pub fn total_reserved_report() -> String {
    let store = sample_store();
    format!("전체 예약 재고: {}", store.total_reserved())
}

/// 샘플 저장소의 SKU 개수를 조회한다.
pub fn sku_count() -> usize {
    sample_store().len()
}

/// 재고 현황을 가용 재고 내림차순으로 정렬해 출력한다(어느 SKU를 먼저
/// 소진 위험으로 볼지 검토할 때 쓴다).
pub fn execute_sorted_by_available() -> String {
    let mut store = sample_store();
    store.sort_by_available_desc();
    format_all(&store)
}

/// 가용 재고가 지정 임계값 미만인 SKU만 걸러 출력한다(저재고 경보용).
pub fn execute_low_stock(threshold: u32) -> String {
    let store = sample_store();
    let low: Vec<InventorySnapshot> = store.all().iter().filter(|s| s.available() < threshold).cloned().collect();
    if low.is_empty() {
        format!("가용 재고 {threshold} 미만 SKU가 없습니다")
    } else {
        low.iter().map(format_snapshot).collect::<Vec<_>>().join("\n")
    }
}

/// 재고 현황 출력 텍스트를 파싱해 SKU별 가용 재고 값을 (SKU, 값) 목록으로
/// 되돌린다(라우팅 테스트/디버그 보조용 — `format_snapshot`의 출력 형식에
/// 맞춘 단순 파서).
pub fn parse_available_values(output: &str) -> Vec<(String, u32)> {
    output
        .lines()
        .filter_map(|line| {
            let (sku, rest) = line.split_once(':')?;
            let available_part = rest.rsplit(' ').next()?;
            available_part.parse::<u32>().ok().map(|v| (sku.to_string(), v))
        })
        .collect()
}

/// 재고 현황 커맨드가 지원하는 정렬 기준 이름 목록.
pub const SORT_MODES: [&str; 2] = ["available", "sku"];

/// 정렬 기준 이름이 이 커맨드가 지원하는 값인지 검사한다.
pub fn is_valid_sort_mode(mode: &str) -> bool {
    SORT_MODES.contains(&mode)
}

/// 재고 현황을 SKU 오름차순으로 정렬해 출력한다(`execute_sorted_by_available`
/// 와 대비되는 정렬 기준).
pub fn execute_sorted_by_sku() -> String {
    let mut store = sample_store();
    store.sort_by_sku();
    format_all(&store)
}

/// 정렬 기준 이름으로 알맞은 실행 함수를 골라 호출한다.
pub fn execute_with_sort(mode: &str) -> String {
    match mode {
        "available" => execute_sorted_by_available(),
        "sku" => execute_sorted_by_sku(),
        _ => execute(None),
    }
}

/// 샘플 저장소에서 특정 SKU가 결품(가용 재고 0) 상태인지 검사한다.
pub fn is_depleted(sku: &str) -> bool {
    sample_store().depleted_skus().iter().any(|s| s == sku)
}

/// 샘플 저장소의 예약률(전체 예약/전체 현재고, %)을 계산한다.
pub fn reservation_ratio_report() -> String {
    let store = sample_store();
    format!("예약률: {}%", store.overall_reservation_ratio_percent())
}

/// 재고 현황 출력 텍스트에서 SKU 이름 목록만 순서대로 뽑아낸다.
pub fn extract_sku_names(output: &str) -> Vec<String> {
    output.lines().filter_map(|l| l.split_once(':').map(|(sku, _)| sku.to_string())).collect()
}

/// 샘플 저장소에서 가용 재고가 가장 큰 SKU를 조회한다.
pub fn most_available_sku() -> Option<String> {
    let store = sample_store();
    inv_store::memory::most_available(&store).map(|s| s.sku)
}

/// 샘플 저장소에 지정된 모든 SKU가 존재하는지 검사한다(사전 점검용).
pub fn has_all_skus(skus: &[String]) -> bool {
    inv_store::memory::has_all_skus(&sample_store(), skus)
}
