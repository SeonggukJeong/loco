//! 감사 로그(audit log) 타입과 헬퍼.
//!
//! "누가, 언제, 어떤 SKU에 대해 어떤 액션을 했는지"를 기록한다. 필드 단위
//! diff는 `history.rs`가 담당하고, 이 파일은 액션 자체(삭제/조정/승인 등)의
//! 발생 이력을 다룬다.

/// 감사 로그 한 건.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogEntry {
    pub actor: String,
    pub action: String,
    pub target_sku: String,
    pub epoch: i64,
}

impl AuditLogEntry {
    pub fn new(actor: impl Into<String>, action: impl Into<String>, target_sku: impl Into<String>, epoch: i64) -> Self {
        AuditLogEntry { actor: actor.into(), action: action.into(), target_sku: target_sku.into(), epoch }
    }
}

/// 새 감사 로그를 기록한다.
pub fn log_action(log: &mut Vec<AuditLogEntry>, actor: &str, action: &str, target_sku: &str, epoch: i64) {
    log.push(AuditLogEntry::new(actor, action, target_sku, epoch));
}

/// 특정 행위자(actor)의 액션 수를 센다.
pub fn count_actions_by_actor(log: &[AuditLogEntry], actor: &str) -> usize {
    log.iter().filter(|e| e.actor == actor).count()
}

/// 특정 액션 종류의 로그만 걸러낸다.
pub fn filter_by_action<'a>(log: &'a [AuditLogEntry], action: &str) -> Vec<&'a AuditLogEntry> {
    log.iter().filter(|e| e.action == action).collect()
}

/// 짧은 시간 창(window_secs) 안에 동일 행위자가 임계 건수 이상의 액션을
/// 수행했는지(이상 행동 의심) 판정한다. 단순 슬라이딩 카운트 방식이다.
pub fn find_suspicious_bursts(log: &[AuditLogEntry], actor: &str, window_secs: i64, threshold: usize) -> bool {
    let mut times: Vec<i64> = log.iter().filter(|e| e.actor == actor).map(|e| e.epoch).collect();
    times.sort();
    for i in 0..times.len() {
        let mut count = 1usize;
        for j in (i + 1)..times.len() {
            if times[j] - times[i] <= window_secs {
                count += 1;
            } else {
                break;
            }
        }
        if count >= threshold {
            return true;
        }
    }
    false
}

/// 특정 역할(role)이 특정 액션을 수행할 권한이 있는지 판정한다.
///
/// 매우 단순화된 권한 모델이다: ADMIN은 전부 허용, VIEWER는 전부 거부,
/// 그 외 역할은 조회성 액션("VIEW")만 허용한다.
pub fn is_action_allowed(role: &str, action: &str) -> bool {
    match role {
        "ADMIN" => true,
        "VIEWER" => false,
        _ => action == "VIEW",
    }
}

/// 특정 SKU에 대한 감사 로그만 걸러낸다.
pub fn filter_by_sku<'a>(log: &'a [AuditLogEntry], sku: &str) -> Vec<&'a AuditLogEntry> {
    log.iter().filter(|e| e.target_sku == sku).collect()
}

/// 감사 로그를 시각 오름차순으로 정렬한다.
pub fn sort_chronological(log: &mut Vec<AuditLogEntry>) {
    log.sort_by_key(|e| e.epoch);
}

/// 로그에 등장하는 고유 행위자 목록(중복 제거, 정렬됨)을 반환한다.
pub fn distinct_actors(log: &[AuditLogEntry]) -> Vec<String> {
    let mut actors: Vec<String> = log.iter().map(|e| e.actor.clone()).collect();
    actors.sort();
    actors.dedup();
    actors
}

/// 지정된 기간 동안의 로그만 걸러낸다.
pub fn logs_in_range(log: &[AuditLogEntry], from_epoch: i64, to_epoch: i64) -> Vec<AuditLogEntry> {
    log.iter().filter(|e| e.epoch >= from_epoch && e.epoch <= to_epoch).cloned().collect()
}

/// 행위자별 액션 종류 다양성(고유 액션 개수)을 계산한다.
///
/// 한 행위자가 다양한 종류의 액션을 짧은 기간에 수행했다면 비정상 패턴일
/// 수 있다는 신호로 쓰인다(구체적 판정은 상위 로직의 몫).
pub fn distinct_actions_by_actor(log: &[AuditLogEntry], actor: &str) -> usize {
    let mut actions: Vec<String> = log.iter().filter(|e| e.actor == actor).map(|e| e.action.clone()).collect();
    actions.sort();
    actions.dedup();
    actions.len()
}

/// 특정 SKU에 대한 마지막 액션을 찾는다(시각 최댓값 기준).
pub fn last_action_on_sku<'a>(log: &'a [AuditLogEntry], sku: &str) -> Option<&'a AuditLogEntry> {
    log.iter().filter(|e| e.target_sku == sku).max_by_key(|e| e.epoch)
}

/// 액션 종류별 발생 건수를 센다(액션명 -> 건수, 액션명 오름차순 정렬).
pub fn action_counts(log: &[AuditLogEntry]) -> Vec<(String, usize)> {
    let mut actions: Vec<String> = log.iter().map(|e| e.action.clone()).collect();
    actions.sort();
    actions.dedup();
    actions
        .into_iter()
        .map(|action| {
            let count = log.iter().filter(|e| e.action == action).count();
            (action, count)
        })
        .collect()
}

/// 두 감사 로그 목록을 시간순으로 병합한다.
pub fn merge_logs(a: &[AuditLogEntry], b: &[AuditLogEntry]) -> Vec<AuditLogEntry> {
    let mut merged: Vec<AuditLogEntry> = a.iter().chain(b.iter()).cloned().collect();
    sort_chronological(&mut merged);
    merged
}

/// 특정 행위자가 특정 SKU에 대해 액션을 수행한 적이 있는지 확인한다.
pub fn has_touched(log: &[AuditLogEntry], actor: &str, sku: &str) -> bool {
    log.iter().any(|e| e.actor == actor && e.target_sku == sku)
}

/// 로그 항목 수가 임계값을 초과하면(로그 폭주) true를 반환한다 — 간단한
/// 용량 가드로 쓰인다.
pub fn is_log_volume_excessive(log: &[AuditLogEntry], threshold: usize) -> bool {
    log.len() > threshold
}
