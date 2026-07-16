//! 재고 단위(UOM: unit of measure) 변환 헬퍼.
//!
//! EA(낱개)/BOX(박스)/PLT(파렛트) 세 단위 사이의 환산을 다룬다. 실제 환산
//! 계수(박스당 개수, 파렛트당 박스 수)는 SKU별로 다르므로 인자로 받는다.

/// 지원하는 재고 단위.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uom {
    Ea,
    Box,
    Pallet,
}

/// 문자열을 단위로 파싱한다.
pub fn parse_uom(s: &str) -> Option<Uom> {
    match s {
        "EA" => Some(Uom::Ea),
        "BOX" => Some(Uom::Box),
        "PLT" => Some(Uom::Pallet),
        _ => None,
    }
}

/// 단위를 문자열로 표시한다.
pub fn uom_label(uom: Uom) -> &'static str {
    match uom {
        Uom::Ea => "EA",
        Uom::Box => "BOX",
        Uom::Pallet => "PLT",
    }
}

/// EA 단위로 환산한다(박스/파렛트 크기 기준).
pub fn to_ea(qty: u32, uom: Uom, box_size: u32, pallet_size_boxes: u32) -> u32 {
    match uom {
        Uom::Ea => qty,
        Uom::Box => qty.saturating_mul(box_size),
        Uom::Pallet => qty.saturating_mul(pallet_size_boxes).saturating_mul(box_size),
    }
}

/// EA 수량을 다른 단위로 환산한다(내림, 즉 부분 박스/파렛트는 버림).
pub fn from_ea(qty_ea: u32, target: Uom, box_size: u32, pallet_size_boxes: u32) -> u32 {
    match target {
        Uom::Ea => qty_ea,
        Uom::Box => {
            if box_size == 0 {
                0
            } else {
                qty_ea / box_size
            }
        }
        Uom::Pallet => {
            let ea_per_pallet = box_size.saturating_mul(pallet_size_boxes);
            if ea_per_pallet == 0 {
                0
            } else {
                qty_ea / ea_per_pallet
            }
        }
    }
}

/// 환산 계수(박스당 EA)가 유효한지(0보다 큼) 검사한다.
pub fn is_valid_box_size(box_size: u32) -> bool {
    box_size > 0
}

/// 환산 계수(파렛트당 박스)가 유효한지 검사한다.
pub fn is_valid_pallet_size(pallet_size_boxes: u32) -> bool {
    pallet_size_boxes > 0
}

/// 단위 코드 문자열이 알려진 값인지 검사한다.
pub fn is_valid_uom_str(s: &str) -> bool {
    parse_uom(s).is_some()
}

/// 두 단위 간 직접 환산 계수가 정수인지(단수 없이 딱 나뉘는지) 검사한다.
///
/// 예: EA -> BOX 환산 시 box_size로 나누어떨어지는지 확인할 때 쓴다.
pub fn is_exact_multiple(qty: u32, divisor: u32) -> bool {
    divisor != 0 && qty % divisor == 0
}

/// EA 수량이 박스로 나누어떨어지지 않을 때 남는 낱개 수(잔여분)를 계산한다.
pub fn leftover_ea_after_boxing(qty_ea: u32, box_size: u32) -> u32 {
    if box_size == 0 {
        qty_ea
    } else {
        qty_ea % box_size
    }
}

/// 무게 단위(g/kg)를 변환한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightUnit {
    Gram,
    Kilogram,
}

/// 그램 단위 무게를 지정한 단위로 환산한다(정수 연산, kg는 내림).
pub fn convert_weight(grams: u32, target: WeightUnit) -> u32 {
    match target {
        WeightUnit::Gram => grams,
        WeightUnit::Kilogram => grams / 1000,
    }
}

/// 부피 단위(리터 기준) 환산 — 파렛트 적재 부피 계산에 쓰인다.
pub fn liters_to_pallets(liters: u32, liters_per_pallet: u32) -> u32 {
    if liters_per_pallet == 0 {
        0
    } else {
        (liters + liters_per_pallet - 1) / liters_per_pallet
    }
}

/// 두 단위가 같은 종류(EA/BOX/PLT)인지 비교한다(단순 동등 비교의 명시적 래퍼).
pub fn is_same_uom(a: Uom, b: Uom) -> bool {
    a == b
}

/// 단위 목록에서 EA가 아닌(포장 단위) 것만 골라낸다.
pub fn packaging_units() -> Vec<Uom> {
    vec![Uom::Box, Uom::Pallet]
}

/// 주어진 EA 수량이 최소 1박스 이상인지 검사한다.
pub fn at_least_one_box(qty_ea: u32, box_size: u32) -> bool {
    box_size > 0 && qty_ea >= box_size
}

/// 박스 크기와 파렛트당 박스 수로 파렛트당 EA 수를 계산한다.
pub fn ea_per_pallet(box_size: u32, pallet_size_boxes: u32) -> u32 {
    box_size.saturating_mul(pallet_size_boxes)
}
