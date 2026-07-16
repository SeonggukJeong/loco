//! 여러 파일(텍스트 블록)을 한 번에 일괄 처리하는 배치 리더.
//!
//! `reader::RowReader`가 파일 하나를 행 단위로 순회한다면, 이 모듈은
//! 여러 파일을 한 번의 호출로 모아 처리하고 파일별 통계를 함께 낸다.
//! 야간 배치 작업(여러 창고에서 올라온 CSV를 한 번에 합쳐 처리)에서
//! 주로 쓰인다.

use crate::csv::{ParseError, ParsedRow};
use crate::reader::RowReader;

/// 파일 하나를 처리한 결과 통계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub valid_rows: usize,
    pub error_rows: usize,
}

/// 여러 파일을 일괄 처리한 결과.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchResult {
    pub rows: Vec<ParsedRow>,
    pub errors: Vec<(usize, ParseError)>,
    pub per_file: Vec<FileStat>,
}

/// 여러 텍스트를 순서대로 파싱해 하나의 배치 결과로 합친다.
pub fn read_batch(texts: &[&str], skip_header: bool) -> BatchResult {
    let mut result = BatchResult::default();
    for text in texts {
        let mut valid = 0usize;
        let mut errored = 0usize;
        for (line_no, parsed) in RowReader::new(text, skip_header) {
            match parsed {
                Ok(row) => {
                    result.rows.push(row);
                    valid += 1;
                }
                Err(e) => {
                    result.errors.push((line_no, e));
                    errored += 1;
                }
            }
        }
        result.per_file.push(FileStat { valid_rows: valid, error_rows: errored });
    }
    result
}

impl BatchResult {
    /// 처리한 파일 개수.
    pub fn file_count(&self) -> usize {
        self.per_file.len()
    }

    /// 전체 유효 행 수.
    pub fn total_valid(&self) -> usize {
        self.rows.len()
    }

    /// 전체 오류 행 수.
    pub fn total_errors(&self) -> usize {
        self.errors.len()
    }

    /// 오류가 하나도 없는 파일의 개수.
    pub fn clean_file_count(&self) -> usize {
        self.per_file.iter().filter(|f| f.error_rows == 0).count()
    }

    /// 오류가 하나라도 있었던 파일의 인덱스 목록(0부터 시작).
    pub fn dirty_file_indices(&self) -> Vec<usize> {
        self.per_file
            .iter()
            .enumerate()
            .filter(|(_, f)| f.error_rows > 0)
            .map(|(i, _)| i)
            .collect()
    }
}

/// 여러 배치 결과를 하나로 합친다(파일 순서를 이어붙인다).
pub fn merge_batches(batches: &[BatchResult]) -> BatchResult {
    let mut merged = BatchResult::default();
    for batch in batches {
        merged.rows.extend(batch.rows.iter().cloned());
        merged.errors.extend(batch.errors.iter().cloned());
        merged.per_file.extend(batch.per_file.iter().cloned());
    }
    merged
}

/// 배치 결과 중 특정 인덱스의 파일 하나만 재요약한다.
pub fn stat_for_file(result: &BatchResult, file_index: usize) -> Option<&FileStat> {
    result.per_file.get(file_index)
}

/// 파일별 유효 행 비율(%)을 계산한다(빈 파일은 0%로 취급).
pub fn valid_ratio_percent(stat: &FileStat) -> u32 {
    let total = stat.valid_rows + stat.error_rows;
    if total == 0 {
        0
    } else {
        (stat.valid_rows * 100 / total) as u32
    }
}

/// 배치 전체에서 가장 오류가 많았던 파일의 인덱스를 찾는다(동률이면 앞선 것).
pub fn worst_file_index(result: &BatchResult) -> Option<usize> {
    result
        .per_file
        .iter()
        .enumerate()
        .max_by_key(|(_, f)| f.error_rows)
        .map(|(i, _)| i)
}
