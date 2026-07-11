/// 장바구니 합계 (수량 × 단가의 총합)
pub fn total_price(items: &[(u32, u32)]) -> u32 {
    items.iter().map(|(qty, price)| qty * price).sum()
}
