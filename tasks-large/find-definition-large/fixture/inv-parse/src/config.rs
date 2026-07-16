//! 사내 설정 텍스트 포맷 파싱.
//!
//! 포맷은 `key=value` 줄 단위이며, `#`로 시작하는 줄은 주석으로 무시한다.
//! 알려지지 않은 키는 무시하고(향후 확장 대비), 알려진 키가 누락되면
//! `defaults` 모듈의 기본값으로 채운다.

use crate::defaults::{DEFAULT_CURRENCY_CODE, DEFAULT_VAT_PERCENT, DEFAULT_WAREHOUSE_COUNT};

/// 파싱된 배치 설정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config { pub vat_percent: u32, pub warehouse_count: u32, pub currency: String }

/// 이 모듈이 인식하는 키 목록(순서는 문서화 편의를 위한 것일 뿐 의미 없음).
pub const KNOWN_KEYS: [&str; 3] = ["vat_percent", "warehouse_count", "currency"];

/// 설정 텍스트를 파싱한다. 누락된 키는 `defaults` 모듈 값으로 채운다.
///
/// 줄 단위로 `key=value` 형태만 인식하며, 값 파싱에 실패한 키는 마치
/// 키가 아예 없었던 것처럼 취급해 기본값으로 대체한다(부분 오염된 설정
/// 파일이 전체 배치를 막지 않도록 하는 방어적 설계).
pub fn parse_config(text: &str) -> Config {
    let mut vat_percent: Option<u32> = None;
    let mut warehouse_count: Option<u32> = None;
    let mut currency: Option<String> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = split_key_value(line) else {
            continue;
        };
        match key {
            "vat_percent" => vat_percent = value.parse::<u32>().ok(),
            "warehouse_count" => warehouse_count = value.parse::<u32>().ok(),
            "currency" if !value.is_empty() => currency = Some(value.to_string()),
            _ => {} // 알 수 없는 키(또는 빈 값)는 무시 — 향후 필드 확장 대비
        }
    }

    Config {
        vat_percent: vat_percent.unwrap_or(DEFAULT_VAT_PERCENT),
        warehouse_count: warehouse_count.unwrap_or(DEFAULT_WAREHOUSE_COUNT),
        currency: currency.unwrap_or_else(|| DEFAULT_CURRENCY_CODE.to_string()),
    }
}

/// `key=value` 한 줄을 키/값으로 나눈다. `=`이 없으면 `None`.
///
/// 값에 `=`이 포함될 수 있어(예: URL 쿼리스트링) 첫 번째 `=`만 구분자로
/// 쓴다.
fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let idx = line.find('=')?;
    let key = line[..idx].trim();
    let value = line[idx + 1..].trim();
    if key.is_empty() {
        None
    } else {
        Some((key, value))
    }
}

/// 주어진 키가 이 모듈이 인식하는 키인지 검사한다.
pub fn is_known_key(key: &str) -> bool {
    KNOWN_KEYS.contains(&key)
}

/// 설정 텍스트에서 알 수 없는 키만 걸러내 목록으로 반환한다(감사/디버깅용).
pub fn unknown_keys(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = split_key_value(line) {
            if !is_known_key(key) && !found.iter().any(|k: &String| k == key) {
                found.push(key.to_string());
            }
        }
    }
    found
}

/// 설정 텍스트에서 알려진 키가 몇 개나 등장했는지 센다(중복 키는 각각 센다).
pub fn count_known_key_occurrences(text: &str) -> usize {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(split_key_value)
        .filter(|(k, _)| is_known_key(k))
        .count()
}

impl Config {
    /// 설정 값이 운영 가능한 범위인지 검사한다.
    pub fn is_valid(&self) -> bool {
        self.vat_percent <= 100 && self.warehouse_count > 0 && !self.currency.trim().is_empty()
    }

    /// 이 설정을 다시 `key=value` 텍스트로 직렬화한다(왕복 검증/디버그 출력용).
    pub fn to_text(&self) -> String {
        format!(
            "vat_percent={}\nwarehouse_count={}\ncurrency={}\n",
            self.vat_percent, self.warehouse_count, self.currency
        )
    }
}

/// 두 설정 값을 비교해 달라진 필드명을 나열한다(변경 감사용).
pub fn diff_fields(a: &Config, b: &Config) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if a.vat_percent != b.vat_percent {
        changed.push("vat_percent");
    }
    if a.warehouse_count != b.warehouse_count {
        changed.push("warehouse_count");
    }
    if a.currency != b.currency {
        changed.push("currency");
    }
    changed
}

/// `override_cfg`에 명시된 값을 우선하고, 그 외에는 `base` 값을 쓰는 병합.
///
/// "명시되었는지"는 이 함수 시그니처만으로는 구분할 수 없으므로, 여기서는
/// `override_cfg.currency`가 빈 문자열이면 미명시로 간주하는 관례를 쓴다.
pub fn merge_prefer_override(base: &Config, override_cfg: &Config) -> Config {
    Config {
        vat_percent: override_cfg.vat_percent,
        warehouse_count: override_cfg.warehouse_count,
        currency: if override_cfg.currency.trim().is_empty() {
            base.currency.clone()
        } else {
            override_cfg.currency.clone()
        },
    }
}

/// 설정 텍스트가 완전히 비어 있거나 주석/공백 줄뿐인지 검사한다.
pub fn is_blank_config_text(text: &str) -> bool {
    text.lines().all(|l| {
        let t = l.trim();
        t.is_empty() || t.starts_with('#')
    })
}

/// 설정 텍스트에서 실제로 파싱 가능한(= 형태를 갖춘) 줄 수를 센다.
pub fn parseable_line_count(text: &str) -> usize {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter(|l| split_key_value(l).is_some())
        .count()
}

/// 설정 텍스트에서 같은 키가 여러 번 등장하는지 검사한다(마지막 값이
/// 이긴다는 규칙을 몰랐던 작성자가 실수로 두 번 적는 경우가 흔하다).
pub fn duplicate_keys(text: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut duplicates: Vec<String> = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = split_key_value(line) {
            if seen.iter().any(|k| k == key) {
                if !duplicates.iter().any(|k| k == key) {
                    duplicates.push(key.to_string());
                }
            } else {
                seen.push(key.to_string());
            }
        }
    }
    duplicates
}

/// 설정 텍스트를 줄 목록으로 나누되, 각 줄이 어떤 종류인지(주석/빈줄/키값)
/// 함께 분류한다. 설정 파일 편집 도구(사내 lint 스크립트 등)의 뼈대로
/// 쓰인다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Blank,
    Comment,
    KeyValue,
    Malformed,
}

/// 한 줄의 종류를 분류한다.
pub fn classify_line(raw_line: &str) -> LineKind {
    let line = raw_line.trim();
    if line.is_empty() {
        LineKind::Blank
    } else if line.starts_with('#') {
        LineKind::Comment
    } else if split_key_value(line).is_some() {
        LineKind::KeyValue
    } else {
        LineKind::Malformed
    }
}

/// 설정 텍스트 전체를 줄별로 분류한다.
pub fn classify_all(text: &str) -> Vec<LineKind> {
    text.lines().map(classify_line).collect()
}

/// 형식이 어긋난(= 어느 분류에도 깔끔히 들어맞지 않는) 줄의 번호(1부터)를 찾는다.
pub fn malformed_line_numbers(text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| matches!(classify_line(l), LineKind::Malformed))
        .map(|(i, _)| i + 1)
        .collect()
}

/// 여러 설정 텍스트를 순서대로 겹쳐 쓴 것처럼 병합한 뒤 파싱한다(뒤에
/// 오는 텍스트의 키가 앞의 것을 덮어쓴다 — 계층형 설정 로딩과 같은 관례).
pub fn parse_layered(texts: &[&str]) -> Config {
    let combined = texts.join("\n");
    parse_config(&combined)
}

/// 통화 코드가 워크스페이스가 인식하는 코드인지 검사한다(inv-core의
/// 통화 목록과는 별개로, 파싱 단계에서 빠르게 걸러내는 얕은 화이트리스트).
pub fn is_known_currency_code(code: &str) -> bool {
    matches!(code, "KRW" | "USD" | "JPY" | "EUR" | "CNY")
}

/// 설정 값 중 통화 코드만 알 수 없는 값으로 바뀌었는지 검사한다(다른
/// 필드는 정상인데 통화만 이상한 경우를 걸러내는 좁은 점검).
pub fn has_unknown_currency_only(cfg: &Config) -> bool {
    cfg.is_valid() && !is_known_currency_code(&cfg.currency)
}
