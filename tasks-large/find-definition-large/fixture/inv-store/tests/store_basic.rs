//! inv-store 베이스 테스트.

mod support;

use inv_core::inventory::InventorySnapshot;
use inv_store::file::{decode_log, encode_log};
use inv_store::location::normalize_location;
use inv_store::memory::MemoryStore;
use inv_store::movement::apply_movement;
use inv_store::retry::{is_exhausted, RetryState};

#[test]
fn apply_movement_adds_positive_delta() {
    assert_eq!(apply_movement(10, 5), 15);
}

#[test]
fn apply_movement_clamps_at_zero() {
    assert_eq!(apply_movement(3, -10), 0);
}

#[test]
fn normalize_location_trims_uppercases_and_unifies_separators() {
    assert_eq!(normalize_location("  sel1_a01 "), "SEL1-A01");
    assert_eq!(normalize_location("sel1/a01"), "SEL1-A01");
    assert_eq!(normalize_location("sel1  a01"), "SEL1-A01");
}

#[test]
fn retry_state_tracks_attempts_and_exhaustion() {
    let mut state = RetryState::new();
    assert!(state.can_retry());
    state.record_attempt();
    state.record_attempt();
    state.record_attempt();
    assert!(is_exhausted(state.attempts()));
    assert!(!state.can_retry());
}

#[test]
fn memory_store_upsert_and_get() {
    let mut store = MemoryStore::new();
    store.upsert(InventorySnapshot::new("EL-000123", 100, 20));
    let snap = store.get("EL-000123").expect("should exist");
    assert_eq!(snap.available(), 80);
}

#[test]
fn memory_store_apply_delta_uses_movement_logic() {
    let mut store = MemoryStore::new();
    store.upsert(InventorySnapshot::new("EL-000123", 10, 0));
    assert!(store.apply_delta("EL-000123", -3));
    assert_eq!(store.get("EL-000123").unwrap().on_hand, 7);
}

#[test]
fn memory_store_apply_delta_missing_sku_returns_false() {
    let mut store = MemoryStore::new();
    assert!(!store.apply_delta("NOPE", 5));
}

#[test]
fn file_format_round_trips_snapshots() {
    let snapshots = support::sample_snapshots();
    let text = encode_log(&snapshots);
    let decoded = decode_log(&text).expect("should decode");
    assert_eq!(decoded.len(), snapshots.len());
    assert_eq!(decoded[0].sku, snapshots[0].sku);
}

#[test]
fn support_mock_apply_movement_is_independent_of_real_logic() {
    // tests/support 모듈이 실제로 컴파일·링크되는지 확인하는 용도의 테스트.
    // 목의 반환값은 실제 movement::apply_movement 로직과 무관한 고정값이다.
    let mock_result = support::apply_movement(10, 5);
    let real_result = apply_movement(10, 5);
    assert_eq!(mock_result, 42);
    assert_ne!(mock_result, real_result);
}

#[test]
fn support_snapshot_helper_builds_expected_fields() {
    let snap = support::snapshot("EL-000999", 30, 10);
    assert_eq!(snap.sku, "EL-000999");
    assert_eq!(snap.available(), 20);
}

#[test]
fn location_index_tracks_scattered_skus() {
    use inv_store::index::{build_index, scattered_count};
    let index = build_index(&[
        ("EL-000123".to_string(), "SEL1-A01".to_string()),
        ("EL-000123".to_string(), "BSN1-A01".to_string()),
        ("EL-000456".to_string(), "SEL1-A02".to_string()),
    ]);
    assert_eq!(index.sku_count(), 2);
    assert_eq!(scattered_count(&index), 1);
}

#[test]
fn transfer_request_rejects_same_location() {
    use inv_store::transfer::{validate_request, TransferError, TransferRequest};
    let req = TransferRequest {
        sku: "EL-000123".to_string(),
        from_location: "sel1_a01".to_string(),
        to_location: "SEL1 A01".to_string(),
        qty: 5,
    };
    assert_eq!(validate_request(&req), Err(TransferError::SameLocation));
}

#[test]
fn reservation_reserve_and_release_round_trip() {
    use inv_store::reservation::{reserve, release};
    let snapshots = support::sample_snapshots();
    let reserved = reserve(&snapshots, "EL-000456", 10).expect("should reserve");
    assert_eq!(reserved.reserved, 10);
    let released = release(std::slice::from_ref(&reserved), "EL-000456", 10).expect("should release");
    assert_eq!(released.reserved, 0);
}

#[test]
fn snapshot_diff_detects_changes() {
    use inv_store::snapshot::diff;
    let before = vec![support::snapshot("EL-000123", 100, 0)];
    let after = vec![support::snapshot("EL-000123", 80, 0)];
    let diffs = diff(&before, &after);
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].delta(), -20);
}
