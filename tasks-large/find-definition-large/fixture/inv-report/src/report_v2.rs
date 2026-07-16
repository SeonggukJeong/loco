//! 신규 보고서 조립 계층(v2).
//!
//! `report` 모듈의 뒤를 이어 만들어진 조립 계층으로, 순매출 계산에
//! `monthly` 모듈의 fold 기반 v2 로직(`monthly_total`)을 쓴다. 향후 CLI의
//! 기본 경로를 이 모듈로 옮길 예정이지만, 아직은 옛 경로(`report`)와
//! 나란히 존재한다.

use inv_core::ledger::LedgerLine;

use crate::forecast::forecast_projection;
use crate::invoice::invoice_total;
use crate::monthly::monthly_total;

/// v2 조립 계층이 만드는 보고서 한 건.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportV2 {
    pub net_krw: i64,
    pub invoice_total_krw: i64,
    pub forecast_krw: i64,
    pub line_count: usize,
    pub sku_count: usize,
}

impl ReportV2 {
    /// 보고서를 한 줄 요약 문자열로 만든다.
    pub fn summary_line(&self) -> String {
        format!(
            "[v2] 순매출 {}원 / 청구총액 {}원 / 전망 {}원 (라인 {}건, SKU {}종)",
            self.net_krw, self.invoice_total_krw, self.forecast_krw, self.line_count, self.sku_count
        )
    }

    /// v1 조립 결과(`report::Report`)와 순매출을 비교한다(마이그레이션 중
    /// 두 경로의 결과 차이를 점검할 때 쓴다).
    pub fn net_diff_from(&self, other_net_krw: i64) -> i64 {
        self.net_krw - other_net_krw
    }
}

/// 원장 라인으로부터 v2 보고서를 조립한다.
pub fn build_report_v2(lines: &[LedgerLine]) -> ReportV2 {
    let net = monthly_total(lines);
    let mut skus: Vec<&str> = lines.iter().map(|l| l.sku.as_str()).collect();
    skus.sort();
    skus.dedup();
    ReportV2 {
        net_krw: net,
        invoice_total_krw: invoice_total(net.max(0)),
        forecast_krw: forecast_projection(net),
        line_count: lines.len(),
        sku_count: skus.len(),
    }
}

/// 여러 배치에 대해 v2 보고서를 한 번에 조립한다.
pub fn build_reports_v2(batches: &[Vec<LedgerLine>]) -> Vec<ReportV2> {
    batches.iter().map(|lines| build_report_v2(lines)).collect()
}

/// v2 보고서 목록의 순매출 합계를 구한다.
pub fn total_net_across_reports_v2(reports: &[ReportV2]) -> i64 {
    reports.iter().map(|r| r.net_krw).sum()
}

/// v2 보고서 목록 중 SKU 종류가 가장 다양한(커버리지가 넓은) 것을 찾는다.
pub fn widest_coverage_report(reports: &[ReportV2]) -> Option<&ReportV2> {
    reports.iter().max_by_key(|r| r.sku_count)
}

/// v2 보고서를 확장 텍스트로 포맷한다(v1의 `format_report_detail`과
/// 대응하는 신규 포맷 — SKU 종류 수가 추가되었다).
pub fn format_report_v2_detail(report: &ReportV2) -> String {
    format!(
        "== 보고서(v2) ==\n순매출: {}원\n청구총액(부가세 포함): {}원\n전망치: {}원\n원장 라인 수: {}건\nSKU 종류: {}종",
        report.net_krw, report.invoice_total_krw, report.forecast_krw, report.line_count, report.sku_count
    )
}

/// v2 보고서 목록의 평균 SKU 커버리지를 계산한다.
pub fn average_sku_count(reports: &[ReportV2]) -> f64 {
    if reports.is_empty() {
        0.0
    } else {
        reports.iter().map(|r| r.sku_count).sum::<usize>() as f64 / reports.len() as f64
    }
}

/// v2 보고서 목록 중 순매출 상위 N개를 내림차순으로 뽑는다.
pub fn top_n_by_net(reports: &[ReportV2], n: usize) -> Vec<ReportV2> {
    let mut sorted = reports.to_vec();
    sorted.sort_by(|a, b| b.net_krw.cmp(&a.net_krw));
    sorted.into_iter().take(n).collect()
}

/// v2 보고서 목록 중 순매출이 지정 임계값 이상인 것만 걸러낸다.
pub fn reports_v2_above_threshold(reports: &[ReportV2], threshold_krw: i64) -> Vec<ReportV2> {
    reports.iter().filter(|r| r.net_krw >= threshold_krw).cloned().collect()
}

/// v2 보고서 목록의 SKU 커버리지 합계를 구한다(중복 제거 없이 단순 합산 —
/// 배치 간 SKU 중복은 고려하지 않는다).
pub fn total_sku_count_across_reports(reports: &[ReportV2]) -> usize {
    reports.iter().map(|r| r.sku_count).sum()
}

/// 두 보고서(v1/v2)의 순매출 차이가 허용 오차 이내인지 검사한다
/// (마이그레이션 중 두 경로가 같은 결과를 내는지 회귀 검증할 때 쓴다).
pub fn matches_within_tolerance(report_v2: &ReportV2, other_net_krw: i64, tolerance_krw: i64) -> bool {
    report_v2.net_diff_from(other_net_krw).abs() <= tolerance_krw
}

/// v2 보고서 목록을 라인 수 기준으로 오름차순 정렬한다.
pub fn sort_by_line_count_asc(reports: &mut Vec<ReportV2>) {
    reports.sort_by_key(|r| r.line_count);
}

/// v2 보고서 목록 중 SKU 종류가 1개뿐인(단일 품목 배치) 것의 개수를 센다.
pub fn single_sku_report_count(reports: &[ReportV2]) -> usize {
    reports.iter().filter(|r| r.sku_count == 1).count()
}

/// v2 보고서 목록에서 라인당 평균 순매출(라인 수로 나눈 값)을 계산한다.
pub fn net_per_line(report: &ReportV2) -> f64 {
    if report.line_count == 0 {
        0.0
    } else {
        report.net_krw as f64 / report.line_count as f64
    }
}
