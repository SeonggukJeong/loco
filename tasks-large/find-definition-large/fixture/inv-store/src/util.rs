//! 저장소 계층에서 쓰는 소소한 헬퍼 모음.
//!
//! 조회 키 조합, epoch 타임스탬프 포맷, 바이트 크기 표시 등 저장소
//! 연산 전반에서 반복적으로 필요한 자잘한 함수들을 모아둔다.

/// SKU와 위치 문자열을 합쳐 저장소 조회 키를 만든다.
pub fn compose_key(sku: &str, location: &str) -> String {
    format!("{sku}@{location}")
}

/// 저장소 키를 SKU/위치로 다시 나눈다. `@`가 없으면 `None`.
pub fn split_key(key: &str) -> Option<(&str, &str)> {
    key.split_once('@')
}

/// 유닉스 epoch 초를 사람이 읽는 형태(YYYY-MM-DD HH:MM:SS UTC)로 대략
/// 변환한다. 타임존은 항상 UTC로 가정한다(저장소 내부 로그 전용이라
/// 로컬 타임존 변환은 상위 계층의 몫).
pub fn format_epoch_utc(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let secs_of_day = epoch_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let hh = secs_of_day / 3600;
    let mm = (secs_of_day % 3600) / 60;
    let ss = secs_of_day % 60;
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02} UTC")
}

/// Howard Hinnant의 `civil_from_days` 알고리즘(그레고리력, 프롤렙틱).
/// 외부 날짜/시간 크레이트 없이 epoch-day를 연/월/일로 바꾸는 표준적인
/// 정수 연산 기법이다.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// 바이트 수를 사람이 읽는 단위(B/KB/MB)로 포맷한다(저장소 파일 크기
/// 로그 출력용).
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// 두 epoch 초 값 사이의 경과 시간(초)을 계산한다(음수는 0으로 clamp).
pub fn elapsed_secs(from_epoch: i64, to_epoch: i64) -> i64 {
    (to_epoch - from_epoch).max(0)
}

/// 문자열이 저장소 키로 쓰기에 안전한지(제어 문자/`@` 두 개 이상 포함
/// 안 함) 검사한다.
pub fn is_safe_key_component(s: &str) -> bool {
    !s.is_empty() && !s.contains('@') && s.chars().all(|c| !c.is_control())
}

/// 정수 목록의 합계를 오버플로 없이 계산한다(saturating 누적).
pub fn saturating_sum(values: &[i64]) -> i64 {
    values.iter().fold(0i64, |acc, v| acc.saturating_add(*v))
}

/// 문자열을 지정한 길이로 잘라 말줄임표를 붙인다(로그 한 줄 길이 제한용).
pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{truncated}...")
    }
}

/// 값을 백분율 문자열로 포맷한다(정수 %).
pub fn format_percent(value: u32) -> String {
    format!("{}%", value.min(100))
}

/// 두 값 중 더 가까운 쪽(허용 오차 기준)을 판정해 근사 동등성을 검사한다.
pub fn nearly_equal_u32(a: u32, b: u32, tolerance: u32) -> bool {
    a.abs_diff(b) <= tolerance
}

/// 문자열 목록에서 최대 길이를 구한다(표 컬럼 너비 계산 등에 사용).
pub fn max_len(items: &[String]) -> usize {
    items.iter().map(|s| s.chars().count()).max().unwrap_or(0)
}

/// 값이 0이 아니면 그대로, 0이면 대체값을 반환한다(기본값 폴백 패턴).
pub fn or_default_u32(value: u32, default: u32) -> u32 {
    if value == 0 {
        default
    } else {
        value
    }
}

/// 카운터 맵(이름 -> 개수) 목록을 개수 내림차순으로 정렬한다(동률은 이름
/// 오름차순).
pub fn sort_counts_desc(counts: &mut Vec<(String, usize)>) {
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
}
