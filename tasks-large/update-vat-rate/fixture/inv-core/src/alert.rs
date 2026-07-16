//! 재고 경보(alert) 생성 헬퍼.
//!
//! `rules::mod`에 경보 등급을 계산하는 순수 함수(`alert_level_for` 등)가
//! 있고, 이 파일은 그 결과를 실제 `StockAlert` 레코드로 조립하는 역할을
//! 맡는다(판정 로직과 레코드 조립을 분리).

use crate::rules::alert_level_for;

/// 경보 한 건.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockAlert {
    pub sku: String,
    pub level: u8,
    pub raised_at_epoch: i64,
}

impl StockAlert {
    pub fn is_critical(&self) -> bool {
        self.level >= 2
    }
}

/// 공급일수 기반으로 경보를 생성한다. 정상(level 0)이면 `None`.
pub fn raise_alert_if_needed(sku: &str, days_of_supply: u32, warn_days: u32, epoch: i64) -> Option<StockAlert> {
    let level = alert_level_for(days_of_supply, warn_days);
    if level == 0 {
        None
    } else {
        Some(StockAlert { sku: sku.to_string(), level, raised_at_epoch: epoch })
    }
}

/// 경보 목록 중 긴급(critical) 등급만 걸러낸다.
pub fn critical_alerts(alerts: &[StockAlert]) -> Vec<StockAlert> {
    alerts.iter().filter(|a| a.is_critical()).cloned().collect()
}

/// 특정 SKU에 대한 경보가 이미 존재하는지 확인한다(중복 경보 방지용).
pub fn has_alert_for_sku(alerts: &[StockAlert], sku: &str) -> bool {
    alerts.iter().any(|a| a.sku == sku)
}

/// 경보를 등급 내림차순, 동률이면 발생 시각 오름차순으로 정렬한다.
pub fn sort_by_severity(mut alerts: Vec<StockAlert>) -> Vec<StockAlert> {
    alerts.sort_by(|a, b| b.level.cmp(&a.level).then_with(|| a.raised_at_epoch.cmp(&b.raised_at_epoch)));
    alerts
}

/// 경보 목록을 SKU별로 최신 1건만 남기고 압축한다(중복 경보 정리).
pub fn dedup_latest_per_sku(alerts: &[StockAlert]) -> Vec<StockAlert> {
    let mut skus: Vec<String> = alerts.iter().map(|a| a.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus.into_iter()
        .filter_map(|sku| alerts.iter().filter(|a| a.sku == sku).max_by_key(|a| a.raised_at_epoch).cloned())
        .collect()
}

/// 경보 개수를 등급별로 센다: (정상 외 경고, 긴급) 튜플.
pub fn count_by_severity(alerts: &[StockAlert]) -> (usize, usize) {
    let warning = alerts.iter().filter(|a| a.level == 1).count();
    let critical = alerts.iter().filter(|a| a.level >= 2).count();
    (warning, critical)
}

/// 특정 시간 구간 안에 발생한 경보만 걸러낸다.
pub fn alerts_in_range(alerts: &[StockAlert], from_epoch: i64, to_epoch: i64) -> Vec<StockAlert> {
    alerts.iter().filter(|a| a.raised_at_epoch >= from_epoch && a.raised_at_epoch <= to_epoch).cloned().collect()
}

/// 경보 목록에 등장하는 고유 SKU 목록(정렬됨)을 반환한다.
pub fn affected_skus(alerts: &[StockAlert]) -> Vec<String> {
    let mut skus: Vec<String> = alerts.iter().map(|a| a.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus
}

/// 두 경보 목록을 병합한다(단순 연결 후 시각순 정렬).
pub fn merge_alerts(a: &[StockAlert], b: &[StockAlert]) -> Vec<StockAlert> {
    let mut merged: Vec<StockAlert> = a.iter().chain(b.iter()).cloned().collect();
    merged.sort_by_key(|al| al.raised_at_epoch);
    merged
}

/// 경보가 하나도 없는지(정상 상태) 확인하는 편의 함수.
pub fn is_all_clear(alerts: &[StockAlert]) -> bool {
    alerts.is_empty()
}

/// 특정 SKU의 경보 이력 중 가장 심각했던 등급을 찾는다.
pub fn worst_level_for_sku(alerts: &[StockAlert], sku: &str) -> Option<u8> {
    alerts.iter().filter(|a| a.sku == sku).map(|a| a.level).max()
}

/// 경보 등급을 사람이 읽을 수 있는 라벨로 변환한다.
pub fn level_label(level: u8) -> &'static str {
    match level {
        0 => "정상",
        1 => "경고",
        _ => "긴급",
    }
}
