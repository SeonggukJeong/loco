//! 가격 관련 규칙: 세율 적용 및 할인 규칙.
//!
//! 부가세를 적용해 최종 청구 금액을 계산한다.
pub fn apply_tax(amount_krw: i64) -> i64 { amount_krw + amount_krw * 10 / 100 }

/// 대량 구매 할인율(%)을 수량 구간별로 결정한다.
///
/// - 100개 이상: 15%
/// - 50개 이상: 10%
/// - 10개 이상: 5%
/// - 그 미만: 0%
pub fn bulk_discount_percent(qty: u32) -> u32 {
    if qty >= 100 {
        15
    } else if qty >= 50 {
        10
    } else if qty >= 10 {
        5
    } else {
        0
    }
}

/// 할인율(%)을 금액에 적용한 결과(원 단위 절사).
pub fn apply_discount(amount_krw: i64, discount_percent: u32) -> i64 {
    amount_krw - (amount_krw * discount_percent as i64 / 100)
}

/// 대량 구매 할인을 금액에 바로 적용하는 편의 함수.
pub fn apply_bulk_discount(amount_krw: i64, qty: u32) -> i64 {
    apply_discount(amount_krw, bulk_discount_percent(qty))
}

/// 로열티 등급별 추가 할인율(%). 등급 코드는 "GOLD"/"SILVER"/"BRONZE" 중 하나.
pub fn loyalty_discount_percent(tier: &str) -> u32 {
    match tier {
        "GOLD" => 8,
        "SILVER" => 4,
        "BRONZE" => 1,
        _ => 0,
    }
}

/// 대량 구매 할인과 로열티 할인을 순차 적용한다(할인은 누적 적용, 복리 아님).
pub fn apply_combined_discount(amount_krw: i64, qty: u32, tier: &str) -> i64 {
    let after_bulk = apply_bulk_discount(amount_krw, qty);
    apply_discount(after_bulk, loyalty_discount_percent(tier))
}

/// 금액이 음수가 되지 않도록 보정한다(할인 누적 후 방어적 clamp).
pub fn non_negative(amount_krw: i64) -> i64 {
    amount_krw.max(0)
}

/// 쿠폰 할인(정액, 원 단위)을 적용한다. 금액이 쿠폰 금액보다 작으면 0으로
/// 클램프한다(마이너스 청구 방지).
pub fn apply_flat_coupon(amount_krw: i64, coupon_krw: i64) -> i64 {
    non_negative(amount_krw - coupon_krw)
}

/// 프로모션 종류에 따른 추가 할인율(%)을 반환한다.
///
/// "FLASH"(타임세일)가 가장 크고, "BUNDLE"(묶음 구매), "CLEARANCE"(재고
/// 정리) 순으로 낮아진다. 알 수 없는 코드는 할인 없음(0)으로 처리한다.
pub fn promotion_discount_percent(promo_code: &str) -> u32 {
    match promo_code {
        "FLASH" => 20,
        "BUNDLE" => 12,
        "CLEARANCE" => 25,
        _ => 0,
    }
}

/// 두 할인율 중 더 큰 쪽만 적용한다(중복 할인 방지 — 사내 정책상 프로모션은
/// 중첩되지 않고 "더 유리한 하나"만 선택된다).
pub fn best_of_two_discounts(a_percent: u32, b_percent: u32) -> u32 {
    a_percent.max(b_percent)
}

/// 최소 판매가(원가 대비 마진율 하한)를 계산한다.
///
/// `min_margin_percent`는 원가 대비 최소 마진율(%)이다. 예: 원가 1000원,
/// 최소 마진 20% -> 최소 판매가 1200원.
pub fn minimum_sale_price(cost_krw: i64, min_margin_percent: u32) -> i64 {
    cost_krw + (cost_krw * min_margin_percent as i64 / 100)
}

/// 제안 판매가가 최소 마진 기준을 충족하는지 검사한다.
pub fn meets_minimum_margin(cost_krw: i64, proposed_price_krw: i64, min_margin_percent: u32) -> bool {
    proposed_price_krw >= minimum_sale_price(cost_krw, min_margin_percent)
}

/// 할인율을 백분율 문자열로 포맷한다(예: 15 -> "15% 할인").
pub fn format_discount_label(discount_percent: u32) -> String {
    format!("{discount_percent}% 할인")
}

/// 마진율(%)을 원가와 판매가로부터 역산한다(판매가가 0 이하면 0을 반환).
pub fn margin_percent_from_prices(cost_krw: i64, sale_price_krw: i64) -> i64 {
    if sale_price_krw <= 0 {
        return 0;
    }
    ((sale_price_krw - cost_krw) * 100) / sale_price_krw
}

/// 여러 상품의 총 할인 전 금액과 할인 후 금액을 비교해 절감액 합계를
/// 계산한다.
pub fn total_savings(before_after_pairs: &[(i64, i64)]) -> i64 {
    before_after_pairs.iter().map(|(before, after)| (before - after).max(0)).sum()
}

/// 가격이 심리적 가격(예: 990, 9900 등 9로 끝나는 값)인지 검사한다.
///
/// 마케팅 규칙 검증용 — 정가 정책을 지키고 있는지 확인할 때 쓴다.
pub fn is_psychological_price(amount_krw: i64) -> bool {
    amount_krw > 0 && amount_krw % 10 == 9
}
