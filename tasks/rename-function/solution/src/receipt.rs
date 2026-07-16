use crate::cart::price_total;

/// 영수증 한 줄 요약
pub fn summary(items: &[(u32, u32)]) -> String {
    format!("total: {}", price_total(items))
}

/// 배송비 포함 합계 (5000 미만이면 배송비 500)
pub fn with_shipping(items: &[(u32, u32)]) -> u32 {
    let t = price_total(items);
    if t < 5000 { t + 500 } else { t }
}
