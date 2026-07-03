//! 확인 게이트 (스펙 §5). 게이트 판단은 Agent 루프가, 결정은 Approver가 담당한다.

use regex::Regex;

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

/// 설정 문자열 → 대소문자 무시 Regex. 잘못된 패턴은 시작 시 에러 (fail fast)
pub fn compile_patterns(patterns: &[String]) -> anyhow::Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|p| {
            regex::RegexBuilder::new(p)
                .case_insensitive(true)
                .build()
                .map_err(|e| anyhow::anyhow!("auto_deny_patterns의 정규식이 잘못됨 ({p}): {e}"))
        })
        .collect()
}

/// run_command 인자가 차단 패턴에 걸리면 해당 패턴 문자열 반환
pub fn first_deny_match<'a>(patterns: &'a [Regex], args: &serde_json::Value) -> Option<&'a str> {
    let cmd = args.get("command")?.as_str()?;
    patterns.iter().find(|re| re.is_match(cmd)).map(|re| re.as_str())
}

/// --auto: run_command에 한해 auto_deny_patterns에 걸리면 거부, 나머지는 전부 승인
#[derive(Default)]
pub struct AutoApprover {
    deny: Vec<Regex>,
}

impl AutoApprover {
    pub fn new(patterns: &[String]) -> anyhow::Result<Self> {
        Ok(Self { deny: compile_patterns(patterns)? })
    }
    pub fn from_compiled(deny: Vec<Regex>) -> Self {
        Self { deny }
    }
}

impl Approver for AutoApprover {
    fn approve(&mut self, req: &ApprovalRequest<'_>) -> Decision {
        // let-chain — 단독 중첩 if는 clippy::collapsible_if에 걸린다
        if req.tool == "run_command"
            && let Some(pat) = first_deny_match(&self.deny, req.args)
        {
            return Decision::Deny {
                reason: format!("command blocked by auto_deny_patterns (matched `{pat}`)"),
            };
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn auto() -> AutoApprover {
        AutoApprover::new(&crate::config::default_deny_patterns()).unwrap()
    }

    fn req_cmd(cmd: &str) -> serde_json::Value {
        serde_json::json!({"command": cmd})
    }

    #[test]
    fn dangerous_commands_are_denied_in_auto_mode() {
        let mut a = auto();
        for cmd in ["sudo rm x", "rm -rf /", "rm -fr .", "git push origin main", "dd if=/dev/zero"] {
            let args = req_cmd(cmd);
            let d = a.approve(&ApprovalRequest { tool: "run_command", args: &args, preview: "" });
            assert!(matches!(d, Decision::Deny { .. }), "{cmd}는 차단돼야 함");
        }
    }

    #[test]
    fn normal_commands_and_file_tools_pass() {
        let mut a = auto();
        let args = req_cmd("cargo test");
        assert!(matches!(
            a.approve(&ApprovalRequest { tool: "run_command", args: &args, preview: "" }),
            Decision::Approve
        ));
        let w = serde_json::json!({"path": "a.rs", "content": "x"});
        assert!(matches!(
            a.approve(&ApprovalRequest { tool: "write_file", args: &w, preview: "" }),
            Decision::Approve
        ));
    }

    #[test]
    fn invalid_pattern_fails_fast() {
        assert!(AutoApprover::new(&["(".to_string()]).is_err());
    }
}
