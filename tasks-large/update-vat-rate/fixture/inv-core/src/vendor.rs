//! 공급업체(vendor) 모델.
//!
//! `rules::mod`의 벤더 리스크/우대 판정 규칙과 짝을 이루는 데이터 타입이다.
//! 이 파일은 타입 정의와 조회 헬퍼만 갖고, 리스크 판정 로직 자체는
//! `rules::mod`에 있다(도메인 타입과 규칙 로직을 분리하는 사내 컨벤션).

/// 공급업체 한 곳의 정보.
#[derive(Debug, Clone, PartialEq)]
pub struct Vendor {
    pub id: String,
    pub name: String,
    pub lead_time_days: u32,
    pub reliability_score: f64,
}

impl Vendor {
    pub fn new(id: impl Into<String>, name: impl Into<String>, lead_time_days: u32, reliability_score: f64) -> Self {
        Vendor { id: id.into(), name: name.into(), lead_time_days, reliability_score: reliability_score.clamp(0.0, 1.0) }
    }
}

/// 벤더 ID 포맷이 유효한지 검사한다: "V" + 숫자 5자리.
pub fn is_valid_vendor_id(id: &str) -> bool {
    id.len() == 6 && id.starts_with('V') && id[1..].chars().all(|c| c.is_ascii_digit())
}

/// 목록에서 ID로 벤더를 찾는다.
pub fn find_by_id<'a>(vendors: &'a [Vendor], id: &str) -> Option<&'a Vendor> {
    vendors.iter().find(|v| v.id == id)
}

/// 신뢰도 점수 내림차순으로 정렬한다(동률은 리드타임 오름차순).
pub fn sort_by_reliability_desc(vendors: &mut Vec<Vendor>) {
    vendors.sort_by(|a, b| {
        b.reliability_score
            .partial_cmp(&a.reliability_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.lead_time_days.cmp(&b.lead_time_days))
    });
}

/// 리드타임이 기준 이하인 벤더만 걸러낸다.
pub fn vendors_within_lead_time<'a>(vendors: &'a [Vendor], max_days: u32) -> Vec<&'a Vendor> {
    vendors.iter().filter(|v| v.lead_time_days <= max_days).collect()
}

/// 신뢰도 점수의 평균을 계산한다(빈 목록이면 0.0).
pub fn average_reliability(vendors: &[Vendor]) -> f64 {
    if vendors.is_empty() {
        return 0.0;
    }
    let sum: f64 = vendors.iter().map(|v| v.reliability_score).sum();
    sum / vendors.len() as f64
}

/// 가장 신뢰도가 높은 벤더를 찾는다.
pub fn most_reliable(vendors: &[Vendor]) -> Option<&Vendor> {
    vendors.iter().max_by(|a, b| {
        a.reliability_score.partial_cmp(&b.reliability_score).unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// 벤더 이름으로 대소문자 무시 부분 검색을 한다.
pub fn search_by_name<'a>(vendors: &'a [Vendor], query: &str) -> Vec<&'a Vendor> {
    let q = query.to_ascii_lowercase();
    vendors.iter().filter(|v| v.name.to_ascii_lowercase().contains(&q)).collect()
}

/// 벤더 목록에 중복 ID가 없는지 검사한다(데이터 정합성 점검용).
pub fn has_duplicate_ids(vendors: &[Vendor]) -> bool {
    let mut ids: Vec<&str> = vendors.iter().map(|v| v.id.as_str()).collect();
    ids.sort();
    let before = ids.len();
    ids.dedup();
    ids.len() != before
}

/// 신뢰도 점수가 임계값 미만인 벤더만 걸러낸다("주의 벤더" 목록).
pub fn low_reliability_vendors(vendors: &[Vendor], threshold: f64) -> Vec<Vendor> {
    vendors.iter().filter(|v| v.reliability_score < threshold).cloned().collect()
}

/// 벤더 목록을 리드타임 오름차순으로 정렬한다.
pub fn sort_by_lead_time(vendors: &mut Vec<Vendor>) {
    vendors.sort_by_key(|v| v.lead_time_days);
}

/// 벤더의 신뢰도 점수를 갱신한다(0.0~1.0으로 clamp). 대상 벤더가 없으면
/// false를 반환한다.
pub fn update_reliability(vendors: &mut [Vendor], id: &str, new_score: f64) -> bool {
    if let Some(v) = vendors.iter_mut().find(|v| v.id == id) {
        v.reliability_score = new_score.clamp(0.0, 1.0);
        true
    } else {
        false
    }
}

/// 벤더 목록의 평균 리드타임을 계산한다(빈 목록이면 0.0).
pub fn average_lead_time(vendors: &[Vendor]) -> f64 {
    if vendors.is_empty() {
        return 0.0;
    }
    let sum: u32 = vendors.iter().map(|v| v.lead_time_days).sum();
    sum as f64 / vendors.len() as f64
}

/// 벤더 ID 목록으로 여러 벤더를 한 번에 조회한다(순서 유지, 없는 ID는 제외).
pub fn find_many<'a>(vendors: &'a [Vendor], ids: &[String]) -> Vec<&'a Vendor> {
    ids.iter().filter_map(|id| find_by_id(vendors, id)).collect()
}
