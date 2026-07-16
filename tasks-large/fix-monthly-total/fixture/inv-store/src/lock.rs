//! 저장소 연산 전 SKU 단위 어드바이저리 락(advisory lock) 시뮬레이션.
//!
//! 실제 프로세스 간 락(파일 락, DB 락)은 상위 계층의 몫이다. 이 모듈은
//! 단일 프로세스 안에서 "이 SKU는 지금 누군가 수정 중"이라는 상태만
//! 추적하는 가벼운 자료구조로, 테스트에서 락 정책을 검증할 때 쓴다.

/// SKU 단위 락 보유 상태를 추적하는 레지스트리.
#[derive(Debug, Clone, Default)]
pub struct LockRegistry {
    held: Vec<String>,
}

impl LockRegistry {
    pub fn new() -> Self {
        LockRegistry::default()
    }

    /// 락 획득을 시도한다. 이미 잠겨 있으면 false.
    pub fn try_acquire(&mut self, sku: &str) -> bool {
        if self.held.iter().any(|s| s == sku) {
            false
        } else {
            self.held.push(sku.to_string());
            true
        }
    }

    /// 락을 해제한다. 실제로 보유 중이었으면 true.
    pub fn release(&mut self, sku: &str) -> bool {
        let before = self.held.len();
        self.held.retain(|s| s != sku);
        self.held.len() != before
    }

    /// SKU가 현재 잠겨 있는지 검사한다.
    pub fn is_locked(&self, sku: &str) -> bool {
        self.held.iter().any(|s| s == sku)
    }

    /// 현재 잠긴 SKU 개수.
    pub fn locked_count(&self) -> usize {
        self.held.len()
    }

    /// 모든 락을 강제로 해제한다(비정상 종료 후 복구 절차 등에서 사용).
    pub fn release_all(&mut self) {
        self.held.clear();
    }

    /// 현재 잠긴 SKU 목록(정렬됨).
    pub fn locked_skus(&self) -> Vec<String> {
        let mut skus = self.held.clone();
        skus.sort();
        skus
    }
}

/// 여러 SKU를 한 번에 잠근다. 하나라도 실패하면 이미 획득한 락을 모두
/// 되돌리고(원자적 다중 락) false를 반환한다.
pub fn try_acquire_many(registry: &mut LockRegistry, skus: &[String]) -> bool {
    let mut acquired: Vec<String> = Vec::new();
    for sku in skus {
        if registry.try_acquire(sku) {
            acquired.push(sku.clone());
        } else {
            for a in &acquired {
                registry.release(a);
            }
            return false;
        }
    }
    true
}

/// SKU 목록 중 하나라도 잠긴 것이 있는지 검사한다.
pub fn any_locked(registry: &LockRegistry, skus: &[String]) -> bool {
    skus.iter().any(|s| registry.is_locked(s))
}

/// 락 대기 상태를 나타내는 요청. 우선순위(낮을수록 먼저)와 함께 큐에
/// 쌓인다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockRequest {
    pub sku: String,
    pub priority: u8,
}

/// 대기 요청 목록을 우선순위 오름차순으로 정렬한다(동률은 도착 순서 유지).
pub fn sort_requests_by_priority(requests: &mut Vec<LockRequest>) {
    requests.sort_by(|a, b| a.priority.cmp(&b.priority));
}

/// 대기 요청 목록에서 지금 잠겨 있지 않은 SKU에 대한 요청만 걸러낸다
/// (즉시 처리 가능한 요청 후보).
pub fn immediately_processable(registry: &LockRegistry, requests: &[LockRequest]) -> Vec<LockRequest> {
    requests.iter().filter(|r| !registry.is_locked(&r.sku)).cloned().collect()
}

/// 레지스트리 상태를 스냅샷(정렬된 SKU 목록)으로 캡처한다(디버그 로그용).
pub fn snapshot_locked(registry: &LockRegistry) -> Vec<String> {
    registry.locked_skus()
}

/// 두 레지스트리가 정확히 같은 SKU 집합을 잠그고 있는지 비교한다.
pub fn same_lock_state(a: &LockRegistry, b: &LockRegistry) -> bool {
    a.locked_skus() == b.locked_skus()
}

/// 요청 목록에서 중복 SKU를 제거한다(같은 SKU가 여러 번 요청된 경우 첫
/// 요청만 남김 — 우선순위가 가장 높은 요청이 먼저 오도록 정렬 후 호출
/// 하는 것을 권장한다).
pub fn dedup_requests(requests: &[LockRequest]) -> Vec<LockRequest> {
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for req in requests {
        if !seen.contains(&req.sku) {
            seen.push(req.sku.clone());
            out.push(req.clone());
        }
    }
    out
}
