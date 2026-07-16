//! 현재 운영 중인 보고서 조립 계층.
//!
//! 각 집계 모듈(`totals`/`invoice`/`forecast`)의 결과를 모아 하나의 보고서
//! 구조체로 묶는 얇은 조립 계층이다. `reporting`(옛 계층)과 `report_v2`
//! (신 계층) 사이에서 현재 CLI 기본 경로가 실제로 호출하는 곳이며, 순매출
//! 계산은 아직 v1 합계 함수(`totals::calc_total`)를 그대로 쓴다.

use inv_core::ledger::LedgerLine;

use crate::forecast::forecast_projection;
use crate::invoice::invoice_total;
use crate::totals::calc_total;

/// 조립된 보고서 한 건.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub net_krw: i64,
    pub invoice_total_krw: i64,
    pub forecast_krw: i64,
    pub line_count: usize,
}

impl Report {
    /// 보고서를 한 줄 요약 문자열로 만든다.
    pub fn summary_line(&self) -> String {
        format!(
            "순매출 {}원 / 청구총액 {}원 / 전망 {}원 (라인 {}건)",
            self.net_krw, self.invoice_total_krw, self.forecast_krw, self.line_count
        )
    }

    /// 순매출이 음수인지(적자 구간인지) 검사한다.
    pub fn is_negative(&self) -> bool {
        self.net_krw < 0
    }
}

/// 원장 라인으로부터 보고서를 조립한다. 순매출은 v1 합계 로직을 쓰고,
/// 그 위에 청구 총액과 전망치를 얹는다.
pub fn build_report(lines: &[LedgerLine]) -> Report {
    let net = calc_total(lines);
    Report {
        net_krw: net,
        invoice_total_krw: invoice_total(net.max(0)),
        forecast_krw: forecast_projection(net),
        line_count: lines.len(),
    }
}

/// 여러 원장(예: SKU별로 나뉜 배치)에 대해 보고서를 한 번에 조립한다.
pub fn build_reports(batches: &[Vec<LedgerLine>]) -> Vec<Report> {
    batches.iter().map(|lines| build_report(lines)).collect()
}

/// 보고서 목록에서 순매출 합계를 구한다(배치 간 합산, v1 합계 재사용 아님
/// — 이미 계산된 `Report::net_krw`를 그대로 더한다).
pub fn total_net_across_reports(reports: &[Report]) -> i64 {
    reports.iter().map(|r| r.net_krw).sum()
}

/// 보고서 목록 중 순매출이 가장 큰 것을 찾는다.
pub fn best_report(reports: &[Report]) -> Option<&Report> {
    reports.iter().max_by_key(|r| r.net_krw)
}

/// 보고서 목록 중 적자(순매출 음수)인 것만 걸러낸다.
pub fn negative_reports(reports: &[Report]) -> Vec<Report> {
    reports.iter().filter(|r| r.is_negative()).cloned().collect()
}

/// 보고서를 사람이 읽는 여러 줄짜리 텍스트로 확장 포맷한다.
pub fn format_report_detail(report: &Report) -> String {
    format!(
        "== 보고서 ==\n순매출: {}원\n청구총액(부가세 포함): {}원\n전망치: {}원\n원장 라인 수: {}건",
        report.net_krw, report.invoice_total_krw, report.forecast_krw, report.line_count
    )
}

/// 보고서 목록의 평균 라인 수를 계산한다(배치 크기 경향 파악용).
pub fn average_line_count(reports: &[Report]) -> f64 {
    if reports.is_empty() {
        0.0
    } else {
        reports.iter().map(|r| r.line_count).sum::<usize>() as f64 / reports.len() as f64
    }
}

/// 보고서 목록 중 라인 수가 가장 많은(가장 큰 배치) 것을 찾는다.
pub fn largest_batch(reports: &[Report]) -> Option<&Report> {
    reports.iter().max_by_key(|r| r.line_count)
}

/// 보고서 목록을 순매출 내림차순으로 정렬한다.
pub fn sort_by_net_desc(reports: &mut Vec<Report>) {
    reports.sort_by(|a, b| b.net_krw.cmp(&a.net_krw));
}

/// 보고서 목록 중 순매출이 지정 임계값 이상인 것만 걸러낸다.
pub fn reports_above_threshold(reports: &[Report], threshold_krw: i64) -> Vec<Report> {
    reports.iter().filter(|r| r.net_krw >= threshold_krw).cloned().collect()
}

/// 보고서 목록의 총 라인 수를 합산한다.
pub fn total_line_count_across_reports(reports: &[Report]) -> usize {
    reports.iter().map(|r| r.line_count).sum()
}

/// 두 보고서의 순매출 차이를 계산한다(마이그레이션 검증/회귀 비교용).
pub fn net_diff(a: &Report, b: &Report) -> i64 {
    a.net_krw - b.net_krw
}

/// 보고서 목록 중 라인이 하나도 없는(빈 배치) 것의 개수를 센다.
pub fn empty_batch_count(reports: &[Report]) -> usize {
    reports.iter().filter(|r| r.line_count == 0).count()
}
