//! 창고 모델. 창고 코드/등급/용량을 표현하는 타입과 헬퍼.
//!
//! 창고 등급(Grade)은 배분/보충 규칙과 밀접해 `crate::rules`에 두고,
//! 여기서는 창고 자체의 정적 정보(코드/이름/지역/용량)만 다룬다.

use crate::rules::WarehouseGrade;

/// 창고 한 곳의 정적 정보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warehouse {
    pub code: String,
    pub name: String,
    pub region: String,
    pub capacity: u32,
}

impl Warehouse {
    pub fn new(code: impl Into<String>, name: impl Into<String>, region: impl Into<String>, capacity: u32) -> Self {
        Warehouse { code: code.into(), name: name.into(), region: region.into(), capacity }
    }
}

/// 창고 코드 포맷: 영문 대문자 3자 + 숫자 1자 (예: `SEL1`, `BSN2`).
pub fn is_valid_warehouse_code(code: &str) -> bool {
    let bytes: Vec<char> = code.chars().collect();
    bytes.len() == 4
        && bytes[..3].iter().all(|c| c.is_ascii_uppercase())
        && bytes[3].is_ascii_digit()
}

/// 창고 코드에서 지역 접두 3자를 뽑는다(포맷이 아니면 빈 문자열).
pub fn region_prefix(code: &str) -> String {
    if is_valid_warehouse_code(code) {
        code[..3].to_string()
    } else {
        String::new()
    }
}

/// 두 창고가 같은 지역인지 비교(코드 접두 기준).
pub fn is_same_region(a: &str, b: &str) -> bool {
    let pa = region_prefix(a);
    !pa.is_empty() && pa == region_prefix(b)
}

/// 등급 문자열("central"/"regional"/"local", 대소문자 무관)을 실제 등급으로 변환.
pub fn parse_grade(s: &str) -> Option<WarehouseGrade> {
    match s.to_ascii_lowercase().as_str() {
        "central" => Some(WarehouseGrade::Central),
        "regional" => Some(WarehouseGrade::Regional),
        "local" => Some(WarehouseGrade::Local),
        _ => None,
    }
}

/// 등급을 문자열로 표시한다(로그/보고서용).
pub fn grade_label(grade: &WarehouseGrade) -> &'static str {
    match grade {
        WarehouseGrade::Central => "central",
        WarehouseGrade::Regional => "regional",
        WarehouseGrade::Local => "local",
    }
}

/// 등급별 우선순위(숫자가 작을수록 우선) — 중앙 > 지역 > 로컬.
pub fn grade_priority(grade: &WarehouseGrade) -> u8 {
    match grade {
        WarehouseGrade::Central => 0,
        WarehouseGrade::Regional => 1,
        WarehouseGrade::Local => 2,
    }
}

/// 목록에서 코드로 창고를 찾는다.
pub fn find_by_code<'a>(warehouses: &'a [Warehouse], code: &str) -> Option<&'a Warehouse> {
    warehouses.iter().find(|w| w.code == code)
}

/// 같은 지역에 속한 창고들을 걸러낸다.
pub fn warehouses_in_region<'a>(warehouses: &'a [Warehouse], region: &str) -> Vec<&'a Warehouse> {
    warehouses.iter().filter(|w| w.region == region).collect()
}

/// 창고 목록의 총 용량 합계.
pub fn total_capacity(warehouses: &[Warehouse]) -> u32 {
    warehouses.iter().map(|w| w.capacity).sum()
}

/// 용량이 큰 순서로 정렬한다(동률은 코드 오름차순).
pub fn sort_by_capacity_desc(warehouses: &mut Vec<Warehouse>) {
    warehouses.sort_by(|a, b| b.capacity.cmp(&a.capacity).then_with(|| a.code.cmp(&b.code)));
}

/// 코드 포맷이 유효한 창고만 걸러낸다(데이터 정합성 점검용).
pub fn filter_valid_codes(warehouses: &[Warehouse]) -> Vec<Warehouse> {
    warehouses.iter().filter(|w| is_valid_warehouse_code(&w.code)).cloned().collect()
}

/// 창고 운영 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarehouseStatus {
    Active,
    Maintenance,
    Closed,
}

/// 상태 문자열을 파싱한다.
pub fn parse_status(s: &str) -> Option<WarehouseStatus> {
    match s.to_ascii_uppercase().as_str() {
        "ACTIVE" => Some(WarehouseStatus::Active),
        "MAINTENANCE" => Some(WarehouseStatus::Maintenance),
        "CLOSED" => Some(WarehouseStatus::Closed),
        _ => None,
    }
}

/// 창고가 신규 입고를 받을 수 있는 상태인지(Active만 가능) 판정한다.
pub fn accepts_inbound(status: WarehouseStatus) -> bool {
    matches!(status, WarehouseStatus::Active)
}

/// 창고 이름에 부적절한 특수문자가 없는지 검사한다(영문/숫자/공백/하이픈만 허용).
pub fn is_valid_warehouse_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-')
}

/// 창고 목록에서 코드 목록에 해당하는 것만 순서 유지하며 추린다.
pub fn select_by_codes<'a>(warehouses: &'a [Warehouse], codes: &[String]) -> Vec<&'a Warehouse> {
    codes.iter().filter_map(|code| find_by_code(warehouses, code)).collect()
}

/// 창고 목록의 평균 용량을 계산한다(빈 목록이면 0).
pub fn average_capacity(warehouses: &[Warehouse]) -> u32 {
    if warehouses.is_empty() {
        0
    } else {
        total_capacity(warehouses) / warehouses.len() as u32
    }
}

/// 용량이 지정 범위 안인 창고만 걸러낸다.
pub fn warehouses_with_capacity_between<'a>(warehouses: &'a [Warehouse], min: u32, max: u32) -> Vec<&'a Warehouse> {
    warehouses.iter().filter(|w| w.capacity >= min && w.capacity <= max).collect()
}

/// 지역 목록(중복 제거, 정렬됨)을 반환한다.
pub fn distinct_regions(warehouses: &[Warehouse]) -> Vec<String> {
    let mut regions: Vec<String> = warehouses.iter().map(|w| w.region.clone()).collect();
    regions.sort();
    regions.dedup();
    regions
}

/// 지역별 창고 개수를 센다(지역명 -> 개수 목록, 지역명 오름차순).
pub fn count_per_region(warehouses: &[Warehouse]) -> Vec<(String, usize)> {
    distinct_regions(warehouses)
        .into_iter()
        .map(|region| {
            let count = warehouses.iter().filter(|w| w.region == region).count();
            (region, count)
        })
        .collect()
}

/// 두 창고 코드 사이의 "거리 등급"을 간단히 추정한다: 같은 지역이면 0,
/// 아니면 1 (실제 거리 데이터가 없을 때의 대략적 근사치).
pub fn rough_distance_tier(a: &str, b: &str) -> u8 {
    if is_same_region(a, b) {
        0
    } else {
        1
    }
}

/// 창고 이름으로 대소문자 무시 부분 검색을 한다.
pub fn search_by_name<'a>(warehouses: &'a [Warehouse], query: &str) -> Vec<&'a Warehouse> {
    let q = query.to_ascii_lowercase();
    warehouses.iter().filter(|w| w.name.to_ascii_lowercase().contains(&q)).collect()
}
