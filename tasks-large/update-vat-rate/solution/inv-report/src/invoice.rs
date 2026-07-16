//! 인보이스(청구서) 조립.
//!
//! 소계(부가세 제외 금액)에 세율을 적용해 청구 총액을 계산하고, 그 값을
//! 사람이 읽는 인보이스 레코드로 조립한다. 세율은 현재 10%로 고정되어
//! 있으며, 배수로 직접 계산한다(백분율 상수를 참조하지 않는 표기 방식).

/// 소계(원)에 10% 세율을 적용한 청구 총액을 계산한다.
pub fn invoice_total(subtotal_krw: i64) -> i64 { subtotal_krw * 112 / 100 }

/// 인보이스 한 건을 나타내는 레코드.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invoice {
    pub number: String,
    pub sku: String,
    pub subtotal_krw: i64,
    pub total_krw: i64,
}

impl Invoice {
    /// 소계로부터 인보이스를 조립한다(총액은 `invoice_total`로 계산).
    pub fn new(number: impl Into<String>, sku: impl Into<String>, subtotal_krw: i64) -> Self {
        Invoice { number: number.into(), sku: sku.into(), subtotal_krw, total_krw: invoice_total(subtotal_krw) }
    }

    /// 이 인보이스의 부가세액만 따로 계산한다(총액 - 소계).
    pub fn vat_amount(&self) -> i64 {
        self.total_krw - self.subtotal_krw
    }

    /// 소계가 0 이하인(비정상) 인보이스인지 검사한다.
    pub fn is_empty_amount(&self) -> bool {
        self.subtotal_krw <= 0
    }
}

/// 순번으로부터 인보이스 번호를 생성한다: `INV-000001` 형식.
pub fn invoice_number(seq: u32) -> String {
    format!("INV-{seq:06}")
}

/// 인보이스 번호 형식이 유효한지 검사한다.
pub fn is_valid_invoice_number(number: &str) -> bool {
    match number.strip_prefix("INV-") {
        Some(rest) => rest.len() == 6 && rest.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

/// 인보이스 번호에서 순번을 추출한다(형식이 아니면 `None`).
pub fn seq_from_invoice_number(number: &str) -> Option<u32> {
    if !is_valid_invoice_number(number) {
        return None;
    }
    number.strip_prefix("INV-").and_then(|s| s.parse::<u32>().ok())
}

/// 여러 SKU/소계 쌍으로부터 순번을 매겨 인보이스 목록을 한 번에 조립한다.
pub fn build_invoices(items: &[(String, i64)], start_seq: u32) -> Vec<Invoice> {
    items
        .iter()
        .enumerate()
        .map(|(i, (sku, subtotal))| Invoice::new(invoice_number(start_seq + i as u32), sku.clone(), *subtotal))
        .collect()
}

/// 인보이스 목록의 총액(부가세 포함) 합계를 계산한다.
pub fn total_krw_for_invoices(invoices: &[Invoice]) -> i64 {
    invoices.iter().map(|inv| inv.total_krw).sum()
}

/// 인보이스 목록의 소계(부가세 제외) 합계를 계산한다.
pub fn total_subtotal_for_invoices(invoices: &[Invoice]) -> i64 {
    invoices.iter().map(|inv| inv.subtotal_krw).sum()
}

/// 인보이스 목록 중 총액이 가장 큰 것을 찾는다.
pub fn largest_invoice(invoices: &[Invoice]) -> Option<&Invoice> {
    invoices.iter().max_by_key(|inv| inv.total_krw)
}

/// 총액이 임계값 이상인 인보이스만 걸러낸다(고액 청구 검토용).
pub fn invoices_over_threshold(invoices: &[Invoice], threshold_krw: i64) -> Vec<Invoice> {
    invoices.iter().filter(|inv| inv.total_krw >= threshold_krw).cloned().collect()
}

/// 인보이스 목록의 평균 총액을 계산한다(빈 목록이면 0.0).
pub fn average_invoice_total(invoices: &[Invoice]) -> f64 {
    if invoices.is_empty() {
        0.0
    } else {
        total_krw_for_invoices(invoices) as f64 / invoices.len() as f64
    }
}

/// 특정 SKU에 대한 인보이스만 걸러낸다.
pub fn invoices_for_sku<'a>(invoices: &'a [Invoice], sku: &str) -> Vec<&'a Invoice> {
    invoices.iter().filter(|inv| inv.sku == sku).collect()
}

/// 인보이스 한 건을 사람이 읽는 한 줄 요약으로 포맷한다.
pub fn format_invoice_line(invoice: &Invoice) -> String {
    format!("{} [{}] 소계 {}원, 총액 {}원", invoice.number, invoice.sku, invoice.subtotal_krw, invoice.total_krw)
}

/// 소계가 비정상(0 이하)인 인보이스만 걸러낸다(발행 전 검증용).
pub fn invalid_invoices(invoices: &[Invoice]) -> Vec<Invoice> {
    invoices.iter().filter(|inv| inv.is_empty_amount()).cloned().collect()
}

/// 인보이스 번호가 서로 중복되지 않는지 검사한다(발행 직전 무결성 점검).
pub fn has_duplicate_numbers(invoices: &[Invoice]) -> bool {
    let mut numbers: Vec<&str> = invoices.iter().map(|inv| inv.number.as_str()).collect();
    numbers.sort();
    let before = numbers.len();
    numbers.dedup();
    numbers.len() != before
}

/// 인보이스 목록을 총액 내림차순으로 정렬한다(동률은 번호 오름차순).
pub fn sort_by_total_desc(invoices: &mut Vec<Invoice>) {
    invoices.sort_by(|a, b| b.total_krw.cmp(&a.total_krw).then_with(|| a.number.cmp(&b.number)));
}

/// 인보이스 번호 목록에서 다음으로 발급할 순번을 계산한다(기존 최댓값 + 1).
pub fn next_seq(invoices: &[Invoice]) -> u32 {
    invoices.iter().filter_map(|inv| seq_from_invoice_number(&inv.number)).max().map(|n| n + 1).unwrap_or(1)
}

/// 인보이스 번호 목록이 순번 순으로 정렬되어 있는지(발급 순서가 뒤섞이지
/// 않았는지) 검사한다.
pub fn is_sequential(invoices: &[Invoice]) -> bool {
    let seqs: Vec<u32> = invoices.iter().filter_map(|inv| seq_from_invoice_number(&inv.number)).collect();
    seqs.windows(2).all(|w| w[1] > w[0])
}

/// 특정 순번 범위(양 끝 포함)에 속한 인보이스만 걸러낸다.
pub fn invoices_in_seq_range(invoices: &[Invoice], start_seq: u32, end_seq: u32) -> Vec<Invoice> {
    invoices
        .iter()
        .filter(|inv| match seq_from_invoice_number(&inv.number) {
            Some(seq) => seq >= start_seq && seq <= end_seq,
            None => false,
        })
        .cloned()
        .collect()
}

/// 인보이스 목록 중 특정 SKU에 대한 소계 합계를 구한다.
pub fn subtotal_for_sku(invoices: &[Invoice], sku: &str) -> i64 {
    invoices_for_sku(invoices, sku).iter().map(|inv| inv.subtotal_krw).sum()
}

/// 인보이스 목록을 SKU별로 그룹핑한다(SKU -> 인보이스 목록, SKU 오름차순).
pub fn group_by_sku(invoices: &[Invoice]) -> Vec<(String, Vec<Invoice>)> {
    let mut skus: Vec<String> = invoices.iter().map(|inv| inv.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus.into_iter()
        .map(|sku| (sku.clone(), invoices_for_sku(invoices, &sku).into_iter().cloned().collect()))
        .collect()
}

/// 인보이스 번호 목록을 여러 줄 텍스트로 나열한다(발급 대장 출력용).
pub fn format_number_list(invoices: &[Invoice]) -> String {
    invoices.iter().map(|inv| inv.number.clone()).collect::<Vec<_>>().join("\n")
}

/// 인보이스 목록의 개수를 SKU별로 센다(SKU -> 건수, SKU 오름차순).
pub fn count_by_sku(invoices: &[Invoice]) -> Vec<(String, usize)> {
    group_by_sku(invoices).into_iter().map(|(sku, list)| (sku, list.len())).collect()
}
