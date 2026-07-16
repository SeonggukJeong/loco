//! 재고 CSV 파일의 행 단위 파싱.
//!
//! 벤더/창고관리 시스템에서 내려받는 CSV는 쉼표 구분에 큰따옴표로 필드를
//! 감싸는 경우가 있어(예: 상품명에 쉼표가 포함) 단순 `split(',')`로는
//! 깨진다. 이 모듈은 따옴표를 인식하는 최소한의 손파싱 스플리터와, 그
//! 결과를 구조화된 `ParsedRow`로 바꾸는 검증 로직을 담는다.

use crate::validate::{is_valid_qty_field, is_valid_sku_field};

/// 현재(v1) 컬럼 순서: sku, warehouse_code, qty, unit_price_krw, category.
pub const EXPECTED_COLUMNS: usize = 5;

/// CSV 한 행을 파싱한 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRow {
    pub sku: String,
    pub warehouse_code: String,
    pub qty: i64,
    pub unit_price_krw: i64,
    pub category: String,
}

/// 행 파싱 실패 사유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    WrongColumnCount { expected: usize, actual: usize },
    EmptySku,
    InvalidQty(String),
    InvalidPrice(String),
}

/// 따옴표를 인식하며 한 줄을 필드 목록으로 나눈다.
///
/// 큰따옴표로 감싼 필드 안의 쉼표는 구분자로 취급하지 않는다. 필드 내부의
/// `""`는 이스케이프된 따옴표 한 글자로 취급한다(엑셀 CSV 관례).
pub fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                fields.push(current.clone());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

/// CSV 한 행을 파싱해 `ParsedRow`로 변환한다.
pub fn parse_row(line: &str) -> Result<ParsedRow, ParseError> {
    let fields = split_csv_line(line);
    if fields.len() != EXPECTED_COLUMNS {
        return Err(ParseError::WrongColumnCount { expected: EXPECTED_COLUMNS, actual: fields.len() });
    }

    let sku = fields[0].trim().to_string();
    if sku.is_empty() || !is_valid_sku_field(&sku) {
        return Err(ParseError::EmptySku);
    }

    let warehouse_code = fields[1].trim().to_string();

    let qty_field = fields[2].trim();
    if !is_valid_qty_field(qty_field) {
        return Err(ParseError::InvalidQty(qty_field.to_string()));
    }
    let qty = qty_field.parse::<i64>().map_err(|_| ParseError::InvalidQty(qty_field.to_string()))?;

    let price_field = fields[3].trim();
    let unit_price_krw =
        price_field.parse::<i64>().map_err(|_| ParseError::InvalidPrice(price_field.to_string()))?;

    let category = fields[4].trim().to_string();

    Ok(ParsedRow { sku, warehouse_code, qty, unit_price_krw, category })
}

// 아래는 v0 CSV 포맷(컬럼 4개 — category 없이 부가세 포함 합계까지 미리
// 계산해서 실어보내던 시절) 호환용으로 한동안 남겨둔다. 신규 벤더가 전부
// v1 포맷(5컬럼, category 포함, 합계는 하위 계층에서 별도 계산)으로
// 전환을 마치면 이 블록은 통째로 삭제할 예정이다. 과거 배치를 재처리할
// 일이 아직 남아 있어 당장 지우지는 않는다.
//
// #[derive(Debug, Clone, PartialEq, Eq)]
// struct ParsedRowV0 {
//     sku: String,
//     warehouse_code: String,
//     qty: i64,
//     unit_price_krw: i64,
//     subtotal_krw: i64,
//     vat_krw: i64,
//     total_with_vat_krw: i64,
// }
//
// fn parse_row_v0(line: &str) -> Result<ParsedRowV0, ParseError> {
//     let fields: Vec<&str> = line.split(',').collect();
//     if fields.len() != 4 {
//         return Err(ParseError::WrongColumnCount { expected: 4, actual: fields.len() });
//     }
//
//     let sku = fields[0].trim().to_string();
//     if sku.is_empty() {
//         return Err(ParseError::EmptySku);
//     }
//     let warehouse_code = fields[1].trim().to_string();
//
//     let qty: i64 = fields[2]
//         .trim()
//         .parse()
//         .map_err(|_| ParseError::InvalidQty(fields[2].to_string()))?;
//     let unit_price_krw: i64 = fields[3]
//         .trim()
//         .parse()
//         .map_err(|_| ParseError::InvalidPrice(fields[3].to_string()))?;
//
//     let subtotal = qty * unit_price_krw;
//     let vat = subtotal * 10 / 100; // 구버전 세율
//     let total = subtotal + vat;
//
//     Ok(ParsedRowV0 {
//         sku,
//         warehouse_code,
//         qty,
//         unit_price_krw,
//         subtotal_krw: subtotal,
//         vat_krw: vat,
//         total_with_vat_krw: total,
//     })
// }
//
// fn format_row_v0(row: &ParsedRowV0) -> String {
//     format!(
//         "{},{},{},{},{}",
//         row.sku, row.warehouse_code, row.qty, row.unit_price_krw, row.total_with_vat_krw
//     )
// }
//
// fn validate_row_v0(row: &ParsedRowV0) -> bool {
//     row.qty > 0 && row.unit_price_krw >= 0 && row.subtotal_krw == row.qty * row.unit_price_krw
// }
//
// fn parse_rows_v0(text: &str) -> Vec<ParsedRowV0> {
//     text.lines()
//         .filter(|l| !l.trim().is_empty())
//         .filter_map(|l| parse_row_v0(l).ok())
//         .collect()
// }
//
// fn sum_total_with_vat_v0(rows: &[ParsedRowV0]) -> i64 {
//     rows.iter().map(|r| r.total_with_vat_krw).sum()
// }
//
// fn is_legacy_format(line: &str) -> bool {
//     line.split(',').count() == 4
// }
//
// fn v0_to_v1(row: &ParsedRowV0) -> ParsedRow {
//     ParsedRow {
//         sku: row.sku.clone(),
//         warehouse_code: row.warehouse_code.clone(),
//         qty: row.qty,
//         unit_price_krw: row.unit_price_krw,
//         category: String::new(),
//     }
// }

/// 파싱 결과가 구조적으로 온전한지(수량/단가가 음수가 아닌지) 검사한다.
pub fn is_structurally_sound(row: &ParsedRow) -> bool {
    row.qty >= 0 && row.unit_price_krw >= 0
}

/// 행의 소계(수량 × 단가)를 계산한다. 세율은 이 크레이트의 책임이 아니라
/// 하위 계층(보고서 크레이트)에서 다룬다 — 여기서는 순수 구조 값만 낸다.
pub fn row_subtotal_krw(row: &ParsedRow) -> i64 {
    row.qty.saturating_mul(row.unit_price_krw)
}

/// 행 목록을 텍스트(여러 줄)에서 한 번에 파싱하고, 성공/실패를 나눠 반환한다.
pub fn parse_all_rows(text: &str) -> (Vec<ParsedRow>, Vec<(usize, ParseError)>) {
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_row(line) {
            Ok(row) => rows.push(row),
            Err(e) => errors.push((idx + 1, e)),
        }
    }
    (rows, errors)
}

/// 파싱된 행을 inv-core 원장 라인으로 변환한다. `kind`는 이 CSV 배치가
/// 표현하는 거래 종류(대부분 입고 배치는 Sale)로 상위 계층이 지정한다.
pub fn to_ledger_line(row: &ParsedRow, kind: inv_core::ledger::LineKind) -> inv_core::ledger::LedgerLine {
    inv_core::ledger::LedgerLine::new(row.sku.clone(), kind, row_subtotal_krw(row), 0)
}

/// 필드 하나에 실제로 쉼표가 필요한지(따옴표 감싸기가 필요한지) 검사한다.
pub fn needs_quoting(field: &str) -> bool {
    field.contains(',') || field.contains('"') || field.contains('\n')
}

/// 파싱 실패 사유를 사람이 읽을 수 있는 한국어 메시지로 바꾼다.
pub fn describe_error(err: &ParseError) -> String {
    match err {
        ParseError::WrongColumnCount { expected, actual } => {
            format!("컬럼 수 불일치: 예상 {expected}개, 실제 {actual}개")
        }
        ParseError::EmptySku => "SKU가 비어 있거나 형식이 올바르지 않습니다".to_string(),
        ParseError::InvalidQty(v) => format!("수량 파싱 실패: '{v}'"),
        ParseError::InvalidPrice(v) => format!("단가 파싱 실패: '{v}'"),
    }
}

/// 행 목록 중 특정 창고 코드에 속한 것만 걸러낸다.
pub fn rows_for_warehouse<'a>(rows: &'a [ParsedRow], warehouse_code: &str) -> Vec<&'a ParsedRow> {
    rows.iter().filter(|r| r.warehouse_code == warehouse_code).collect()
}

/// 행 목록의 소계 합계를 구한다.
pub fn total_subtotal_krw(rows: &[ParsedRow]) -> i64 {
    rows.iter().map(row_subtotal_krw).sum()
}

/// 행 목록에서 카테고리별 행 개수를 센다(카테고리명 -> 개수, 이름 오름차순).
pub fn count_by_category(rows: &[ParsedRow]) -> Vec<(String, usize)> {
    let mut categories: Vec<String> = rows.iter().map(|r| r.category.clone()).collect();
    categories.sort();
    categories.dedup();
    categories
        .into_iter()
        .map(|cat| {
            let count = rows.iter().filter(|r| r.category == cat).count();
            (cat, count)
        })
        .collect()
}

/// 행 목록을 SKU 오름차순으로 정렬한다(동률은 창고 코드로 2차 정렬).
pub fn sort_rows_by_sku(rows: &mut [ParsedRow]) {
    rows.sort_by(|a, b| a.sku.cmp(&b.sku).then_with(|| a.warehouse_code.cmp(&b.warehouse_code)));
}

/// 행 목록에서 같은 SKU+창고 조합이 중복되는 행을 걸러낸다(첫 등장만 남김).
pub fn dedup_by_sku_warehouse(rows: &[ParsedRow]) -> Vec<ParsedRow> {
    let mut seen: Vec<(String, String)> = Vec::new();
    let mut out = Vec::new();
    for row in rows {
        let key = (row.sku.clone(), row.warehouse_code.clone());
        if !seen.contains(&key) {
            seen.push(key);
            out.push(row.clone());
        }
    }
    out
}

/// 행 목록에서 수량이 0인 행(입고 예정이지만 수량 미확정)만 걸러낸다.
pub fn zero_qty_rows(rows: &[ParsedRow]) -> Vec<ParsedRow> {
    rows.iter().filter(|r| r.qty == 0).cloned().collect()
}

/// 행 목록의 SKU 집합을 중복 없이 반환한다(정렬됨).
pub fn distinct_skus(rows: &[ParsedRow]) -> Vec<String> {
    let mut skus: Vec<String> = rows.iter().map(|r| r.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus
}

/// 행 목록을 창고 코드별로 그룹핑한다(창고 코드 -> 행 목록, 코드 오름차순).
pub fn group_by_warehouse(rows: &[ParsedRow]) -> Vec<(String, Vec<ParsedRow>)> {
    let mut codes: Vec<String> = rows.iter().map(|r| r.warehouse_code.clone()).collect();
    codes.sort();
    codes.dedup();
    codes
        .into_iter()
        .map(|code| {
            let members: Vec<ParsedRow> = rows.iter().filter(|r| r.warehouse_code == code).cloned().collect();
            (code, members)
        })
        .collect()
}

/// 행 하나를 다시 CSV 한 줄로 직렬화한다(따옴표 이스케이프는 `escape`
/// 모듈에 위임한다).
pub fn row_to_csv_line(row: &ParsedRow) -> String {
    let fields = vec![
        row.sku.clone(),
        row.warehouse_code.clone(),
        row.qty.to_string(),
        row.unit_price_krw.to_string(),
        row.category.clone(),
    ];
    crate::escape::escape_row(&fields)
}

/// 행 목록 전체를 CSV 텍스트로 직렬화한다(헤더 포함).
pub fn rows_to_csv_text(rows: &[ParsedRow]) -> String {
    let mut out = crate::header::canonical_header_line();
    out.push('\n');
    for row in rows {
        out.push_str(&row_to_csv_line(row));
        out.push('\n');
    }
    out
}

/// 두 행이 SKU/창고/카테고리는 같고 수량 또는 단가만 다른지 검사한다
/// (같은 품목의 재입력/정정으로 보이는 행 쌍을 찾는 데 쓴다).
pub fn looks_like_correction_of(a: &ParsedRow, b: &ParsedRow) -> bool {
    a.sku == b.sku
        && a.warehouse_code == b.warehouse_code
        && a.category == b.category
        && (a.qty != b.qty || a.unit_price_krw != b.unit_price_krw)
}

/// 행 목록 중 단가가 0원인(무상 제공/샘플로 추정되는) 행만 걸러낸다.
pub fn zero_price_rows(rows: &[ParsedRow]) -> Vec<ParsedRow> {
    rows.iter().filter(|r| r.unit_price_krw == 0).cloned().collect()
}

/// 행 목록의 평균 단가를 계산한다(빈 목록이면 0).
pub fn average_unit_price(rows: &[ParsedRow]) -> i64 {
    if rows.is_empty() {
        return 0;
    }
    rows.iter().map(|r| r.unit_price_krw).sum::<i64>() / rows.len() as i64
}

/// 행 하나를 단계적으로 조립하는 빌더. 파싱 실패 후 부분 정보만이라도
/// 확보해 재입력 화면에 되돌려줘야 하는 UI 워크플로에서 유용하다.
#[derive(Debug, Clone, Default)]
pub struct RowBuilder {
    sku: Option<String>,
    warehouse_code: Option<String>,
    qty: Option<i64>,
    unit_price_krw: Option<i64>,
    category: Option<String>,
}

impl RowBuilder {
    pub fn new() -> Self {
        RowBuilder::default()
    }

    pub fn sku(mut self, sku: impl Into<String>) -> Self {
        self.sku = Some(sku.into());
        self
    }

    pub fn warehouse_code(mut self, code: impl Into<String>) -> Self {
        self.warehouse_code = Some(code.into());
        self
    }

    pub fn qty(mut self, qty: i64) -> Self {
        self.qty = Some(qty);
        self
    }

    pub fn unit_price_krw(mut self, price: i64) -> Self {
        self.unit_price_krw = Some(price);
        self
    }

    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// 필요한 필드가 모두 채워졌으면 `ParsedRow`를 만든다.
    pub fn build(self) -> Option<ParsedRow> {
        Some(ParsedRow {
            sku: self.sku?,
            warehouse_code: self.warehouse_code.unwrap_or_default(),
            qty: self.qty?,
            unit_price_krw: self.unit_price_krw?,
            category: self.category.unwrap_or_default(),
        })
    }

    /// 필수 필드(sku/qty/unit_price_krw) 중 아직 비어있는 것이 있는지 검사한다.
    pub fn is_incomplete(&self) -> bool {
        self.sku.is_none() || self.qty.is_none() || self.unit_price_krw.is_none()
    }
}

/// 행 목록에서 소계가 특정 임계값 이상인 것만 걸러낸다(고액 거래 감사용).
pub fn rows_above_subtotal(rows: &[ParsedRow], threshold_krw: i64) -> Vec<ParsedRow> {
    rows.iter().filter(|r| row_subtotal_krw(r) >= threshold_krw).cloned().collect()
}

/// 행 목록을 소계 내림차순으로 정렬한다(동률은 SKU 오름차순).
pub fn sort_by_subtotal_desc(rows: &mut [ParsedRow]) {
    rows.sort_by(|a, b| row_subtotal_krw(b).cmp(&row_subtotal_krw(a)).then_with(|| a.sku.cmp(&b.sku)));
}

/// 파싱 결과(성공/실패 튜플)에서 성공한 행만 다시 추려낸다(파이프라인
/// 중간 단계에서 오류를 버리고 다음 단계로 넘길 때 쓴다).
pub fn oks_only(results: &[(usize, Result<ParsedRow, ParseError>)]) -> Vec<ParsedRow> {
    results.iter().filter_map(|(_, r)| r.as_ref().ok()).cloned().collect()
}

/// 행 목록이 모두 같은 창고 코드를 쓰는지 검사한다(단일 창고 배치인지 여부).
pub fn is_single_warehouse_batch(rows: &[ParsedRow]) -> bool {
    if rows.is_empty() {
        return true;
    }
    let first = &rows[0].warehouse_code;
    rows.iter().all(|r| &r.warehouse_code == first)
}
