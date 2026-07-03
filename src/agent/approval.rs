//! 확인 게이트 (스펙 §5). 게이트 판단은 Agent 루프가, 결정은 Approver가 담당한다.

/// mutating 액션 하나에 대한 확인 요청
pub struct ApprovalRequest<'a> {
    pub tool: &'a str,
    pub args: &'a serde_json::Value,
    /// Tool::preview() 결과 (diff, 명령어 등) — 사용자에게 보여줄 내용
    pub preview: &'a str,
}

pub enum Decision {
    Approve,
    /// reason은 tool_result로 모델에 전달된다 — 영어 (스펙 §4)
    Deny { reason: String },
}

/// 동기 트레이트로 유지한다: TtyApprover가 의도적으로 블로킹해 REPL select!의
/// Ctrl+C 소비(고아 stdin 리더)를 막는다 — 설계 결정 1 참조
pub trait Approver {
    fn approve(&mut self, req: &ApprovalRequest<'_>) -> Decision;
}

/// --auto: 전부 승인 (deny 패턴 차단은 Task 7에서 추가)
#[derive(Default)]
pub struct AutoApprover;

impl Approver for AutoApprover {
    fn approve(&mut self, _req: &ApprovalRequest<'_>) -> Decision {
        Decision::Approve
    }
}

/// -p에서 --auto 없음: 프롬프트를 띄우지 않고 거부한다 (스펙 §7)
pub struct NonInteractiveApprover;

impl Approver for NonInteractiveApprover {
    fn approve(&mut self, _req: &ApprovalRequest<'_>) -> Decision {
        Decision::Deny {
            reason: "mutating tools are unavailable in non-interactive mode; \
                     the user must re-run loco with --auto to allow them"
                .to_string(),
        }
    }
}
