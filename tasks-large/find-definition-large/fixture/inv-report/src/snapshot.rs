//! 특정 시점의 보고서 상태를 남겨두는 스냅샷 헬퍼.
//!
//! 배치를 반복 실행하며 결과가 어떻게 바뀌는지 추적하고 싶을 때, 매
//! 실행마다 핵심 숫자 몇 개를 스냅샷으로 남겨 이후 비교에 쓴다. 이 모듈은
//! 스냅샷 저장/조회 로직만 담고, 값 계산 자체는 호출자가 이미 마친
//! 상태로 넘겨받는다.

/// 특정 시점(라벨)의 보고서 스냅샷.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSnapshot {
    pub label: String,
    pub net_krw: i64,
    pub line_count: usize,
}

impl ReportSnapshot {
    pub fn new(label: impl Into<String>, net_krw: i64, line_count: usize) -> Self {
        ReportSnapshot { label: label.into(), net_krw, line_count }
    }
}

/// 스냅샷 목록에서 라벨로 하나를 찾는다.
pub fn find_by_label<'a>(snapshots: &'a [ReportSnapshot], label: &str) -> Option<&'a ReportSnapshot> {
    snapshots.iter().find(|s| s.label == label)
}

/// 두 스냅샷의 순매출 차이를 계산한다.
pub fn net_diff(a: &ReportSnapshot, b: &ReportSnapshot) -> i64 {
    b.net_krw - a.net_krw
}

/// 스냅샷 목록을 순매출 기준 내림차순으로 정렬한다.
pub fn sort_by_net_desc(snapshots: &mut Vec<ReportSnapshot>) {
    snapshots.sort_by(|a, b| b.net_krw.cmp(&a.net_krw));
}

/// 스냅샷 목록 중 순매출이 가장 큰 것을 찾는다.
pub fn best_snapshot(snapshots: &[ReportSnapshot]) -> Option<&ReportSnapshot> {
    snapshots.iter().max_by_key(|s| s.net_krw)
}

/// 스냅샷 목록 중 순매출이 가장 작은 것을 찾는다.
pub fn worst_snapshot(snapshots: &[ReportSnapshot]) -> Option<&ReportSnapshot> {
    snapshots.iter().min_by_key(|s| s.net_krw)
}

/// 최신 스냅샷과 그 이전 스냅샷의 순매출 차이를 계산한다(목록이 시간
/// 순서대로 쌓여 있다고 가정 — 마지막 두 개를 비교).
pub fn latest_delta(snapshots: &[ReportSnapshot]) -> Option<i64> {
    if snapshots.len() < 2 {
        return None;
    }
    let last = &snapshots[snapshots.len() - 1];
    let prev = &snapshots[snapshots.len() - 2];
    Some(net_diff(prev, last))
}

/// 스냅샷 목록을 라벨만 뽑아 순서대로 나열한다(타임라인 표시용).
pub fn labels(snapshots: &[ReportSnapshot]) -> Vec<String> {
    snapshots.iter().map(|s| s.label.clone()).collect()
}

/// 스냅샷 목록의 순매출 시계열만 추출한다.
pub fn net_series(snapshots: &[ReportSnapshot]) -> Vec<i64> {
    snapshots.iter().map(|s| s.net_krw).collect()
}

/// 스냅샷 목록에서 순매출이 이전 스냅샷보다 감소한 지점의 라벨 목록을
/// 찾는다(회귀 감지용).
pub fn regression_labels(snapshots: &[ReportSnapshot]) -> Vec<String> {
    snapshots.windows(2).filter(|w| w[1].net_krw < w[0].net_krw).map(|w| w[1].label.clone()).collect()
}

/// 스냅샷 하나를 사람이 읽는 한 줄로 포맷한다.
pub fn format_snapshot(snapshot: &ReportSnapshot) -> String {
    format!("[{}] 순매출 {}원 (라인 {}건)", snapshot.label, snapshot.net_krw, snapshot.line_count)
}

/// 스냅샷 목록 전체를 여러 줄 텍스트로 포맷한다.
pub fn format_all(snapshots: &[ReportSnapshot]) -> String {
    snapshots.iter().map(format_snapshot).collect::<Vec<_>>().join("\n")
}
