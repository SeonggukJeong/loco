//! 저장소 상태의 시점별 스냅샷 비교(diff).
//!
//! 배치 처리 전후로 저장소 상태를 통째로 캡처해 두면, 이 모듈로 "무엇이
//! 바뀌었는지"를 SKU 단위로 비교할 수 있다. 감사/디버깅 시 특히 유용하다.

use inv_core::inventory::InventorySnapshot;

/// SKU 하나의 변화를 나타내는 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotDiff {
    pub sku: String,
    pub before_on_hand: u32,
    pub after_on_hand: u32,
}

impl SnapshotDiff {
    /// 순 변화량(증가는 양수).
    pub fn delta(&self) -> i64 {
        self.after_on_hand as i64 - self.before_on_hand as i64
    }

    /// 실제로 변화가 있었는지.
    pub fn changed(&self) -> bool {
        self.before_on_hand != self.after_on_hand
    }
}

/// 두 시점의 스냅샷 목록을 비교해 SKU별 변화 목록을 만든다. 한쪽에만
/// 있던 SKU는 없던 쪽을 0으로 취급한다(신규 등록/완전 제거로 해석).
pub fn diff(before: &[InventorySnapshot], after: &[InventorySnapshot]) -> Vec<SnapshotDiff> {
    let mut skus: Vec<String> =
        before.iter().map(|s| s.sku.clone()).chain(after.iter().map(|s| s.sku.clone())).collect();
    skus.sort();
    skus.dedup();

    skus.into_iter()
        .map(|sku| {
            let before_on_hand = before.iter().find(|s| s.sku == sku).map(|s| s.on_hand).unwrap_or(0);
            let after_on_hand = after.iter().find(|s| s.sku == sku).map(|s| s.on_hand).unwrap_or(0);
            SnapshotDiff { sku, before_on_hand, after_on_hand }
        })
        .collect()
}

/// diff 목록에서 실제로 변화가 있었던 항목만 걸러낸다.
pub fn changed_only(diffs: &[SnapshotDiff]) -> Vec<SnapshotDiff> {
    diffs.iter().filter(|d| d.changed()).cloned().collect()
}

/// diff 목록에서 증가한 항목만 걸러낸다.
pub fn increases(diffs: &[SnapshotDiff]) -> Vec<SnapshotDiff> {
    diffs.iter().filter(|d| d.delta() > 0).cloned().collect()
}

/// diff 목록에서 감소한 항목만 걸러낸다.
pub fn decreases(diffs: &[SnapshotDiff]) -> Vec<SnapshotDiff> {
    diffs.iter().filter(|d| d.delta() < 0).cloned().collect()
}

/// diff 목록의 순 변화량 합계.
pub fn total_delta(diffs: &[SnapshotDiff]) -> i64 {
    diffs.iter().map(|d| d.delta()).sum()
}

/// diff 목록을 변화량 절댓값 내림차순으로 정렬한다(가장 크게 바뀐 SKU가
/// 먼저 오도록 — 감사 리포트의 상위 N개 노출용).
pub fn sort_by_magnitude_desc(diffs: &mut Vec<SnapshotDiff>) {
    diffs.sort_by(|a, b| b.delta().abs().cmp(&a.delta().abs()));
}

/// 두 스냅샷 목록이 완전히 동일한지(diff가 비어있는지) 검사한다.
pub fn is_identical(before: &[InventorySnapshot], after: &[InventorySnapshot]) -> bool {
    changed_only(&diff(before, after)).is_empty()
}

/// diff 목록 중 신규로 등장한(이전에 없던) SKU만 걸러낸다.
pub fn newly_added(diffs: &[SnapshotDiff]) -> Vec<SnapshotDiff> {
    diffs.iter().filter(|d| d.before_on_hand == 0 && d.after_on_hand > 0).cloned().collect()
}

/// diff 목록 중 완전히 소진된(이후 0이 된) SKU만 걸러낸다.
pub fn newly_depleted(diffs: &[SnapshotDiff]) -> Vec<SnapshotDiff> {
    diffs.iter().filter(|d| d.before_on_hand > 0 && d.after_on_hand == 0).cloned().collect()
}

/// diff 목록을 사람이 읽는 한 줄 요약 문자열 목록으로 바꾼다.
pub fn describe_diffs(diffs: &[SnapshotDiff]) -> Vec<String> {
    diffs.iter().map(|d| format!("{}: {} -> {} ({:+})", d.sku, d.before_on_hand, d.after_on_hand, d.delta())).collect()
}

/// 스냅샷 목록의 총 현재고 합계를 구한다.
pub fn total_on_hand(snapshots: &[InventorySnapshot]) -> u32 {
    snapshots.iter().map(|s| s.on_hand).sum()
}

/// 세 시점(전전/전/현재)의 스냅샷을 연쇄 비교해 두 구간의 diff를 모두 계산한다.
pub fn diff_chain(
    a: &[InventorySnapshot],
    b: &[InventorySnapshot],
    c: &[InventorySnapshot],
) -> (Vec<SnapshotDiff>, Vec<SnapshotDiff>) {
    (diff(a, b), diff(b, c))
}

/// diff 목록에서 특정 SKU에 대한 항목만 찾는다.
pub fn diff_for_sku<'a>(diffs: &'a [SnapshotDiff], sku: &str) -> Option<&'a SnapshotDiff> {
    diffs.iter().find(|d| d.sku == sku)
}

/// diff 목록을 SKU 오름차순으로 정렬한다(보고서 출력 순서 고정용).
pub fn sort_by_sku(diffs: &mut Vec<SnapshotDiff>) {
    diffs.sort_by(|a, b| a.sku.cmp(&b.sku));
}

/// diff 목록에서 변화량 절댓값이 임계값 이상인 항목만 걸러낸다(경보
/// 대상 축소용).
pub fn significant_changes(diffs: &[SnapshotDiff], threshold: i64) -> Vec<SnapshotDiff> {
    diffs.iter().filter(|d| d.delta().abs() >= threshold).cloned().collect()
}

/// diff 목록의 증가/감소 건수 비율을 (증가 비율%, 감소 비율%)로 계산한다.
pub fn direction_ratio_percent(diffs: &[SnapshotDiff]) -> (u32, u32) {
    let changed = changed_only(diffs);
    if changed.is_empty() {
        return (0, 0);
    }
    let total = changed.len();
    let up = changed.iter().filter(|d| d.delta() > 0).count();
    let down = changed.iter().filter(|d| d.delta() < 0).count();
    ((up * 100 / total) as u32, (down * 100 / total) as u32)
}
