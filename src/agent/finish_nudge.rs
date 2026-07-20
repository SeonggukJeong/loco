//! 검증완료 후 finish 유도 상태기계 (M9 §4-2). 목표 패턴: "이미 확인한 사실을
//! 문자 그대로 재확인하는 루프" — 순수 동일-명령 루프는 순환 교정·반복정지가
//! 전담하고(우선순위는 agent 루프가 보장), 이 기계는 이종/혼합 재검증을 겨냥한다.

use std::collections::VecDeque;

/// 발동 시 1회 주입 (M9 §4-2). 모델 대상 — 영어
pub const FINISH_NUDGE: &str = "You already ran a successful verification. If the task is complete, \
call finish with a summary now; do not re-verify what you have already confirmed.";

/// §3-3-3-1 — 마지막 검증이 파이프여서 "successful"이 참이 아닌 경우.
/// 기본 문구를 그대로 쓰면 파이프 VERIFY_NUDGE와 같은 이벤트를 반대로 부른다
pub const FINISH_NUDGE_PIPE: &str = "You have re-verified several times. Note your last verification \
was a shell pipeline, so it did not establish that the tests passed - run it once without a pipe, then finish.";

/// 발동에 필요한 연속 카운트 턴 수 (§4-2: K=4)
const IDLE_WINDOW: usize = 4;

/// run() 루프가 턴마다 분류해 넘기는 이벤트 (M9 §4-2 전이 표와 1:1)
pub enum TurnEvent {
    /// edit_file/write_file 성공 디스패치 ("뮤테이션"의 정의 — is_mutating()과 다름)
    MutationOk,
    /// edit_file/write_file 실패 시도 (오류·게이트 거부 포함)
    MutationAttempt,
    /// run_command Ok ∧ 본문 첫 줄 `exit code: 0`. repeat = 반복-호출 여부
    VerifyOk { repeat: bool },
    /// run_command 그 외 (비0 종료코드·타임아웃·취소·Err)
    VerifyOther,
    /// read_file/grep/list_files. repeat = 반복-호출 여부
    ReadOnly { repeat: bool },
    /// finish 시도 (유효·무효 무관 — 무효 finish 교정은 §4-1이 전담)
    FinishAttempt,
    /// 그 외 (미지 도구, 게이트 거부된 run_command) — 상태 불변
    Other,
}

pub struct FinishNudge {
    mutated: bool,
    armed: bool,
    /// 카운트된 최근 IDLE_WINDOW턴의 반복-호출 여부 (§4-2 발동 조건)
    idle: VecDeque<bool>,
    latched: bool,
}

impl FinishNudge {
    pub fn new() -> Self {
        Self { mutated: false, armed: false, idle: VecDeque::with_capacity(IDLE_WINDOW), latched: false }
    }

    /// 이벤트를 반영하고, 발동 조건이 차면 FINISH_NUDGE를 1회 반환 (런당 래치)
    pub fn on_turn(&mut self, ev: TurnEvent) -> Option<&'static str> {
        match ev {
            TurnEvent::MutationOk => {
                self.mutated = true;
                self.disarm();
            }
            TurnEvent::MutationAttempt => self.disarm(),
            TurnEvent::VerifyOk { repeat } => {
                if self.armed {
                    // 재검증도 카운트 — 매 검증마다 리셋하면 run_command 재검증
                    // 루프에 영원히 발동하지 않는다 (§4-2 표 3행)
                    self.count(repeat);
                } else if self.mutated {
                    self.armed = true;
                    self.idle.clear();
                }
            }
            TurnEvent::VerifyOther => self.disarm(),
            TurnEvent::ReadOnly { repeat } => {
                if self.armed {
                    self.count(repeat);
                }
            }
            TurnEvent::FinishAttempt => self.idle.clear(),
            TurnEvent::Other => {}
        }
        if self.armed && self.idle.len() >= IDLE_WINDOW && self.idle.iter().any(|r| *r) && !self.latched {
            self.latched = true;
            return Some(FINISH_NUDGE);
        }
        None
    }

    fn count(&mut self, repeat: bool) {
        if self.idle.len() == IDLE_WINDOW {
            self.idle.pop_front();
        }
        self.idle.push_back(repeat);
    }

    fn disarm(&mut self) {
        self.armed = false;
        self.idle.clear();
    }
}

impl Default for FinishNudge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 뮤테이션 성공 + exit-0 검증까지 마친(무장된) 기계
    fn armed_machine() -> FinishNudge {
        let mut n = FinishNudge::new();
        assert!(n.on_turn(TurnEvent::MutationOk).is_none());
        assert!(n.on_turn(TurnEvent::VerifyOk { repeat: false }).is_none(), "무장 턴 자체는 카운트 없음");
        n
    }

    #[test]
    fn fires_on_fourth_counted_turn_with_a_repeat_then_latches() {
        let mut n = armed_machine();
        assert!(n.on_turn(TurnEvent::ReadOnly { repeat: false }).is_none());
        assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none());
        assert!(n.on_turn(TurnEvent::ReadOnly { repeat: false }).is_none());
        assert_eq!(n.on_turn(TurnEvent::ReadOnly { repeat: false }), Some(FINISH_NUDGE));
        assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none(), "런당 1회 래치");
    }

    #[test]
    fn four_novel_turns_do_not_fire_until_a_repeat_enters_the_window() {
        let mut n = armed_machine();
        for _ in 0..4 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: false }).is_none(), "신규 탐색만으로는 불발 (§4-2)");
        }
        assert_eq!(n.on_turn(TurnEvent::ReadOnly { repeat: true }), Some(FINISH_NUDGE), "반복이 창에 들어오면 발동");
    }

    #[test]
    fn armed_verify_ok_counts_toward_idle() {
        let mut n = armed_machine();
        for _ in 0..3 {
            assert!(n.on_turn(TurnEvent::VerifyOk { repeat: true }).is_none());
        }
        assert_eq!(n.on_turn(TurnEvent::VerifyOk { repeat: true }), Some(FINISH_NUDGE), "run_command 재검증도 카운트");
    }

    #[test]
    fn mutation_attempt_disarms_and_a_later_verify_rearms() {
        let mut n = armed_machine();
        n.on_turn(TurnEvent::ReadOnly { repeat: true });
        n.on_turn(TurnEvent::MutationAttempt); // S/R 루프 등 — 무장 해제 (§4-2 표 2행)
        for _ in 0..4 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none(), "비무장은 카운트 없음");
        }
        assert!(n.on_turn(TurnEvent::VerifyOk { repeat: false }).is_none()); // 재무장
        for _ in 0..3 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none());
        }
        assert_eq!(n.on_turn(TurnEvent::ReadOnly { repeat: true }), Some(FINISH_NUDGE));
    }

    #[test]
    fn verify_without_prior_mutation_does_not_arm() {
        let mut n = FinishNudge::new();
        n.on_turn(TurnEvent::VerifyOk { repeat: false });
        for _ in 0..5 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none(), "뮤테이션 없는 런은 무장 안 함");
        }
    }

    #[test]
    fn failed_or_timed_out_verify_disarms() {
        let mut n = armed_machine();
        n.on_turn(TurnEvent::VerifyOther);
        for _ in 0..5 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none(), "실패한 검증 뒤 finish 유도는 역효과 (§4-2)");
        }
    }

    #[test]
    fn finish_attempt_resets_idle_but_keeps_armed() {
        let mut n = armed_machine();
        for _ in 0..3 {
            n.on_turn(TurnEvent::ReadOnly { repeat: true });
        }
        n.on_turn(TurnEvent::FinishAttempt);
        for _ in 0..3 {
            assert!(n.on_turn(TurnEvent::ReadOnly { repeat: true }).is_none(), "리셋 후 다시 4턴 필요");
        }
        assert_eq!(n.on_turn(TurnEvent::ReadOnly { repeat: true }), Some(FINISH_NUDGE), "armed는 유지");
    }

    #[test]
    fn other_turns_leave_state_unchanged() {
        let mut n = armed_machine();
        for _ in 0..3 {
            n.on_turn(TurnEvent::ReadOnly { repeat: true });
        }
        n.on_turn(TurnEvent::Other); // 미지 도구·게이트 거부 run_command (§4-2 표 8행)
        assert_eq!(
            n.on_turn(TurnEvent::ReadOnly { repeat: false }),
            Some(FINISH_NUDGE),
            "Other가 카운터를 건드리지 않았으므로 4번째 카운트 턴에 발동"
        );
    }
}
