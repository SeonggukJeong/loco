//! 한 번에 한 행씩 읽는 스트리밍형 리더.
//!
//! 대용량 CSV를 메모리에 전부 올리지 않고 행 단위로 순회할 때 쓴다. 이
//! 크레이트 자체는 파일 I/O를 하지 않으므로, 실제로는 이미 읽어들인
//! 텍스트를 줄 단위 이터레이터로 감싸는 형태다 — 실제 I/O 경계는 상위
//! 계층(inv-cli)의 몫이다. `readers` 모듈의 일괄 처리와 대비되는, 파일
//! 하나를 순서대로 처리하는 경로다.

use crate::csv::{parse_row, ParseError, ParsedRow};

/// 텍스트 하나를 행 단위로 순회하는 리더.
pub struct RowReader<'a> {
    lines: std::str::Lines<'a>,
    skip_header: bool,
    header_skipped: bool,
    row_number: usize,
}

impl<'a> RowReader<'a> {
    /// 새 리더를 만든다. `skip_header`가 true면 첫 번째 비어있지 않은
    /// 줄을 헤더로 간주해 건너뛴다.
    pub fn new(text: &'a str, skip_header: bool) -> Self {
        RowReader { lines: text.lines(), skip_header, header_skipped: false, row_number: 0 }
    }

    /// 지금까지 진행한 데이터 행 번호(헤더 제외, 1부터 시작).
    pub fn current_row_number(&self) -> usize {
        self.row_number
    }
}

impl<'a> Iterator for RowReader<'a> {
    type Item = (usize, Result<ParsedRow, ParseError>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = self.lines.next()?;
            if line.trim().is_empty() {
                continue;
            }
            if self.skip_header && !self.header_skipped {
                self.header_skipped = true;
                continue;
            }
            self.row_number += 1;
            return Some((self.row_number, parse_row(line)));
        }
    }
}

/// 리더를 끝까지 소진해 성공한 행만 모은다(오류는 버린다 — 오류까지
/// 필요하면 `RowReader`를 직접 순회할 것).
pub fn collect_valid_rows(text: &str, skip_header: bool) -> Vec<ParsedRow> {
    RowReader::new(text, skip_header).filter_map(|(_, result)| result.ok()).collect()
}

/// 리더를 끝까지 소진해 첫 번째 오류만 반환한다(있으면).
pub fn first_error(text: &str, skip_header: bool) -> Option<(usize, ParseError)> {
    RowReader::new(text, skip_header).find_map(|(n, result)| result.err().map(|e| (n, e)))
}

/// 데이터 행(헤더 제외) 개수를 센다.
pub fn count_data_rows(text: &str, skip_header: bool) -> usize {
    RowReader::new(text, skip_header).count()
}

/// 리더를 앞에서부터 최대 `limit`개까지만 읽는다(대용량 파일 미리보기용).
pub fn preview_rows(text: &str, skip_header: bool, limit: usize) -> Vec<(usize, Result<ParsedRow, ParseError>)> {
    RowReader::new(text, skip_header).take(limit).collect()
}

/// 데이터 행이 하나도 없는(헤더뿐이거나 완전히 빈) 텍스트인지 검사한다.
pub fn is_empty_of_data(text: &str, skip_header: bool) -> bool {
    RowReader::new(text, skip_header).next().is_none()
}

/// 리더를 순회하며 성공/실패 개수를 함께 센다(오류 세부 내역이 필요 없을
/// 때의 가벼운 버전 — `readers::read_batch`보다 할당이 적다).
pub fn count_valid_and_errors(text: &str, skip_header: bool) -> (usize, usize) {
    let mut valid = 0usize;
    let mut errored = 0usize;
    for (_, result) in RowReader::new(text, skip_header) {
        if result.is_ok() {
            valid += 1;
        } else {
            errored += 1;
        }
    }
    (valid, errored)
}

/// 특정 조건을 만족하는 첫 번째 유효 행을 찾는다.
pub fn find_first_valid<F>(text: &str, skip_header: bool, predicate: F) -> Option<ParsedRow>
where
    F: Fn(&ParsedRow) -> bool,
{
    RowReader::new(text, skip_header).filter_map(|(_, r)| r.ok()).find(|row| predicate(row))
}

/// 리더를 순회하며 각 오류를 사람이 읽을 수 있는 메시지로 바꿔 모은다.
pub fn describe_all_errors(text: &str, skip_header: bool) -> Vec<String> {
    RowReader::new(text, skip_header)
        .filter_map(|(n, r)| r.err().map(|e| (n, e)))
        .map(|(n, e)| format!("{n}번째 행: {}", crate::csv::describe_error(&e)))
        .collect()
}

/// 리더가 소진될 때까지 읽은 총 줄 수(빈 줄/헤더 포함)를 계산한다.
pub fn total_line_count(text: &str) -> usize {
    text.lines().count()
}

/// 텍스트의 데이터 행 대 오류 행 비율이 허용 범위인지 미리 점검한다
/// (본격적으로 저장소에 반영하기 전, 배치 자체를 반려할지 빠르게 판단).
pub fn passes_error_budget(text: &str, skip_header: bool, max_error_rate_percent: u32) -> bool {
    let (valid, errored) = count_valid_and_errors(text, skip_header);
    !crate::validate::should_reject_batch(valid, errored, max_error_rate_percent)
}
