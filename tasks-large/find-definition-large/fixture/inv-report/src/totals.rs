//! 원장 라인 합계 계산의 초기 버전(v1).
//!
//! 판매/환불/조정 라인을 순회하며 순매출을 구하는, 이 크레이트에서 가장
//! 먼저 작성된 집계 로직이다. 이후 `monthly` 모듈에 fold 기반의 두 번째
//! 버전이 추가되었지만, 이 v1 계산도 일부 조립 경로에서 여전히 쓰인다.

use inv_core::ledger::{LedgerLine, LineKind};

// FIXME: 반품 부호 처리가 의심스럽다 — 확인 필요 (2024-11)
pub fn calc_total(lines: &[LedgerLine]) -> i64 {
    let mut total = 0i64;
    for line in lines {
        match line.kind {
            LineKind::Sale => total += line.amount_krw,
            LineKind::Refund => total -= line.amount_krw,
            LineKind::Adjustment => total += line.adjustment_krw(),
        }
    }
    total
}

/// v1 규칙과 동일한 부호로 부분 합계를 낼 때 이 파일의 다른 헬퍼들이
/// 재사용하는 내부 조합 함수. `calc_total`을 직접 다시 호출하는 대신,
/// 이미 계산해 둔 판매/환불/조정 부분 합계(아래 세 함수)를 조합한다.
fn combined_v1_style(lines: &[LedgerLine]) -> i64 {
    total_sales_only(lines) - total_refunds_magnitude(lines) + total_adjustments_only(lines)
}

/// 판매(Sale) 라인만 골라 합계를 구한다(환불/조정은 무시).
pub fn total_sales_only(lines: &[LedgerLine]) -> i64 {
    lines.iter().filter(|l| matches!(l.kind, LineKind::Sale)).map(|l| l.amount_krw).sum()
}

/// 환불(Refund) 라인의 금액 합계를 절대값(양수)으로 구한다.
///
/// `calc_total`은 환불을 차감 부호로 반영하지만, 이 함수는 "환불이 총
/// 얼마나 발생했는가"를 보고할 때 쓰는 크기(magnitude) 값이다.
pub fn total_refunds_magnitude(lines: &[LedgerLine]) -> i64 {
    lines.iter().filter(|l| matches!(l.kind, LineKind::Refund)).map(|l| l.amount_krw).sum()
}

/// 조정(Adjustment) 라인의 조정 금액 합계를 구한다.
pub fn total_adjustments_only(lines: &[LedgerLine]) -> i64 {
    lines.iter().filter(|l| matches!(l.kind, LineKind::Adjustment)).map(|l| l.adjustment_krw()).sum()
}

/// 특정 SKU에 속한 라인만 걸러 v1 합계를 계산한다.
pub fn total_for_sku(lines: &[LedgerLine], sku: &str) -> i64 {
    let filtered: Vec<LedgerLine> = lines.iter().filter(|l| l.sku == sku).cloned().collect();
    combined_v1_style(&filtered)
}

/// 특정 SKU를 제외한 나머지 라인으로 v1 합계를 계산한다.
pub fn total_excluding_sku(lines: &[LedgerLine], sku: &str) -> i64 {
    let filtered: Vec<LedgerLine> = lines.iter().filter(|l| l.sku != sku).cloned().collect();
    combined_v1_style(&filtered)
}

/// 합계 계산에 실제로 기여하는(금액이 0이 아닌) 라인 개수를 센다.
pub fn contributing_line_count(lines: &[LedgerLine]) -> usize {
    lines.iter().filter(|l| l.amount_krw != 0 || l.adjustment_krw() != 0).count()
}

/// 라인 목록의 평균 금액(판매/환불 라인 기준, 조정 제외)을 계산한다.
///
/// 대상 라인이 하나도 없으면 0.0을 반환한다(0으로 나누기 방지).
pub fn average_line_amount(lines: &[LedgerLine]) -> f64 {
    let relevant: Vec<i64> =
        lines.iter().filter(|l| !matches!(l.kind, LineKind::Adjustment)).map(|l| l.amount_krw).collect();
    if relevant.is_empty() {
        0.0
    } else {
        relevant.iter().sum::<i64>() as f64 / relevant.len() as f64
    }
}

/// 판매 라인 중 금액이 가장 큰 라인의 금액을 찾는다(없으면 `None`).
pub fn largest_sale_amount(lines: &[LedgerLine]) -> Option<i64> {
    lines.iter().filter(|l| matches!(l.kind, LineKind::Sale)).map(|l| l.amount_krw).max()
}

/// 원장에 환불 라인이 하나라도 있는지 확인한다.
pub fn has_any_refund(lines: &[LedgerLine]) -> bool {
    lines.iter().any(|l| matches!(l.kind, LineKind::Refund))
}

/// v1 규칙을 그대로 적용하되, 라인을 하나씩 누적해 가는 중간 합계를
/// 순서대로 기록한다(추이 그래프/디버그 출력용).
pub fn running_totals(lines: &[LedgerLine]) -> Vec<i64> {
    let mut running = 0i64;
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        running = combined_v1_style(std::slice::from_ref(line)) + running;
        out.push(running);
    }
    out
}

/// 금액(`amount_krw`)이 지정 구간 [min, max] 안에 있는 라인만 골라 v1
/// 합계를 계산한다(고액/저액 구간별 부분 합계 조회용).
pub fn total_within_amount_range(lines: &[LedgerLine], min_krw: i64, max_krw: i64) -> i64 {
    let filtered: Vec<LedgerLine> =
        lines.iter().filter(|l| l.amount_krw >= min_krw && l.amount_krw <= max_krw).cloned().collect();
    combined_v1_style(&filtered)
}

/// 환불 금액이 판매 금액 대비 몇 퍼센트인지 계산한다(판매가 0이면 0).
pub fn refund_ratio_percent(lines: &[LedgerLine]) -> u32 {
    let sales = total_sales_only(lines);
    if sales <= 0 {
        return 0;
    }
    let refunds = total_refunds_magnitude(lines);
    ((refunds.saturating_mul(100)) / sales).clamp(0, 100) as u32
}

/// 여러 SKU에 대한 v1 합계를 한 번에 계산해 (SKU, 합계) 목록으로 돌려준다
/// (SKU 오름차순).
pub fn totals_by_sku(lines: &[LedgerLine]) -> Vec<(String, i64)> {
    let mut skus: Vec<String> = lines.iter().map(|l| l.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus.into_iter().map(|sku| (sku.clone(), total_for_sku(lines, &sku))).collect()
}

/// v1 합계가 음수인지(환불이 판매+조정을 초과했는지) 검사한다.
pub fn is_net_negative(lines: &[LedgerLine]) -> bool {
    combined_v1_style(lines) < 0
}

/// 여러 SKU에 속한 라인만 걸러 v1 합계를 계산한다.
pub fn total_for_multiple_skus(lines: &[LedgerLine], skus: &[String]) -> i64 {
    let filtered: Vec<LedgerLine> = lines.iter().filter(|l| skus.iter().any(|s| s == &l.sku)).cloned().collect();
    combined_v1_style(&filtered)
}

/// 특정 SKU에 속한 라인 개수를 센다.
pub fn count_lines_for_sku(lines: &[LedgerLine], sku: &str) -> usize {
    lines.iter().filter(|l| l.sku == sku).count()
}

/// 라인 목록 중 금액이 가장 작은 라인의 금액을 찾는다(종류 무관).
pub fn min_line_amount(lines: &[LedgerLine]) -> Option<i64> {
    lines.iter().map(|l| l.amount_krw).min()
}

/// 라인 목록 중 금액이 가장 큰 라인의 금액을 찾는다(종류 무관, 판매만
/// 보는 `largest_sale_amount`와 달리 환불/조정 라인도 포함한다).
pub fn max_line_amount(lines: &[LedgerLine]) -> Option<i64> {
    lines.iter().map(|l| l.amount_krw).max()
}

/// 조정(Adjustment) 라인의 조정 금액 절대값 합계를 구한다(부호 무시,
/// 조정이 얼마나 활발했는지의 크기 지표).
pub fn total_adjustment_magnitude(lines: &[LedgerLine]) -> i64 {
    lines.iter().filter(|l| matches!(l.kind, LineKind::Adjustment)).map(|l| l.adjustment_krw().abs()).sum()
}

/// 조정 라인을 제외하고 판매-환불만으로 계산한 순매출(v1 부호 규칙 적용).
pub fn sales_minus_refunds_only(lines: &[LedgerLine]) -> i64 {
    let filtered: Vec<LedgerLine> =
        lines.iter().filter(|l| !matches!(l.kind, LineKind::Adjustment)).cloned().collect();
    combined_v1_style(&filtered)
}

/// 원장에 조정 라인이 하나라도 있는지 확인한다.
pub fn has_any_adjustment(lines: &[LedgerLine]) -> bool {
    lines.iter().any(|l| matches!(l.kind, LineKind::Adjustment))
}

/// 금액이 정확히 0인 라인 개수를 센다(수량 미확정/보류 라인 지표).
pub fn zero_amount_line_count(lines: &[LedgerLine]) -> usize {
    lines.iter().filter(|l| l.amount_krw == 0 && l.adjustment_krw() == 0).count()
}

/// 판매 라인이 전체 라인에서 차지하는 비율(%)을 계산한다.
pub fn percent_lines_are_sale(lines: &[LedgerLine]) -> u32 {
    if lines.is_empty() {
        return 0;
    }
    let sale_count = lines.iter().filter(|l| matches!(l.kind, LineKind::Sale)).count();
    ((sale_count * 100) / lines.len()) as u32
}

/// v1 합계가 음수이면 0으로 잘라 보고한다(표시 전 방어적 clamp — 실제
/// 계산 결과 자체는 바꾸지 않고 표시용 값만 만든다).
pub fn clamp_total_non_negative(lines: &[LedgerLine]) -> i64 {
    combined_v1_style(lines).max(0)
}

/// 원장 라인 목록을 v1 합계 기준으로 두 그룹(임계값 이상/미만)으로 나눈다.
pub fn partition_by_total_threshold(
    batches: &[Vec<LedgerLine>],
    threshold_krw: i64,
) -> (Vec<Vec<LedgerLine>>, Vec<Vec<LedgerLine>>) {
    let mut above = Vec::new();
    let mut below = Vec::new();
    for batch in batches {
        if combined_v1_style(batch) >= threshold_krw {
            above.push(batch.clone());
        } else {
            below.push(batch.clone());
        }
    }
    (above, below)
}
