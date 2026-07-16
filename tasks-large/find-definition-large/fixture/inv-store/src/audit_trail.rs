//! 저장소 변경 이력(감사 추적) 기록.
//!
//! 재고가 언제, 누가, 왜 바뀌었는지 남기는 단순 인메모리 로그. 실제
//! 서비스에서는 별도 감사 DB로 나가지만, 이 크레이트 범위에서는 로그
//! 항목 자료구조와 조회 헬퍼만 제공한다.

/// 감사 로그 항목 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub sku: String,
    pub delta: i64,
    pub actor: String,
    pub epoch_secs: i64,
}

/// 감사 로그(추가 전용).
#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

impl AuditLog {
    pub fn new() -> Self {
        AuditLog::default()
    }

    /// 항목을 추가한다.
    pub fn record(&mut self, entry: AuditEntry) {
        self.entries.push(entry);
    }

    /// 전체 항목 개수.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 전체 항목을 시간순으로 반환한다(기록 순서 = 시간 순서라고 가정).
    pub fn all(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// 특정 SKU에 대한 항목만 걸러낸다.
    pub fn for_sku(&self, sku: &str) -> Vec<AuditEntry> {
        self.entries.iter().filter(|e| e.sku == sku).cloned().collect()
    }

    /// 특정 작업자(actor)가 남긴 항목만 걸러낸다.
    pub fn for_actor(&self, actor: &str) -> Vec<AuditEntry> {
        self.entries.iter().filter(|e| e.actor == actor).cloned().collect()
    }

    /// 지정 구간(epoch 초, 양 끝 포함) 안의 항목만 걸러낸다.
    pub fn within_range(&self, from_epoch: i64, to_epoch: i64) -> Vec<AuditEntry> {
        self.entries.iter().filter(|e| e.epoch_secs >= from_epoch && e.epoch_secs <= to_epoch).cloned().collect()
    }

    /// 특정 SKU의 순 변화량 합계.
    pub fn net_delta_for_sku(&self, sku: &str) -> i64 {
        self.for_sku(sku).iter().map(|e| e.delta).sum()
    }

    /// 가장 최근 항목(마지막으로 기록된 것)을 반환한다.
    pub fn latest(&self) -> Option<&AuditEntry> {
        self.entries.last()
    }
}

/// 감사 로그 항목을 사람이 읽는 한 줄로 포맷한다.
pub fn format_entry(entry: &AuditEntry) -> String {
    format!("[{}] {} {:+} by {}", entry.epoch_secs, entry.sku, entry.delta, entry.actor)
}

/// 로그 항목 목록에서 작업자별 변경 건수를 센다(작업자명 -> 건수, 이름 오름차순).
pub fn count_by_actor(entries: &[AuditEntry]) -> Vec<(String, usize)> {
    let mut actors: Vec<String> = entries.iter().map(|e| e.actor.clone()).collect();
    actors.sort();
    actors.dedup();
    actors.into_iter().map(|a| (a.clone(), entries.iter().filter(|e| e.actor == a).count())).collect()
}

/// 로그 항목 중 절대값이 큰(대량 변경) 항목만 걸러낸다.
pub fn large_changes(entries: &[AuditEntry], threshold: i64) -> Vec<AuditEntry> {
    entries.iter().filter(|e| e.delta.abs() >= threshold).cloned().collect()
}

/// 로그 항목을 시간순으로 정렬한 새 목록을 반환한다.
pub fn sorted_by_time(entries: &[AuditEntry]) -> Vec<AuditEntry> {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| e.epoch_secs);
    sorted
}

/// 로그 항목 목록에서 SKU별 마지막 변경 시각을 구한다(SKU -> epoch, SKU
/// 오름차순).
pub fn last_change_per_sku(entries: &[AuditEntry]) -> Vec<(String, i64)> {
    let mut skus: Vec<String> = entries.iter().map(|e| e.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus.into_iter()
        .map(|sku| {
            let last = entries.iter().filter(|e| e.sku == sku).map(|e| e.epoch_secs).max().unwrap_or(0);
            (sku, last)
        })
        .collect()
}

/// 로그 항목 목록에서 증가/감소 건수를 각각 센다: (증가, 감소, 변화없음).
pub fn count_directions(entries: &[AuditEntry]) -> (usize, usize, usize) {
    let increases = entries.iter().filter(|e| e.delta > 0).count();
    let decreases = entries.iter().filter(|e| e.delta < 0).count();
    let unchanged = entries.iter().filter(|e| e.delta == 0).count();
    (increases, decreases, unchanged)
}

/// 로그를 감사용 텍스트(줄마다 한 항목)로 직렬화한다.
pub fn to_text(log: &AuditLog) -> String {
    log.all().iter().map(format_entry).collect::<Vec<_>>().join("\n")
}

/// 로그 항목 목록에서 지정 SKU에 대해 순 변화량이 0이 되는(입고와 출고가
/// 상쇄되는) 구간이 있었는지 검사한다(재고 원상 복구 패턴 탐지용).
pub fn has_net_zero_window(entries: &[AuditEntry], sku: &str, window: usize) -> bool {
    let sku_entries: Vec<&AuditEntry> = entries.iter().filter(|e| e.sku == sku).collect();
    if sku_entries.len() < window {
        return false;
    }
    sku_entries.windows(window).any(|w| w.iter().map(|e| e.delta).sum::<i64>() == 0)
}

/// 두 감사 로그를 시간순으로 병합한다.
pub fn merge_logs(a: &AuditLog, b: &AuditLog) -> AuditLog {
    let mut merged = AuditLog::new();
    for e in a.all() {
        merged.record(e.clone());
    }
    for e in b.all() {
        merged.record(e.clone());
    }
    merged
}

/// 로그에서 특정 시각 이후의 항목만 걸러낸다.
pub fn entries_after(log: &AuditLog, epoch: i64) -> Vec<AuditEntry> {
    log.all().iter().filter(|e| e.epoch_secs > epoch).cloned().collect()
}
