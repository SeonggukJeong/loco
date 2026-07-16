//! 원장 데이터에 대한 고수준 요약 통계.
//!
//! 보고서를 조립하기 전에 "이 원장에 뭐가 들어있는지" 빠르게 파악하는
//! 용도의 집계 함수 모음이다. 금액 합계 자체보다는 건수/분포 위주다.

use inv_core::ledger::{count_by_kind, distinct_skus, LedgerLine, LineKind};

/// 원장 요약 통계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerSummary {
    pub line_count: usize,
    pub sku_count: usize,
    pub sale_count: usize,
    pub refund_count: usize,
    pub adjustment_count: usize,
}

/// 원장 라인 목록으로부터 요약 통계를 만든다.
pub fn summarize(lines: &[LedgerLine]) -> LedgerSummary {
    let (sale, refund, adjustment) = count_by_kind(lines);
    LedgerSummary {
        line_count: lines.len(),
        sku_count: distinct_skus(lines).len(),
        sale_count: sale,
        refund_count: refund,
        adjustment_count: adjustment,
    }
}

impl LedgerSummary {
    /// 판매 라인이 전체에서 차지하는 비율(%)을 계산한다.
    pub fn sale_ratio_percent(&self) -> u32 {
        if self.line_count == 0 {
            0
        } else {
            ((self.sale_count * 100) / self.line_count) as u32
        }
    }

    /// 요약이 비어 있는(라인이 하나도 없는) 원장을 가리키는지 검사한다.
    pub fn is_empty(&self) -> bool {
        self.line_count == 0
    }

    /// SKU당 평균 라인 수(같은 SKU가 몇 번 등장했는지의 평균)를 계산한다.
    pub fn average_lines_per_sku(&self) -> f64 {
        if self.sku_count == 0 {
            0.0
        } else {
            self.line_count as f64 / self.sku_count as f64
        }
    }
}

/// 요약을 사람이 읽는 한 줄로 포맷한다.
pub fn format_summary_line(summary: &LedgerSummary) -> String {
    format!(
        "라인 {}건 (판매 {}, 환불 {}, 조정 {}) / SKU {}종",
        summary.line_count, summary.sale_count, summary.refund_count, summary.adjustment_count, summary.sku_count
    )
}

/// 두 원장 요약을 비교해 라인 수 증감을 계산한다.
pub fn line_count_delta(current: &LedgerSummary, previous: &LedgerSummary) -> i64 {
    current.line_count as i64 - previous.line_count as i64
}

/// 원장에서 특정 종류(kind)의 라인 비율(%)을 계산한다.
pub fn kind_ratio_percent(lines: &[LedgerLine], kind: LineKind) -> u32 {
    if lines.is_empty() {
        return 0;
    }
    let count = lines.iter().filter(|l| l.kind == kind).count();
    ((count * 100) / lines.len()) as u32
}

/// 원장에서 가장 많이 등장한 SKU(빈도 기준)를 찾는다(동률이면 사전순 먼저).
pub fn most_frequent_sku(lines: &[LedgerLine]) -> Option<String> {
    let mut skus = distinct_skus(lines);
    skus.sort();
    skus.into_iter().max_by_key(|sku| lines.iter().filter(|l| &l.sku == sku).count())
}

/// 원장이 판매 라인을 하나도 포함하지 않는지(전부 환불/조정인지) 검사한다.
pub fn has_no_sales(lines: &[LedgerLine]) -> bool {
    !lines.iter().any(|l| matches!(l.kind, LineKind::Sale))
}

/// 여러 원장 배치의 요약을 한 번에 계산한다.
pub fn summarize_batches(batches: &[Vec<LedgerLine>]) -> Vec<LedgerSummary> {
    batches.iter().map(|lines| summarize(lines)).collect()
}

/// 여러 요약의 라인 수 총합을 구한다.
pub fn total_line_count(summaries: &[LedgerSummary]) -> usize {
    summaries.iter().map(|s| s.line_count).sum()
}

/// 요약 목록 중 SKU 종류가 가장 많은 것을 찾는다.
pub fn widest_summary(summaries: &[LedgerSummary]) -> Option<&LedgerSummary> {
    summaries.iter().max_by_key(|s| s.sku_count)
}

/// 요약 목록의 평균 SKU 종류 수를 계산한다.
pub fn average_sku_count(summaries: &[LedgerSummary]) -> f64 {
    if summaries.is_empty() {
        0.0
    } else {
        summaries.iter().map(|s| s.sku_count).sum::<usize>() as f64 / summaries.len() as f64
    }
}

/// 요약 목록 중 라인이 하나도 없는(빈) 요약의 개수를 센다.
pub fn empty_summary_count(summaries: &[LedgerSummary]) -> usize {
    summaries.iter().filter(|s| s.is_empty()).count()
}

/// 원장에서 SKU별 라인 개수를 (SKU, 개수) 목록으로 계산한다(SKU 오름차순).
pub fn line_count_by_sku(lines: &[LedgerLine]) -> Vec<(String, usize)> {
    let mut skus = distinct_skus(lines);
    skus.sort();
    skus.into_iter().map(|sku| (sku.clone(), lines.iter().filter(|l| l.sku == sku).count())).collect()
}

/// 원장에서 라인이 하나뿐인(단일 등장) SKU 목록을 찾는다.
pub fn single_occurrence_skus(lines: &[LedgerLine]) -> Vec<String> {
    line_count_by_sku(lines).into_iter().filter(|(_, count)| *count == 1).map(|(sku, _)| sku).collect()
}

/// 원장에서 라인이 지정 횟수 이상 등장한(빈번한) SKU 목록을 찾는다.
pub fn frequent_skus(lines: &[LedgerLine], min_occurrences: usize) -> Vec<String> {
    line_count_by_sku(lines).into_iter().filter(|(_, count)| *count >= min_occurrences).map(|(sku, _)| sku).collect()
}

/// 두 요약을 합쳐 하나의 합산 요약을 만든다(여러 배치를 한 번에 볼 때
/// 쓴다). SKU 종류 수는 원본 라인 없이는 정확한 합집합을 구할 수 없어
/// 두 값 중 큰 쪽으로 근사한다(대략적인 대시보드 표시용, 정밀 집계에는
/// `summarize`를 합친 원장에 직접 적용할 것).
pub fn merge_summaries(a: &LedgerSummary, b: &LedgerSummary) -> LedgerSummary {
    LedgerSummary {
        line_count: a.line_count + b.line_count,
        sku_count: a.sku_count.max(b.sku_count),
        sale_count: a.sale_count + b.sale_count,
        refund_count: a.refund_count + b.refund_count,
        adjustment_count: a.adjustment_count + b.adjustment_count,
    }
}
