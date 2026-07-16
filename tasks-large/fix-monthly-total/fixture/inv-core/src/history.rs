//! 필드 변경 이력(history) 기록 타입과 조회 헬퍼.
//!
//! 재고/가격/등급 등 주요 필드가 바뀔 때마다 남기는 감사용 이력이다.
//! `audit.rs`의 액션 로그와는 다르다 — 이쪽은 "무엇이 무엇으로 바뀌었는지"
//! 필드 단위 diff를 남기고, audit.rs는 "누가 어떤 액션을 했는지"를 남긴다.

/// 필드 변경 한 건을 표현한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeHistoryEntry {
    pub sku: String,
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub changed_at_epoch: i64,
}

impl ChangeHistoryEntry {
    pub fn new(
        sku: impl Into<String>,
        field: impl Into<String>,
        old_value: impl Into<String>,
        new_value: impl Into<String>,
        changed_at_epoch: i64,
    ) -> Self {
        ChangeHistoryEntry {
            sku: sku.into(),
            field: field.into(),
            old_value: old_value.into(),
            new_value: new_value.into(),
            changed_at_epoch,
        }
    }

    /// 실제로 값이 바뀐 변경인지(같은 값으로의 "변경"은 무의미) 확인한다.
    pub fn is_effective_change(&self) -> bool {
        self.old_value != self.new_value
    }
}

/// 새 변경 이력을 기록한다(무변경이면 기록하지 않고 `None`을 반환).
pub fn record_change(
    log: &mut Vec<ChangeHistoryEntry>,
    sku: &str,
    field: &str,
    old_value: &str,
    new_value: &str,
    epoch: i64,
) -> Option<()> {
    if old_value == new_value {
        return None;
    }
    log.push(ChangeHistoryEntry::new(sku, field, old_value, new_value, epoch));
    Some(())
}

/// 특정 SKU의 변경 이력만 걸러낸다.
pub fn filter_by_sku<'a>(log: &'a [ChangeHistoryEntry], sku: &str) -> Vec<&'a ChangeHistoryEntry> {
    log.iter().filter(|e| e.sku == sku).collect()
}

/// 특정 필드의 변경 이력만 걸러낸다.
pub fn filter_by_field<'a>(log: &'a [ChangeHistoryEntry], field: &str) -> Vec<&'a ChangeHistoryEntry> {
    log.iter().filter(|e| e.field == field).collect()
}

/// 특정 SKU·필드 조합의 가장 최근 변경을 찾는다(시각 내림차순 1건).
pub fn latest_change<'a>(log: &'a [ChangeHistoryEntry], sku: &str, field: &str) -> Option<&'a ChangeHistoryEntry> {
    log.iter()
        .filter(|e| e.sku == sku && e.field == field)
        .max_by_key(|e| e.changed_at_epoch)
}

/// 지정된 시간 구간(양 끝 포함) 안의 변경 이력만 걸러낸다.
pub fn changes_in_range(log: &[ChangeHistoryEntry], from_epoch: i64, to_epoch: i64) -> Vec<ChangeHistoryEntry> {
    log.iter()
        .filter(|e| e.changed_at_epoch >= from_epoch && e.changed_at_epoch <= to_epoch)
        .cloned()
        .collect()
}

/// 특정 SKU의 변경 횟수를 센다.
pub fn change_count_for_sku(log: &[ChangeHistoryEntry], sku: &str) -> usize {
    log.iter().filter(|e| e.sku == sku).count()
}

/// 변경 이력을 시각 오름차순으로 정렬한다.
pub fn sort_chronological(log: &mut Vec<ChangeHistoryEntry>) {
    log.sort_by_key(|e| e.changed_at_epoch);
}

/// 가장 자주 변경된 SKU를 찾는다(동률이면 SKU 문자열 오름차순으로 첫 항목).
pub fn most_changed_sku(log: &[ChangeHistoryEntry]) -> Option<String> {
    let mut skus: Vec<String> = log.iter().map(|e| e.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus.into_iter().max_by_key(|s| change_count_for_sku(log, s))
}

/// 특정 필드가 지정된 시간 창(window) 안에 변경된 적이 있는지 확인한다.
///
/// 짧은 시간 안에 반복 수정되는 필드를 찾아 데이터 품질 이슈를 감지하는
/// 용도로 쓴다.
pub fn was_recently_changed(log: &[ChangeHistoryEntry], sku: &str, field: &str, now_epoch: i64, window_secs: i64) -> bool {
    log.iter()
        .any(|e| e.sku == sku && e.field == field && (now_epoch - e.changed_at_epoch) <= window_secs)
}

/// 특정 SKU의 변경 이력을 필드별로 그룹핑한다(필드명 -> 변경 건수 목록).
pub fn field_change_counts(log: &[ChangeHistoryEntry], sku: &str) -> Vec<(String, usize)> {
    let mut fields: Vec<String> = log.iter().filter(|e| e.sku == sku).map(|e| e.field.clone()).collect();
    fields.sort();
    fields.dedup();
    fields
        .into_iter()
        .map(|field| {
            let count = log.iter().filter(|e| e.sku == sku && e.field == field).count();
            (field, count)
        })
        .collect()
}

/// 이력 목록을 시간 역순(최신 먼저)으로 정렬한 사본을 반환한다.
pub fn newest_first(log: &[ChangeHistoryEntry]) -> Vec<ChangeHistoryEntry> {
    let mut copy = log.to_vec();
    copy.sort_by(|a, b| b.changed_at_epoch.cmp(&a.changed_at_epoch));
    copy
}

/// 이력에서 특정 값으로 변경된 적이 있는지(new_value 기준) 확인한다.
pub fn has_ever_been_set_to(log: &[ChangeHistoryEntry], sku: &str, field: &str, value: &str) -> bool {
    log.iter().any(|e| e.sku == sku && e.field == field && e.new_value == value)
}

/// 이력을 병합한다: 두 로그를 시간순으로 합치고 정렬해 반환한다.
pub fn merge_logs(a: &[ChangeHistoryEntry], b: &[ChangeHistoryEntry]) -> Vec<ChangeHistoryEntry> {
    let mut merged: Vec<ChangeHistoryEntry> = a.iter().chain(b.iter()).cloned().collect();
    sort_chronological(&mut merged);
    merged
}

/// 특정 기간 동안의 변경 건수를 센다(빠른 조회용, `changes_in_range`의
/// 개수 버전).
pub fn change_count_in_range(log: &[ChangeHistoryEntry], from_epoch: i64, to_epoch: i64) -> usize {
    changes_in_range(log, from_epoch, to_epoch).len()
}

/// 이력에 등장하는 고유 필드명 목록(정렬됨)을 반환한다.
pub fn distinct_fields(log: &[ChangeHistoryEntry]) -> Vec<String> {
    let mut fields: Vec<String> = log.iter().map(|e| e.field.clone()).collect();
    fields.sort();
    fields.dedup();
    fields
}
