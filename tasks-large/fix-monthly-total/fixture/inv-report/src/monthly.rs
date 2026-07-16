//! 월간 정산 보고서 집계 로직(v2).
//!
//! `totals` 모듈의 for-루프 기반 v1 계산을 fold 스타일로 다시 쓰고, 월
//! 단위 정산에 특화된 진입점(`monthly_total`)을 추가한 두 번째 버전이다.
//! 신규 조립 계층(`report_v2`)은 이 모듈의 함수만 사용한다.

use inv_core::ledger::{LedgerLine, LineKind};

/// 월간 정산 보고서 합계. 반품은 이 함수에 들어오기 전 단계에서 이미
/// 차감되어 들어오므로, 여기서는 원장 라인을 종류별로 그대로 누적하기만
/// 하면 된다.
pub fn calc_total_v2(lines: &[LedgerLine]) -> i64 {
    lines.iter().fold(0i64, |acc, line| match line.kind {
        LineKind::Sale => acc - line.amount_krw,
        LineKind::Refund => acc - line.amount_krw,
        LineKind::Adjustment => acc + line.adjustment_krw(),
    })
}

pub fn monthly_total(lines: &[LedgerLine]) -> i64 { calc_total_v2(lines) }

/// 월(1~12)이 유효한 범위인지 검사한다.
pub fn is_valid_month(month: u32) -> bool {
    (1..=12).contains(&month)
}

/// 연-월 식별자를 사람이 읽는 라벨로 포맷한다("2024-11").
pub fn format_month_label(year: u32, month: u32) -> String {
    format!("{year:04}-{month:02}")
}

/// 종류별로 나눈 월간 합계를 (판매, 환불, 조정) 튜플로 계산한다. 세
/// 값을 각각 fold 규칙으로 계산하므로 합쳐도 `calc_total_v2`와 동일하다.
pub fn monthly_total_by_kind(lines: &[LedgerLine]) -> (i64, i64, i64) {
    let sale = lines.iter().filter(|l| matches!(l.kind, LineKind::Sale)).map(|l| l.amount_krw).sum();
    let refund = lines.iter().filter(|l| matches!(l.kind, LineKind::Refund)).map(|l| l.amount_krw).sum();
    let adjustment = lines.iter().filter(|l| matches!(l.kind, LineKind::Adjustment)).map(|l| l.adjustment_krw()).sum();
    (sale, refund, adjustment)
}

/// 이번 달과 지난달의 월간 합계 차이(전월 대비 증감액)를 계산한다.
pub fn month_over_month_delta(current_month_lines: &[LedgerLine], prev_month_lines: &[LedgerLine]) -> i64 {
    monthly_total(current_month_lines) - monthly_total(prev_month_lines)
}

/// 전월 대비 증감률(%)을 계산한다. 전월 합계가 0이면 0을 반환한다(0으로
/// 나누기 방지).
pub fn month_over_month_percent(current_month_lines: &[LedgerLine], prev_month_lines: &[LedgerLine]) -> i64 {
    let prev = monthly_total(prev_month_lines);
    if prev == 0 {
        return 0;
    }
    let delta = month_over_month_delta(current_month_lines, prev_month_lines);
    (delta.saturating_mul(100)) / prev
}

/// 월간 원장에 등장한 고유 SKU 수로 나눈 SKU당 평균 정산액을 계산한다.
pub fn monthly_average_per_sku(lines: &[LedgerLine]) -> f64 {
    let mut skus: Vec<&str> = lines.iter().map(|l| l.sku.as_str()).collect();
    skus.sort();
    skus.dedup();
    if skus.is_empty() {
        0.0
    } else {
        monthly_total(lines) as f64 / skus.len() as f64
    }
}

/// 월간 원장 라인 개수를 센다(월별 거래량 지표로 쓰인다).
pub fn monthly_line_count(lines: &[LedgerLine]) -> usize {
    lines.len()
}

/// 월간 조정 라인 중 절대값이 가장 큰 조정 금액을 찾는다.
pub fn largest_monthly_adjustment(lines: &[LedgerLine]) -> Option<i64> {
    lines
        .iter()
        .filter(|l| matches!(l.kind, LineKind::Adjustment))
        .map(|l| l.adjustment_krw())
        .max_by_key(|v| v.abs())
}

/// 월간 원장에서 환불 라인이 차지하는 비율(%)을 라인 개수 기준으로 계산한다.
pub fn monthly_refund_line_ratio_percent(lines: &[LedgerLine]) -> u32 {
    if lines.is_empty() {
        return 0;
    }
    let refund_count = lines.iter().filter(|l| matches!(l.kind, LineKind::Refund)).count();
    ((refund_count * 100) / lines.len()) as u32
}

/// 연-월 식별자를 다루는 최소 구조체(정산 기간 표기용).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonthId {
    pub year: u32,
    pub month: u32,
}

impl MonthId {
    /// 새 연-월 식별자를 만든다. 월이 범위를 벗어나면 1로 clamp한다.
    pub fn new(year: u32, month: u32) -> Self {
        MonthId { year, month: if is_valid_month(month) { month } else { 1 } }
    }

    /// 다음 달 식별자를 계산한다(12월 다음은 다음 해 1월).
    pub fn next(self) -> MonthId {
        if self.month == 12 {
            MonthId { year: self.year + 1, month: 1 }
        } else {
            MonthId { year: self.year, month: self.month + 1 }
        }
    }

    /// 라벨 문자열로 포맷한다.
    pub fn label(self) -> String {
        format_month_label(self.year, self.month)
    }
}

/// 여러 달의 월간 합계를 순서대로 계산해 (MonthId, 합계) 목록으로 만든다.
pub fn monthly_totals_series(months: &[(MonthId, Vec<LedgerLine>)]) -> Vec<(MonthId, i64)> {
    months.iter().map(|(id, lines)| (*id, monthly_total(lines))).collect()
}

/// 월간 합계 시계열에서 값이 가장 컸던 달을 찾는다.
pub fn peak_month(series: &[(MonthId, i64)]) -> Option<MonthId> {
    series.iter().max_by_key(|(_, total)| *total).map(|(id, _)| *id)
}

/// 월간 합계 시계열의 평균을 계산한다(빈 시계열은 0.0).
pub fn average_monthly_total(series: &[(MonthId, i64)]) -> f64 {
    if series.is_empty() {
        0.0
    } else {
        series.iter().map(|(_, total)| *total).sum::<i64>() as f64 / series.len() as f64
    }
}

/// 월간 합계 시계열에서 값이 가장 작았던(최저) 달을 찾는다.
pub fn trough_month(series: &[(MonthId, i64)]) -> Option<MonthId> {
    series.iter().min_by_key(|(_, total)| *total).map(|(id, _)| *id)
}

/// 특정 연-월의 월간 합계를 시계열에서 조회한다.
pub fn total_for_month(series: &[(MonthId, i64)], target: MonthId) -> Option<i64> {
    series.iter().find(|(id, _)| *id == target).map(|(_, total)| *total)
}

/// 두 연-월 식별자 사이에 몇 개월이 지났는지 계산한다(같은 해는 월 차,
/// 해가 다르면 연도 차를 반영).
pub fn months_between(a: MonthId, b: MonthId) -> i64 {
    let a_index = a.year as i64 * 12 + a.month as i64;
    let b_index = b.year as i64 * 12 + b.month as i64;
    (b_index - a_index).abs()
}

/// 연-월 식별자가 지정된 범위(양 끝 포함) 안에 있는지 검사한다.
pub fn is_within_range(target: MonthId, start: MonthId, end: MonthId) -> bool {
    target >= start && target <= end
}

/// 월간 원장에서 조정 라인이 전체 라인 대비 차지하는 비율(%)을 계산한다.
pub fn monthly_adjustment_line_ratio_percent(lines: &[LedgerLine]) -> u32 {
    if lines.is_empty() {
        return 0;
    }
    let count = lines.iter().filter(|l| matches!(l.kind, LineKind::Adjustment)).count();
    ((count * 100) / lines.len()) as u32
}

/// 월간 원장의 판매 라인 개수만 센다.
pub fn monthly_sale_line_count(lines: &[LedgerLine]) -> usize {
    lines.iter().filter(|l| matches!(l.kind, LineKind::Sale)).count()
}

/// 연속된 두 달의 라벨을 하이픈으로 이어붙여 기간 라벨을 만든다
/// ("2024-11~2024-12" 형태).
pub fn format_month_range_label(start: MonthId, end: MonthId) -> String {
    format!("{}~{}", start.label(), end.label())
}

/// 월간 합계 시계열이 연속(빠진 달 없이)인지 검사한다.
pub fn is_contiguous_series(series: &[(MonthId, i64)]) -> bool {
    series.windows(2).all(|w| w[0].0.next() == w[1].0)
}

/// 특정 연도에 속한 월간 합계만 걸러낸다.
pub fn totals_for_year(series: &[(MonthId, i64)], year: u32) -> Vec<(MonthId, i64)> {
    series.iter().filter(|(id, _)| id.year == year).cloned().collect()
}
