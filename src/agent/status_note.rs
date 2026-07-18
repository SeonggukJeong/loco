//! M11 §4 — 진행 상태 접지(상태선). 하네스가 이미 아는 진행 상태(수정 파일·
//! 검증 여부·잔여 턴)를 조건부로 툴 결과 note에 접지한다. 유도형 — 차단 없음.
//! 채널 격리: note는 body와 분리되므로 반복 해시·오류 스트릭·exit 파싱 무영향.

use std::path::{Component, Path};

/// 상태선 마커 — session::remove_status_note·scripts/exp_metrics.py와 공유 계약
pub const STATUS_MARKER: &str = "[status] ";
/// 연속 줄 들여쓰기(9칸 = 마커 길이) — 블록 경계 판정 구조 (§4 블록 경계 핀)
pub const CONT_INDENT: &str = "         ";

/// 뮤테이션 0회 케이던스 (조건 2 — 탐색 루프 겨냥)
const ZERO_MUT_CADENCE: [usize; 4] = [5, 10, 15, 20];
/// 무조건 페이싱 임계 (조건 3 — 턴 소진 겨냥)
const PACING: [usize; 2] = [15, 20];

/// run()이 turns를 증가시키는 매 턴 종료 지점에서 넘기는 이번 턴의 사실
pub struct TurnCtx {
    /// 이번 액션 턴의 1-기준 순번 = run()의 `turns + 1` (도달 판정 시점 핀)
    pub turn: usize,
    pub max_turns: usize,
    /// 이번 턴이 성공 뮤테이션(edit_file/write_file 디스패치 Ok)인가 (조건 1)
    pub mutation_ok: bool,
    /// push_tool_result 경로인가 — false(length·finish 오류·VERIFY_NUDGE 반려)면
    /// 임계는 pending 이월 (§4 이월 핀)
    pub has_note_channel: bool,
    /// run()의 mutated_since_verify (VERIFY_NUDGE 의미론 상속 — §4)
    pub mutated_since_verify: bool,
}

pub struct StatusNote {
    mutated_paths: Vec<String>,
    last_cmd_exit: Option<String>,
    pending: bool,
}

impl Default for StatusNote {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusNote {
    pub fn new() -> Self {
        Self { mutated_paths: Vec::new(), last_cmd_exit: None, pending: false }
    }

    /// 성공 뮤테이션의 path 수집 — 렉시컬 정규화·중복 제거·삽입 순서 유지.
    /// path 부재/비문자열은 생략 (§4)
    pub fn record_mutation(&mut self, args: &serde_json::Value) {
        let Some(p) = args.get("path").and_then(|v| v.as_str()) else { return };
        let n = normalize(p);
        if !self.mutated_paths.contains(&n) {
            self.mutated_paths.push(n);
        }
    }

    /// run_command Ok의 body 첫 줄에서 exit 값을 접두 파싱 — 줄이 없으면 None으로
    /// **덮어쓴다**(스테일 방지 핀 §4: 이전 성공의 0이 거짓 접지되지 않게)
    pub fn record_command_exit(&mut self, body: &str) {
        self.last_cmd_exit = body
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("exit code: "))
            .map(str::to_string);
    }

    /// 턴 종료 지점 판정 — 발동이면 렌더된 상태선(턴당 최대 1회는 호출 지점이
    /// 턴당 하나라는 배선 계약으로 성립). 채널 없는 턴의 임계는 pending 이월
    pub fn on_turn(&mut self, ctx: &TurnCtx) -> Option<String> {
        let cadence = self.mutated_paths.is_empty() && ZERO_MUT_CADENCE.contains(&ctx.turn);
        let pacing = PACING.contains(&ctx.turn);
        if !(ctx.mutation_ok || cadence || pacing || self.pending) {
            return None;
        }
        if !ctx.has_note_channel {
            self.pending = true;
            return None;
        }
        self.pending = false;
        Some(self.render(ctx))
    }

    fn render(&self, ctx: &TurnCtx) -> String {
        let turns_line = format!("turns: {} of {} used", ctx.turn, ctx.max_turns);
        if self.mutated_paths.is_empty() {
            return format!("{STATUS_MARKER}files edited: none yet | {turns_line}");
        }
        let shown = self.mutated_paths.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
        let extra = self.mutated_paths.len().saturating_sub(5);
        let files = if extra > 0 {
            format!("{} ({shown} and {extra} more)", self.mutated_paths.len())
        } else {
            format!("{} ({shown})", self.mutated_paths.len())
        };
        let verification = if ctx.mutated_since_verify {
            "verification: none since your last edit".to_string()
        } else {
            match &self.last_cmd_exit {
                Some(code) => format!("verification: last command exited {code}"),
                None => "verification: last command gave no exit code".to_string(),
            }
        };
        format!("{STATUS_MARKER}files edited: {files}\n{CONT_INDENT}{verification}\n{CONT_INDENT}{turns_line}")
    }
}

/// 렉시컬 정규화 — CurDir 제거·ParentDir 팝, 파일시스템 조회 없음
/// (m10/arm-block:src/agent/sr_block.rs에서 포팅 — M10 스펙 §4)
fn normalize(path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for c in Path::new(path).components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other.as_os_str().to_string_lossy().into_owned()),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(turn: usize, mutation_ok: bool, channel: bool, msv: bool) -> TurnCtx {
        TurnCtx { turn, max_turns: 25, mutation_ok, has_note_channel: channel, mutated_since_verify: msv }
    }

    #[test]
    fn zero_mutation_cadence_fires_at_5_10_15_20_only() {
        let mut s = StatusNote::new();
        for t in 1..=25 {
            let got = s.on_turn(&ctx(t, false, true, false)).is_some();
            let want = matches!(t, 5 | 10 | 15 | 20);
            assert_eq!(got, want, "turn {t}");
        }
    }

    #[test]
    fn mutation_turn_fires_immediately_and_cadence_stops_after_mutation() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "src/a.rs"}));
        let note = s.on_turn(&ctx(2, true, true, true)).expect("조건 1");
        assert!(note.contains("files edited: 1 (src/a.rs)"), "{note}");
        assert!(note.contains("verification: none since your last edit"), "{note}");
        assert!(note.contains("turns: 2 of 25 used"), "{note}");
        // 뮤테이션 이후 케이던스(조건 2)는 침묵 — 15·20(조건 3)만 남는다
        assert!(s.on_turn(&ctx(5, false, true, true)).is_none());
        assert!(s.on_turn(&ctx(10, false, true, true)).is_none());
        assert!(s.on_turn(&ctx(15, false, true, true)).is_some(), "조건 3");
    }

    #[test]
    fn zero_mutation_render_is_single_line() {
        let mut s = StatusNote::new();
        let note = s.on_turn(&ctx(5, false, true, false)).unwrap();
        assert_eq!(note, "[status] files edited: none yet | turns: 5 of 25 used");
    }

    #[test]
    fn verified_state_shows_last_exit_and_overwrite_pins() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        s.record_command_exit("exit code: 101\nfailed");
        let note = s.on_turn(&ctx(15, false, true, false)).unwrap();
        assert!(note.contains("verification: last command exited 101"), "{note}");
        // 덮어쓰기 핀: exit 줄 없는 Ok(타임아웃 본문)는 None으로 덮는다 (§4)
        s.record_command_exit("command timed out after 240s and was killed\n");
        let note = s.on_turn(&ctx(20, false, true, false)).unwrap();
        assert!(note.contains("verification: last command gave no exit code"), "{note}");
    }

    #[test]
    fn path_list_caps_at_five_with_lexical_dedup() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "./src/a.rs"}));
        s.record_mutation(&serde_json::json!({"path": "src/a.rs"})); // 표기 변형 — 합산
        for p in ["b.rs", "c.rs", "d.rs", "e.rs", "f.rs", "g.rs"] {
            s.record_mutation(&serde_json::json!({"path": p}));
        }
        let note = s.on_turn(&ctx(9, true, true, true)).unwrap();
        assert!(note.contains("files edited: 7 (src/a.rs, b.rs, c.rs, d.rs, e.rs and 2 more)"), "{note}");
    }

    #[test]
    fn threshold_on_channelless_turn_carries_over_once() {
        let mut s = StatusNote::new();
        assert!(s.on_turn(&ctx(5, false, false, false)).is_none(), "채널 없음 — 이월");
        let note = s.on_turn(&ctx(6, false, true, false)).expect("이월분 1회 주입");
        assert!(note.contains("turns: 6 of 25"), "{note}");
        assert!(s.on_turn(&ctx(7, false, true, false)).is_none(), "이월은 1회로 소진");
    }

    #[test]
    fn continuation_lines_use_nine_space_indent() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        let note = s.on_turn(&ctx(3, true, true, true)).unwrap();
        for line in note.lines().skip(1) {
            assert!(line.starts_with(CONT_INDENT), "블록 경계 구조 계약: {line:?}");
        }
    }

    #[test]
    fn missing_or_non_string_path_is_skipped() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({}));
        s.record_mutation(&serde_json::json!({"path": 42}));
        assert!(s.on_turn(&ctx(5, false, true, false)).unwrap().contains("none yet"));
    }
}
