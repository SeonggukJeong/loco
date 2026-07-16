//! 원장(ledger) 라인 타입과 정렬/필터 헬퍼.
//!
//! `LedgerLine`은 판매(Sale)/환불(Refund)/조정(Adjustment) 세 종류의 거래를
//! 표현하는 코어 타입이다. 이 타입은 inv-report의 합계 함수들이 소비하지만,
//! 합계 로직 자체는 이 크레이트에 두지 않는다(코어는 타입과 순수 헬퍼만).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineKind { Sale, Refund, Adjustment }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerLine { pub sku: String, pub kind: LineKind, pub amount_krw: i64, pub adj_krw: i64 }
impl LedgerLine { pub fn adjustment_krw(&self) -> i64 { self.adj_krw } }

impl LedgerLine {
    /// 새 원장 라인을 만든다. `adj_krw`는 Adjustment 종류가 아니면 보통 0이다.
    pub fn new(sku: impl Into<String>, kind: LineKind, amount_krw: i64, adj_krw: i64) -> Self {
        LedgerLine { sku: sku.into(), kind, amount_krw, adj_krw }
    }

    /// 이 라인이 특정 SKU에 속하는지 확인한다.
    pub fn is_for_sku(&self, sku: &str) -> bool {
        self.sku == sku
    }

    /// 이 라인이 환불 라인인지 여부.
    pub fn is_refund(&self) -> bool {
        matches!(self.kind, LineKind::Refund)
    }
}

/// SKU 문자열 오름차순으로 정렬한다(안정 정렬, 동일 SKU는 원래 순서 유지).
///
/// 판정 테스트가 참조하는 헬퍼 — 합계 값이 아니라 "정렬 순서"만 검증 대상이다.
pub fn sort_by_sku(lines: &mut Vec<LedgerLine>) {
    lines.sort_by(|a, b| a.sku.cmp(&b.sku));
}

/// `amount_krw` 내림차순으로 정렬한다(동률은 SKU 오름차순으로 2차 정렬).
pub fn sort_by_amount_desc(lines: &mut Vec<LedgerLine>) {
    lines.sort_by(|a, b| b.amount_krw.cmp(&a.amount_krw).then_with(|| a.sku.cmp(&b.sku)));
}

/// 지정된 종류(kind)의 라인만 걸러낸다.
pub fn filter_by_kind(lines: &[LedgerLine], kind: &LineKind) -> Vec<LedgerLine> {
    lines.iter().filter(|l| &l.kind == kind).cloned().collect()
}

/// 지정된 SKU의 라인만 걸러낸다.
pub fn filter_by_sku<'a>(lines: &'a [LedgerLine], sku: &str) -> Vec<&'a LedgerLine> {
    lines.iter().filter(|l| l.sku == sku).collect()
}

/// 원장에 등장하는 고유 SKU 목록을 정렬된 상태로 반환한다(중복 제거).
pub fn distinct_skus(lines: &[LedgerLine]) -> Vec<String> {
    let mut skus: Vec<String> = lines.iter().map(|l| l.sku.clone()).collect();
    skus.sort();
    skus.dedup();
    skus
}

/// 라인 개수를 종류별로 센다: (판매, 환불, 조정) 순서의 튜플.
pub fn count_by_kind(lines: &[LedgerLine]) -> (usize, usize, usize) {
    let mut sale = 0usize;
    let mut refund = 0usize;
    let mut adjustment = 0usize;
    for line in lines {
        match line.kind {
            LineKind::Sale => sale += 1,
            LineKind::Refund => refund += 1,
            LineKind::Adjustment => adjustment += 1,
        }
    }
    (sale, refund, adjustment)
}

/// 원장 라인 목록이 비어 있지 않고 모든 SKU가 공백이 아닌지 검사한다.
///
/// 파싱 직후의 최소 무결성 체크로 쓰인다 — 세율/합계 계산과는 무관하다.
pub fn is_well_formed(lines: &[LedgerLine]) -> bool {
    !lines.is_empty() && lines.iter().all(|l| !l.sku.trim().is_empty())
}

/// 원장 라인 중 금액이 음수인 것만 걸러낸다(비정상 입력 탐지용 — 정상
/// 원장에서는 `amount_krw`가 항상 0 이상이어야 한다는 전제를 점검한다).
pub fn negative_amount_lines(lines: &[LedgerLine]) -> Vec<LedgerLine> {
    lines.iter().filter(|l| l.amount_krw < 0).cloned().collect()
}

/// 두 원장 라인 목록을 SKU 기준으로 이어붙인다(단순 연결, 중복 제거 없음).
pub fn concat(a: &[LedgerLine], b: &[LedgerLine]) -> Vec<LedgerLine> {
    let mut combined = a.to_vec();
    combined.extend_from_slice(b);
    combined
}

/// 원장에서 특정 SKU의 라인들만 제거한 새 목록을 만든다.
pub fn without_sku(lines: &[LedgerLine], sku: &str) -> Vec<LedgerLine> {
    lines.iter().filter(|l| l.sku != sku).cloned().collect()
}

/// 원장 라인 목록을 종류(kind) -> 라인 개수 맵으로 요약한다(디버그 출력용).
pub fn summarize_kinds(lines: &[LedgerLine]) -> Vec<(String, usize)> {
    let (sale, refund, adjustment) = count_by_kind(lines);
    vec![
        ("Sale".to_string(), sale),
        ("Refund".to_string(), refund),
        ("Adjustment".to_string(), adjustment),
    ]
}

/// 원장에서 조정(Adjustment) 라인만 걸러낸다(가장 흔히 재사용되는 필터라
/// 별도 편의 함수로 둔다).
pub fn adjustment_lines(lines: &[LedgerLine]) -> Vec<LedgerLine> {
    filter_by_kind(lines, &LineKind::Adjustment)
}

/// 원장 라인 개수가 배치 처리 상한을 넘는지 검사한다.
pub fn exceeds_batch_limit(lines: &[LedgerLine], limit: usize) -> bool {
    lines.len() > limit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_by_sku_orders_ascending() {
        let mut lines = vec![
            LedgerLine::new("SKU-B", LineKind::Sale, 1000, 0),
            LedgerLine::new("SKU-A", LineKind::Sale, 2000, 0),
        ];
        sort_by_sku(&mut lines);
        assert_eq!(lines[0].sku, "SKU-A");
        assert_eq!(lines[1].sku, "SKU-B");
    }

    #[test]
    fn distinct_skus_dedups_and_sorts() {
        let lines = vec![
            LedgerLine::new("SKU-B", LineKind::Sale, 1000, 0),
            LedgerLine::new("SKU-A", LineKind::Refund, 500, 0),
            LedgerLine::new("SKU-B", LineKind::Adjustment, 0, 10),
        ];
        assert_eq!(distinct_skus(&lines), vec!["SKU-A".to_string(), "SKU-B".to_string()]);
    }

    #[test]
    fn count_by_kind_tally() {
        let lines = vec![
            LedgerLine::new("A", LineKind::Sale, 1, 0),
            LedgerLine::new("A", LineKind::Sale, 1, 0),
            LedgerLine::new("A", LineKind::Refund, 1, 0),
        ];
        assert_eq!(count_by_kind(&lines), (2, 1, 0));
    }
}
