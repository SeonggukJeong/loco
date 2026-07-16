//! `report` 서브커맨드: 원장 데이터를 요약해 사람이 읽는 텍스트로 출력한다.
//!
//! 실서비스에서는 원장 데이터를 inv-store에서 읽어오지만, 이 서브커맨드는
//! 온보딩/스모크 테스트용으로 내장된 샘플 데이터로 동작하도록 만들어져
//! 있다 — 실제 원장 연동은 후속 작업이다.

use inv_core::inventory::{restock_threshold, WarehouseGrade};
use inv_core::ledger::{LedgerLine, LineKind};
use inv_report::report::build_report;
use inv_report::totals::calc_total;

/// 리포트 커맨드에서 쓰는 샘플 원장 데이터.
pub fn sample_lines() -> Vec<LedgerLine> {
    vec![
        LedgerLine::new("EL-000123", LineKind::Sale, 50_000, 0),
        LedgerLine::new("EL-000123", LineKind::Refund, 10_000, 0),
        LedgerLine::new("FD-000456", LineKind::Sale, 30_000, 0),
        LedgerLine::new("FD-000456", LineKind::Adjustment, 0, 2_000),
    ]
}

/// 현재 경로: `inv_report::report`의 조립 계층을 그대로 호출한다.
pub fn execute(lines: &[LedgerLine]) -> String {
    let report = build_report(lines);
    let note = restock_note(lines.len() as u32);
    format!("{}\n{}", report.summary_line(), note)
}

/// 재입고 권고 메모 한 줄을 만든다. `inv_core::inventory` 재수출 경로로
/// `restock_threshold`를 가져와 쓴다. 창고 등급은 현재 지역 창고(Regional)
/// 고정값을 쓴다(다중 창고 지원은 백로그).
fn restock_note(daily_avg: u32) -> String {
    let threshold = restock_threshold(daily_avg, 7, WarehouseGrade::Regional);
    format!("재입고 임계값(참고): {threshold}")
}

/// 레거시 출력 포맷 경로. 옛 배치 스크립트들이 이 포맷을 파싱하도록
/// 만들어져 있어, 신규 조립 계층으로 옮긴 뒤에도 당분간 이 함수를
/// 남겨둔다. 순매출은 v1 합계 함수를 직접 호출한다.
pub fn execute_legacy(lines: &[LedgerLine]) -> String {
    let total = calc_total(lines);
    format!("[LEGACY] total={total}")
}

/// 레거시 경로 출력이 예상되는 접두("[LEGACY] total=")로 시작하는지
/// 검사한다(라우팅 테스트에서 두 경로를 구분할 때 쓴다).
pub fn is_legacy_output(output: &str) -> bool {
    output.starts_with("[LEGACY] total=")
}

/// 리포트 실행 결과 문자열에서 재입고 임계값 메모 줄만 뽑아낸다.
pub fn extract_restock_line(output: &str) -> Option<&str> {
    output.lines().find(|l| l.starts_with("재입고 임계값"))
}

/// 창고 등급 이름(문자열)을 받아 `WarehouseGrade`로 변환한다. 알 수 없는
/// 값은 가장 보수적인 등급(Local, 버퍼 없음)으로 대체한다.
fn parse_grade(name: &str) -> WarehouseGrade {
    match name.to_ascii_uppercase().as_str() {
        "CENTRAL" => WarehouseGrade::Central,
        "REGIONAL" => WarehouseGrade::Regional,
        _ => WarehouseGrade::Local,
    }
}

/// 창고 등급을 지정해 재입고 권고 메모를 만든다(`execute`가 쓰는
/// `restock_note`의 등급 지정 가능 버전 — 향후 `--grade` 플래그 지원 대비).
pub fn restock_note_for_grade(daily_avg: u32, grade_name: &str) -> String {
    let grade = parse_grade(grade_name);
    let threshold = restock_threshold(daily_avg, 7, grade);
    format!("재입고 임계값({grade_name}): {threshold}")
}

/// 원장 라인 개수로부터 배치 규모 등급을 문자열로 판정한다(출력 메모 보조).
pub fn batch_size_label(lines: &[LedgerLine]) -> &'static str {
    match lines.len() {
        0 => "빈 배치",
        1..=10 => "소규모",
        11..=100 => "중규모",
        _ => "대규모",
    }
}

/// 샘플 원장 데이터의 SKU 종류 수를 센다(리포트 헤더 보조 정보용).
pub fn sample_sku_count() -> usize {
    let lines = sample_lines();
    let mut skus: Vec<&str> = lines.iter().map(|l| l.sku.as_str()).collect();
    skus.sort();
    skus.dedup();
    skus.len()
}

/// 리포트 실행 결과에 재입고 메모 줄이 포함되어 있는지 검사한다(현재
/// 경로 전용 — 레거시 경로는 재입고 메모를 만들지 않는다).
pub fn has_restock_note(output: &str) -> bool {
    extract_restock_line(output).is_some()
}

/// 현재 경로와 레거시 경로 출력을 나란히 만들어 비교용 텍스트로 합친다
/// (마이그레이션 검토 중 두 경로의 출력을 한눈에 볼 때 쓴다).
pub fn compare_paths(lines: &[LedgerLine]) -> String {
    format!("[현재]\n{}\n\n[레거시]\n{}", execute(lines), execute_legacy(lines))
}

/// 두 경로의 출력이 모두 비어 있지 않은지(최소한의 스모크 검사) 확인한다.
pub fn both_paths_nonempty(lines: &[LedgerLine]) -> bool {
    !execute(lines).is_empty() && !execute_legacy(lines).is_empty()
}

/// 레거시 출력에서 total= 뒤의 숫자 문자열만 뽑아낸다(파싱 검증용).
pub fn extract_legacy_total_str(output: &str) -> Option<&str> {
    output.strip_prefix("[LEGACY] total=")
}

/// 레거시 출력에서 순매출 값을 정수로 파싱한다.
pub fn parse_legacy_total(output: &str) -> Option<i64> {
    extract_legacy_total_str(output)?.parse::<i64>().ok()
}

/// 창고 등급 이름 목록(사용자에게 보여줄 안내용).
pub const GRADE_NAMES: [&str; 3] = ["CENTRAL", "REGIONAL", "LOCAL"];

/// 창고 등급 이름이 유효한지 검사한다.
pub fn is_valid_grade_name(name: &str) -> bool {
    GRADE_NAMES.contains(&name.to_ascii_uppercase().as_str())
}

/// 샘플 원장의 판매 라인만 골라 개수를 센다(리포트 헤더 보조 정보).
pub fn sample_sale_line_count() -> usize {
    sample_lines().iter().filter(|l| matches!(l.kind, LineKind::Sale)).count()
}

/// 샘플 원장의 환불 라인만 골라 개수를 센다.
pub fn sample_refund_line_count() -> usize {
    sample_lines().iter().filter(|l| matches!(l.kind, LineKind::Refund)).count()
}

/// 현재 경로 출력에 순매출 라벨("순매출")이 포함되어 있는지 검사한다
/// (출력 형식이 예상과 다르게 바뀌지 않았는지 확인하는 가벼운 스모크
/// 체크).
pub fn has_net_label(output: &str) -> bool {
    output.contains("순매출")
}

/// 여러 등급에 대해 재입고 메모를 한 번에 만들어 여러 줄 텍스트로 반환한다.
pub fn restock_notes_for_all_grades(daily_avg: u32) -> String {
    GRADE_NAMES.iter().map(|g| restock_note_for_grade(daily_avg, g)).collect::<Vec<_>>().join("\n")
}

/// 배치 규모 라벨이 지정 목록 중 하나인지 검사한다(라우팅 테스트 보조용).
pub fn is_known_batch_label(label: &str) -> bool {
    matches!(label, "빈 배치" | "소규모" | "중규모" | "대규모")
}
