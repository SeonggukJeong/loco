//! 대화형 확인 게이트 (스펙 §5). 미리보기 표시 후 y/N.

use std::cell::RefCell;

use regex::Regex;

use crate::agent::approval::{ApprovalRequest, Approver, Decision, first_deny_match};
use crate::ui::status::Spinner;

pub fn answer_is_yes(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

pub struct TtyApprover<'a> {
    pub spinner: &'a RefCell<Spinner>,
    /// 대화형에서는 차단하지 않고 [경고] 표시만 (스펙 §5 — 사용자가 게이트)
    pub deny: &'a [Regex],
}

impl Approver for TtyApprover<'_> {
    fn approve(&mut self, req: &ApprovalRequest<'_>) -> Decision {
        self.spinner.borrow_mut().stop();
        println!("\n── 확인 필요: {} ──", req.tool);
        println!("{}", req.preview);
        // let-chain (clippy::collapsible_if). [경고]가 비ASCII 기호 대신인 이유: CP949 레거시 콘솔
        if req.tool == "run_command"
            && let Some(pat) = first_deny_match(self.deny, req.args)
        {
            println!("[경고] 차단 패턴에 해당하는 명령입니다: {pat}");
        }
        // 의도적 동기 블로킹: REPL select!가 이 사이 Ctrl+C를 소비해 고아 stdin
        // 리더를 만드는 것을 방지한다. rustyline은 raw mode라 Ctrl+C가 SIGINT가
        // 아니라 Interrupted(→ 거부)로 돌아온다 — 승인된 설계 결정 1
        let answer = rustyline::DefaultEditor::new()
            .and_then(|mut rl| rl.readline("적용할까요? [y/N] "))
            .unwrap_or_default();
        if answer_is_yes(&answer) {
            Decision::Approve
        } else {
            println!("(거부함)");
            Decision::Deny {
                reason: "The user declined this action. Try a different approach, or call `finish`."
                    .to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::answer_is_yes;

    #[test]
    fn only_y_and_yes_mean_yes() {
        assert!(answer_is_yes("y"));
        assert!(answer_is_yes(" Y "));
        assert!(answer_is_yes("yes"));
        assert!(!answer_is_yes(""), "빈 입력(엔터)은 거부 — 기본값 N");
        assert!(!answer_is_yes("n"));
        assert!(!answer_is_yes("ㅇ"));
    }
}
