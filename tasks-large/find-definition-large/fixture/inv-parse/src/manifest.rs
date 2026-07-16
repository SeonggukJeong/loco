//! 배치 처리 결과를 요약한 임포트 매니페스트.
//!
//! 배치를 다 처리한 뒤 "이번 배치가 몇 건 성공/실패했는지"를 운영팀에
//! 보고하거나 로그로 남길 때 쓰는 요약 구조체다. `readers::BatchResult`가
//! 원시 결과(행/오류 전부)를 담는다면, 이 모듈은 그걸 사람이 읽을 보고서
//! 형태로 압축한다.

use crate::readers::BatchResult;

/// 배치 처리 결과 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportManifest {
    pub file_count: usize,
    pub total_rows: usize,
    pub valid_rows: usize,
    pub error_rows: usize,
    pub clean_file_count: usize,
}

/// `BatchResult`로부터 매니페스트를 만든다.
pub fn summarize_batch(result: &BatchResult) -> ImportManifest {
    ImportManifest {
        file_count: result.file_count(),
        total_rows: result.total_valid() + result.total_errors(),
        valid_rows: result.total_valid(),
        error_rows: result.total_errors(),
        clean_file_count: result.clean_file_count(),
    }
}

impl ImportManifest {
    /// 유효 행 비율(%)을 계산한다(전체가 0행이면 100%로 본다 — 처리할
    /// 것이 없었으니 실패도 없었다는 관례).
    pub fn success_rate_percent(&self) -> u32 {
        if self.total_rows == 0 {
            100
        } else {
            (self.valid_rows * 100 / self.total_rows) as u32
        }
    }

    /// 오류가 하나라도 있는 파일이 있는지 여부.
    pub fn has_dirty_files(&self) -> bool {
        self.clean_file_count < self.file_count
    }

    /// 배치를 통째로 반려해야 할 만큼 오류율이 높은지 판정한다.
    pub fn should_flag_for_review(&self, min_success_rate_percent: u32) -> bool {
        self.success_rate_percent() < min_success_rate_percent
    }

    /// 사람이 읽는 한 줄 요약 문자열을 만든다.
    pub fn summary_line(&self) -> String {
        format!(
            "파일 {}개, 총 {}행 중 {}행 성공({}%), {}행 오류",
            self.file_count,
            self.total_rows,
            self.valid_rows,
            self.success_rate_percent(),
            self.error_rows
        )
    }
}

/// 두 매니페스트를 합친다(연속된 배치를 이어 처리한 경우 누적 집계용).
pub fn combine(a: &ImportManifest, b: &ImportManifest) -> ImportManifest {
    ImportManifest {
        file_count: a.file_count + b.file_count,
        total_rows: a.total_rows + b.total_rows,
        valid_rows: a.valid_rows + b.valid_rows,
        error_rows: a.error_rows + b.error_rows,
        clean_file_count: a.clean_file_count + b.clean_file_count,
    }
}

/// 매니페스트 목록에서 성공률이 가장 낮았던 것을 찾는다.
pub fn worst_manifest(manifests: &[ImportManifest]) -> Option<&ImportManifest> {
    manifests.iter().min_by_key(|m| m.success_rate_percent())
}

/// 빈 매니페스트(처리한 것이 아무것도 없는 상태)를 만든다.
pub fn empty_manifest() -> ImportManifest {
    ImportManifest { file_count: 0, total_rows: 0, valid_rows: 0, error_rows: 0, clean_file_count: 0 }
}

/// 매니페스트 목록의 평균 성공률(%)을 계산한다(빈 목록이면 100%).
pub fn average_success_rate_percent(manifests: &[ImportManifest]) -> u32 {
    if manifests.is_empty() {
        return 100;
    }
    let sum: u32 = manifests.iter().map(|m| m.success_rate_percent()).sum();
    sum / manifests.len() as u32
}

/// 여러 배치를 순서대로 처리한 매니페스트 이력에서, 성공률이 이전 배치보다
/// 떨어진 지점(인덱스)들을 찾는다(품질 저하 추세 감지용).
pub fn regression_points(history: &[ImportManifest]) -> Vec<usize> {
    let mut points = Vec::new();
    for i in 1..history.len() {
        if history[i].success_rate_percent() < history[i - 1].success_rate_percent() {
            points.push(i);
        }
    }
    points
}

/// 매니페스트가 완전히 깨끗한(오류가 전혀 없는) 배치였는지 검사한다.
pub fn is_perfect_batch(manifest: &ImportManifest) -> bool {
    manifest.error_rows == 0 && manifest.total_rows > 0
}

/// 매니페스트 목록 중 완전히 깨끗했던 배치의 개수를 센다.
pub fn perfect_batch_count(manifests: &[ImportManifest]) -> usize {
    manifests.iter().filter(|m| is_perfect_batch(m)).count()
}

/// 매니페스트를 감사 로그용 한 줄(탭 구분) 레코드로 직렬화한다.
pub fn to_audit_line(manifest: &ImportManifest) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}",
        manifest.file_count, manifest.total_rows, manifest.valid_rows, manifest.error_rows, manifest.clean_file_count
    )
}

/// 매니페스트 목록 전체의 총 처리 행 수를 구한다.
pub fn total_rows_processed(manifests: &[ImportManifest]) -> usize {
    manifests.iter().map(|m| m.total_rows).sum()
}
