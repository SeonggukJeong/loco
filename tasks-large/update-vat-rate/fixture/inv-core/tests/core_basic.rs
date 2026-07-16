//! inv-core 베이스 테스트.
//!
//! monthly_total/calc_total_v2의 합계값, apply_tax/invoice_total/
//! forecast_projection/DEFAULT_VAT_PERCENT 등 세율 파생값은 설정이나
//! 정책에 따라 바뀔 수 있는 값이라 이 파일에서는 단정하지 않는다. 대신
//! 정렬/검증/임계값 계산 같은 순수 헬퍼 함수의 동작만 검증한다.

use inv_core::ledger::{sort_by_sku, LedgerLine, LineKind};
use inv_core::rules::{restock_threshold, WarehouseGrade};
use inv_core::sku::{is_valid_sku, parse_sku};
use inv_core::warehouse::is_valid_warehouse_code;
use inv_core::inventory::InventorySnapshot;
use inv_core::config::CoreConfig;
use inv_core::util::{camel_to_snake, non_blank};

#[test]
fn ledger_sort_by_sku_orders_ascending() {
    let mut lines = vec![
        LedgerLine::new("SKU-C", LineKind::Sale, 100, 0),
        LedgerLine::new("SKU-A", LineKind::Refund, 50, 0),
        LedgerLine::new("SKU-B", LineKind::Adjustment, 0, 5),
    ];
    sort_by_sku(&mut lines);
    let order: Vec<&str> = lines.iter().map(|l| l.sku.as_str()).collect();
    assert_eq!(order, vec!["SKU-A", "SKU-B", "SKU-C"]);
}

#[test]
fn restock_threshold_local_has_no_buffer() {
    // Local 등급은 리드타임 기간 소비량 그대로가 임계값이다(버퍼 없음).
    let threshold = restock_threshold(10, 5, WarehouseGrade::Local);
    assert_eq!(threshold, 50);
}

#[test]
fn restock_threshold_central_has_largest_buffer() {
    // Central > Regional > Local 순으로 버퍼가 커야 한다(값 자체는 핀 고정
    // 공식에서 유도되므로, 등급 간 대소 관계만 검증한다).
    let central = restock_threshold(10, 5, WarehouseGrade::Central);
    let regional = restock_threshold(10, 5, WarehouseGrade::Regional);
    let local = restock_threshold(10, 5, WarehouseGrade::Local);
    assert!(central > regional);
    assert!(regional > local);
}

#[test]
fn sku_parse_accepts_valid_format() {
    let parsed = parse_sku("EL-000123").expect("should parse");
    assert_eq!(parsed.category, "EL");
    assert_eq!(parsed.serial, "000123");
    assert!(parsed.variant.is_none());
}

#[test]
fn sku_parse_accepts_variant_suffix() {
    let parsed = parse_sku("EL-000123-BLK").expect("should parse");
    assert_eq!(parsed.variant.as_deref(), Some("BLK"));
}

#[test]
fn sku_rejects_bad_format() {
    assert!(!is_valid_sku("bad-sku"));
    assert!(!is_valid_sku("EL-12"));
    assert!(!is_valid_sku(""));
}

#[test]
fn warehouse_code_format_check() {
    assert!(is_valid_warehouse_code("SEL1"));
    assert!(!is_valid_warehouse_code("sel1"));
    assert!(!is_valid_warehouse_code("SE1"));
}

#[test]
fn inventory_snapshot_available_excludes_reserved() {
    let snap = InventorySnapshot::new("EL-000123", 100, 30);
    assert_eq!(snap.available(), 70);
    assert!(!snap.is_depleted());
}

#[test]
fn inventory_snapshot_depleted_when_fully_reserved() {
    let snap = InventorySnapshot::new("EL-000123", 20, 20);
    assert!(snap.is_depleted());
}

#[test]
fn core_config_default_is_valid() {
    let cfg = CoreConfig::default();
    assert!(cfg.is_valid());
}

#[test]
fn core_config_rejects_zero_warehouse_count() {
    let cfg = CoreConfig::with_warehouse_count(0);
    assert!(!cfg.is_valid());
}

#[test]
fn util_non_blank_trims_and_detects_empty() {
    assert_eq!(non_blank("  hello  "), Some("hello"));
    assert_eq!(non_blank("   "), None);
}

#[test]
fn util_camel_to_snake_converts() {
    assert_eq!(camel_to_snake("skuCode"), "sku_code");
    assert_eq!(camel_to_snake("SkuCode"), "sku_code");
}
