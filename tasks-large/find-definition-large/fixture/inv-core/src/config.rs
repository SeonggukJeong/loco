//! 코어 설정 타입과 재시도/배치 관련 전역 상수.
//!
//! 일시적 오류(네트워크 지연, 락 경합 등) 발생 시 코어 도메인 연산을 몇
//! 번까지 재시도할지 정하는 상한이다.
pub const MAX_RETRY: u32 = 3;

/// 기본 배치 크기(한 번에 처리할 원장 라인 수).
pub const DEFAULT_BATCH_SIZE: u32 = 200;

/// 워크스페이스가 지원하는 최대 창고 수(운영상 상한, 하드 리밋은 아님).
pub const MAX_WAREHOUSES: u32 = 64;

/// 기본 통화 코드.
pub const DEFAULT_CURRENCY: &str = "KRW";

/// 코어 도메인 전역 설정. 각 크레이트가 자체 Config를 갖되(inv-parse의
/// `Config`가 대표적), 이 타입은 크레이트 전역에서 공유하는 최소 설정이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreConfig {
    pub warehouse_count: u32,
    pub default_currency: String,
    pub batch_size: u32,
    pub max_retry: u32,
}

impl Default for CoreConfig {
    fn default() -> Self {
        CoreConfig {
            warehouse_count: 1,
            default_currency: DEFAULT_CURRENCY.to_string(),
            batch_size: DEFAULT_BATCH_SIZE,
            max_retry: MAX_RETRY,
        }
    }
}

impl CoreConfig {
    /// 새 설정을 만든다. 나머지 필드는 기본값을 사용한다.
    pub fn with_warehouse_count(warehouse_count: u32) -> Self {
        CoreConfig { warehouse_count, ..CoreConfig::default() }
    }

    /// 설정이 운영 가능한 범위인지 검사한다(창고 수 상한, 배치 크기 0 금지).
    pub fn is_valid(&self) -> bool {
        self.warehouse_count > 0
            && self.warehouse_count <= MAX_WAREHOUSES
            && self.batch_size > 0
            && !self.default_currency.trim().is_empty()
    }

    /// 재시도 한도를 넘겼는지 판정한다(현재 시도 횟수 기준, 1부터 시작).
    pub fn retry_exhausted(&self, attempt: u32) -> bool {
        attempt >= self.max_retry
    }
}

/// 두 배치 크기 중 더 보수적인(작은) 값을 고른다.
pub fn conservative_batch_size(a: u32, b: u32) -> u32 {
    a.min(b)
}

/// 통화 코드가 워크스페이스가 인식하는 코드인지 확인한다.
pub fn is_known_currency(code: &str) -> bool {
    matches!(code, "KRW" | "USD" | "JPY")
}

/// 환경 프로파일(운영/스테이징/개발)에 따른 기본 설정을 만든다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Production,
    Staging,
    Development,
}

impl Environment {
    /// 문자열을 프로파일로 파싱한다(알 수 없으면 Development로 대체).
    pub fn parse(s: &str) -> Environment {
        match s.to_ascii_lowercase().as_str() {
            "production" | "prod" => Environment::Production,
            "staging" | "stage" => Environment::Staging,
            _ => Environment::Development,
        }
    }
}

/// 프로파일별 재시도 한도를 반환한다. 운영 환경은 `MAX_RETRY`를 그대로
/// 쓰지만, 개발 환경은 디버깅 편의를 위해 재시도를 1회로 줄인다.
pub fn retry_limit_for(env: Environment) -> u32 {
    match env {
        Environment::Production => MAX_RETRY,
        Environment::Staging => MAX_RETRY,
        Environment::Development => 1,
    }
}

/// 프로파일별 배치 크기를 반환한다(개발 환경은 빠른 반복을 위해 작게).
pub fn batch_size_for(env: Environment) -> u32 {
    match env {
        Environment::Production => DEFAULT_BATCH_SIZE,
        Environment::Staging => DEFAULT_BATCH_SIZE / 2,
        Environment::Development => 10,
    }
}

/// 설정 두 개를 비교해 달라진 필드명을 나열한다(변경 감사용).
pub fn diff_fields(a: &CoreConfig, b: &CoreConfig) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if a.warehouse_count != b.warehouse_count {
        changed.push("warehouse_count");
    }
    if a.default_currency != b.default_currency {
        changed.push("default_currency");
    }
    if a.batch_size != b.batch_size {
        changed.push("batch_size");
    }
    if a.max_retry != b.max_retry {
        changed.push("max_retry");
    }
    changed
}

/// 설정 값을 기본값 기준으로 병합한다: `override_cfg`의 0/빈 값 필드는
/// `base`의 값으로 채운다(부분 설정 오버레이 패턴).
pub fn merge_with_defaults(base: &CoreConfig, override_cfg: &CoreConfig) -> CoreConfig {
    CoreConfig {
        warehouse_count: if override_cfg.warehouse_count == 0 { base.warehouse_count } else { override_cfg.warehouse_count },
        default_currency: if override_cfg.default_currency.is_empty() {
            base.default_currency.clone()
        } else {
            override_cfg.default_currency.clone()
        },
        batch_size: if override_cfg.batch_size == 0 { base.batch_size } else { override_cfg.batch_size },
        max_retry: if override_cfg.max_retry == 0 { base.max_retry } else { override_cfg.max_retry },
    }
}
