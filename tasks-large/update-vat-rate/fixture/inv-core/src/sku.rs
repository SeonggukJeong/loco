//! SKU 코드 파싱/검증.
//!
//! SKU 포맷 규칙: `<카테고리 2자리>-<일련번호 6자리>[-<변형 코드>]`
//! 예) `EL-000123`, `EL-000123-BLK`. 이 파일은 이 포맷을 파싱/검증하는
//! 순수 헬퍼만 담는다 — 재고 수량이나 가격은 다루지 않는다.

/// 파싱된 SKU의 구성 요소.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSku {
    pub category: String,
    pub serial: String,
    pub variant: Option<String>,
}

/// SKU 문자열을 파싱한다. 포맷을 벗어나면 `None`.
pub fn parse_sku(raw: &str) -> Option<ParsedSku> {
    let parts: Vec<&str> = raw.split('-').collect();
    match parts.as_slice() {
        [category, serial] => {
            if is_valid_category(category) && is_valid_serial(serial) {
                Some(ParsedSku { category: category.to_string(), serial: serial.to_string(), variant: None })
            } else {
                None
            }
        }
        [category, serial, variant] => {
            if is_valid_category(category) && is_valid_serial(serial) && is_valid_variant(variant) {
                Some(ParsedSku {
                    category: category.to_string(),
                    serial: serial.to_string(),
                    variant: Some(variant.to_string()),
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 카테고리 코드: 영문 대문자 2자.
pub fn is_valid_category(s: &str) -> bool {
    s.len() == 2 && s.chars().all(|c| c.is_ascii_uppercase())
}

/// 일련번호: 숫자 6자.
pub fn is_valid_serial(s: &str) -> bool {
    s.len() == 6 && s.chars().all(|c| c.is_ascii_digit())
}

/// 변형 코드: 영문 대문자 2~4자.
pub fn is_valid_variant(s: &str) -> bool {
    (2..=4).contains(&s.len()) && s.chars().all(|c| c.is_ascii_uppercase())
}

/// 전체 SKU 문자열이 유효한 포맷인지 검사(파싱 성공 여부와 동일).
pub fn is_valid_sku(raw: &str) -> bool {
    parse_sku(raw).is_some()
}

/// SKU에서 카테고리 코드만 뽑아낸다(파싱 실패 시 빈 문자열).
pub fn category_of(raw: &str) -> String {
    parse_sku(raw).map(|p| p.category).unwrap_or_default()
}

/// 두 SKU가 같은 카테고리에 속하는지 비교한다.
pub fn same_category(a: &str, b: &str) -> bool {
    match (parse_sku(a), parse_sku(b)) {
        (Some(pa), Some(pb)) => pa.category == pb.category,
        _ => false,
    }
}

/// 변형이 있는 SKU인지 여부.
pub fn has_variant(raw: &str) -> bool {
    parse_sku(raw).map(|p| p.variant.is_some()).unwrap_or(false)
}

/// SKU 목록 중 유효한 것만 걸러낸다.
pub fn filter_valid(skus: &[String]) -> Vec<String> {
    skus.iter().filter(|s| is_valid_sku(s)).cloned().collect()
}

/// SKU 목록 중 잘못된 것만 걸러낸다(데이터 정합성 점검용).
pub fn filter_invalid(skus: &[String]) -> Vec<String> {
    skus.iter().filter(|s| !is_valid_sku(s)).cloned().collect()
}

/// 일련번호를 6자리로 0-패딩한다(파싱 전 정규화에 사용).
pub fn pad_serial(serial: &str) -> String {
    if serial.len() >= 6 {
        serial.to_string()
    } else {
        format!("{:0>6}", serial)
    }
}

/// 카테고리 + 일련번호 + (선택) 변형으로 SKU 문자열을 조립한다.
pub fn build_sku(category: &str, serial: &str, variant: Option<&str>) -> String {
    match variant {
        Some(v) => format!("{category}-{serial}-{v}"),
        None => format!("{category}-{serial}"),
    }
}

/// 바코드(GTIN 스타일 13자리 숫자) 형식이 유효한지 검사한다.
///
/// 체크섬까지는 검증하지 않는다(단순 자릿수/숫자 여부만) — 체크섬 검증은
/// inv-parse 쪽에서 입력 데이터 정제 시 별도로 수행한다.
pub fn is_valid_barcode(barcode: &str) -> bool {
    barcode.len() == 13 && barcode.chars().all(|c| c.is_ascii_digit())
}

/// SKU 목록을 카테고리별로 그룹핑한다(카테고리 코드 -> SKU 목록).
pub fn group_by_category(skus: &[String]) -> Vec<(String, Vec<String>)> {
    let mut categories: Vec<String> = skus.iter().map(|s| category_of(s)).collect();
    categories.sort();
    categories.dedup();
    categories
        .into_iter()
        .filter(|c| !c.is_empty())
        .map(|cat| {
            let members: Vec<String> = skus.iter().filter(|s| category_of(s) == cat).cloned().collect();
            (cat, members)
        })
        .collect()
}

/// SKU 목록에서 특정 변형 코드를 가진 것만 걸러낸다.
pub fn filter_by_variant<'a>(skus: &'a [String], variant: &str) -> Vec<&'a String> {
    skus.iter()
        .filter(|s| parse_sku(s).and_then(|p| p.variant).as_deref() == Some(variant))
        .collect()
}

/// SKU 목록 중 특정 접두(카테고리)로 시작하는 것만 걸러낸다.
pub fn filter_by_category_prefix<'a>(skus: &'a [String], category: &str) -> Vec<&'a String> {
    skus.iter().filter(|s| s.starts_with(category)).collect()
}

/// 일련번호 부분만 정수로 파싱한다(포맷이 아니면 `None`).
pub fn serial_as_number(raw: &str) -> Option<u32> {
    parse_sku(raw).and_then(|p| p.serial.parse::<u32>().ok())
}

/// 동일 카테고리 내에서 다음 일련번호를 계산한다(기존 SKU 중 최댓값 + 1).
pub fn next_serial_in_category(skus: &[String], category: &str) -> u32 {
    skus.iter()
        .filter(|s| category_of(s) == category)
        .filter_map(|s| serial_as_number(s))
        .max()
        .map(|n| n + 1)
        .unwrap_or(1)
}

/// 두 SKU를 카테고리 -> 일련번호 순으로 비교한다(정렬용 비교자).
pub fn compare_skus(a: &str, b: &str) -> std::cmp::Ordering {
    match (parse_sku(a), parse_sku(b)) {
        (Some(pa), Some(pb)) => pa.category.cmp(&pb.category).then_with(|| pa.serial.cmp(&pb.serial)),
        _ => a.cmp(b),
    }
}

/// SKU 목록을 카테고리/일련번호 기준으로 정렬한다.
pub fn sort_skus(skus: &mut Vec<String>) {
    skus.sort_by(|a, b| compare_skus(a, b));
}

/// SKU 문자열에서 공백/제어문자를 제거해 정규화한다(파싱 전 전처리용).
pub fn normalize_sku_input(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_uppercase()
}

/// 두 SKU 목록의 교집합(양쪽 모두에 존재하는 SKU)을 구한다.
pub fn intersect(a: &[String], b: &[String]) -> Vec<String> {
    let mut result: Vec<String> = a.iter().filter(|s| b.contains(s)).cloned().collect();
    result.sort();
    result.dedup();
    result
}

/// 첫 번째 목록에만 있고 두 번째 목록에는 없는 SKU를 구한다(차집합).
pub fn difference(a: &[String], b: &[String]) -> Vec<String> {
    let mut result: Vec<String> = a.iter().filter(|s| !b.contains(s)).cloned().collect();
    result.sort();
    result.dedup();
    result
}
