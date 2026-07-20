//! 세션 기록(스펙 §7)과 대화 상태(Task 10에서 Session 추가).
//! 기록은 최선 노력이다 — 기록 실패가 에이전트를 죽여선 안 된다.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Unix epoch 초 → "YYYYMMDDTHHMMSSZ" (ISO8601 basic — Windows 파일명에 `:` 불가).
/// chrono 없이 (의존성 고정): Howard Hinnant의 civil_from_days 알고리즘
pub fn utc_stamp(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let secs = unix_secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z", secs / 3600, (secs % 3600) / 60, secs % 60)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

use crate::llm::types::ChatMessage;

/// 모델에게 가는 생략 마커 — 영어 (스펙 §6의 "[결과 생략]"의 영어 구현)
pub const ELIDED: &str = "[tool result elided]";

/// bytes/4 휴리스틱 (스펙 §6 — chars/4는 한국어에서 과소추정)
pub fn estimate_tokens(s: &str) -> usize {
    s.len() / 4
}

/// 툴 결과 user 래핑 (스펙 §3 — role:"tool" 금지). agent에서 이동
pub fn tool_result_message(tool: &str, body: &str) -> ChatMessage {
    ChatMessage::user(format!("<tool_result name=\"{tool}\">\n{body}\n</tool_result>"))
}

/// 전체 복제 스냅샷. M2의 {len, tail} 방식은 pack()이 런 중 히스토리를 줄일 수 있는
/// M3에서 위험하다: len이 스냅샷보다 작아지면 truncate가 no-op이 되고 스테일 tail이
/// 무관한 현재 메시지를 덮어쓴다. 히스토리는 예산 상한(≈5.5K토큰 ≈ 22KB)이라
/// 복제 비용은 무시 가능
pub struct Snapshot {
    messages: Vec<ChatMessage>,
}

/// 대화 상태의 소유자: 히스토리 + 트랜스크립트 + §6 예산 패킹
pub struct Session {
    messages: Vec<ChatMessage>,
    transcript: Transcript,
}

impl Session {
    pub fn new(initial: Vec<ChatMessage>, mut transcript: Transcript) -> Session {
        for m in &initial {
            transcript.record(&m.role, &m.content);
        }
        Session { messages: initial, transcript }
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn push(&mut self, msg: ChatMessage) {
        self.transcript.record(&msg.role, &msg.content);
        self.messages.push(msg);
    }

    /// 툴 결과 + 선택적 교정 노트를 **하나의** user 메시지로 (스펙 §3 병합 규칙)
    pub fn push_tool_result(&mut self, tool: &str, args: &serde_json::Value, body: &str, note: Option<&str>) {
        self.transcript.record_tool(tool, args, body);
        let mut msg = tool_result_message(tool, body);
        if let Some(n) = note {
            self.transcript.record("user", n);
            msg.content = format!("{}\n\n{}", msg.content, n);
        }
        self.messages.push(msg);
    }

    /// 사용자 요청 — 꼬리가 user면 병합 (스펙 §3 role 교대), 아니면 push
    pub fn push_user_request(&mut self, request: &str) {
        self.transcript.record("user", request);
        match self.messages.last_mut() {
            Some(m) if m.role == "user" => m.content = format!("{}\n\n{}", m.content, request),
            _ => self.messages.push(ChatMessage::user(request)),
        }
    }

    /// §4-2-1 회복 문구 — 꼬리가 이미 같은 문구로 끝나면 **아무것도 하지 않는다**.
    /// 이 문구는 `</tool_result>` 뒤 접미에 병합되는데 `pack()`의 축약이 그 접미를
    /// 의도적으로 보존하므로(:133~136), 연속 주입분은 회수 경로가 없다.
    /// 교대 형태(사이에 다른 턴이 끼는 경우)의 사본은 서로 다른 메시지에 실려
    /// 쌍 삭제가 걷어내므로 여기서 막지 않는다
    pub fn push_recovery_notice(&mut self, notice: &str) {
        if self.messages.last().is_some_and(|m| m.role == "user" && m.content.ends_with(notice)) {
            return;
        }
        self.push_user_request(notice);
    }

    /// 히스토리에 넣지 않는 부가 기록 (/chat 경로 등)
    pub fn record_extra(&mut self, kind: &str, content: &str) {
        self.transcript.record(kind, content);
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot { messages: self.messages.clone() }
    }

    /// 실패/중단 롤백 — 요청 이전 상태로 완전 복원 (꼬리 병합·pack 절삭 모두 안전)
    pub fn rollback(&mut self, snap: Snapshot) {
        self.messages = snap.messages;
    }

    fn total_tokens(&self) -> usize {
        self.messages.iter().map(|m| estimate_tokens(&m.content)).sum()
    }

    /// §6 절삭: ① 오래된 툴 결과 본문 생략 → ② 오래된 user+assistant 쌍 원자 제거.
    /// 시스템 프롬프트(0)와 마지막 메시지(현재 요청/결과)는 보존.
    /// 저장 히스토리 자체를 변형한다 — 원문은 트랜스크립트에 이미 있음
    pub fn pack(&mut self, input_budget_tokens: usize) {
        let last = self.messages.len().saturating_sub(1);
        for i in 1..last {
            if self.total_tokens() <= input_budget_tokens {
                return;
            }
            let m = &mut self.messages[i];
            if m.role == "user" && m.content.starts_with("<tool_result") && !m.content.contains(ELIDED) {
                let first_line = m.content.lines().next().unwrap_or("<tool_result>").to_string();
                // 본문만 생략하고 `</tool_result>` 뒤에 병합된 내용(push_tool_result의 교정
                // 노트, push_user_request의 후속 요청)은 보존한다 — 없으면 빈 문자열
                let suffix = m.content.split_once("</tool_result>").map(|(_, s)| s).unwrap_or("");
                m.content = format!("{first_line}\n{ELIDED}\n</tool_result>{suffix}");
            }
        }
        while self.total_tokens() > input_budget_tokens && self.messages.len() > 3 {
            if self.messages[1].role == "user" && self.messages[2].role == "assistant" {
                self.messages.drain(1..=2);
            } else {
                self.messages.remove(1); // 교대가 어긋난 히스토리 — 하나씩 걷어내고 병합으로 복구
            }
            merge_adjacent_same_role(&mut self.messages);
        }
    }

    /// M11 §4 최신만 유지 — 저장 히스토리에서 기존 상태선 블록을 제거한다.
    /// pack()의 생략 단계는 `</tool_result>` 뒤 접미(병합 노트)를 보존하므로 옛
    /// 상태선은 축약으로 회수되지 않는다 — 새 주입 직전에 이 메서드로 걷어내
    /// 문맥에 상태선이 항상 최대 1개이게 한다. 탐색은 각 user 메시지의 마지막
    /// `</tool_result>` 이후 접미로 한정(툴 body 안의 가짜 마커 보호), 블록 =
    /// 마커 줄 + 이어지는 CONT_INDENT 들여쓴 줄, 블록 뒤 텍스트(병합된 후속
    /// 요청)는 보존. 트랜스크립트는 원본 유지(pack()과 같은 제자리 변형)
    pub fn remove_status_note(&mut self) {
        use crate::agent::status_note::{CONT_INDENT, STATUS_MARKER};
        for m in &mut self.messages {
            if m.role != "user" {
                continue;
            }
            let Some(close) = m.content.rfind("</tool_result>") else { continue };
            let split_at = close + "</tool_result>".len();
            if !m.content[split_at..].contains(STATUS_MARKER) {
                continue;
            }
            let (head, suffix) = m.content.split_at(split_at);
            let mut in_block = false;
            let kept: Vec<&str> = suffix
                .lines()
                .filter(|line| {
                    if line.starts_with(STATUS_MARKER) {
                        in_block = true;
                        false
                    } else if in_block && line.starts_with(CONT_INDENT) {
                        false
                    } else {
                        in_block = false;
                        true
                    }
                })
                .collect();
            let mut new_suffix = kept.join("\n");
            while new_suffix.ends_with('\n') {
                new_suffix.pop();
            }
            m.content = format!("{head}{new_suffix}");
        }
    }
}

/// 쌍 제거 후 교대 재검증 — 인접 동일 role은 병합 (스펙 §6)
fn merge_adjacent_same_role(msgs: &mut Vec<ChatMessage>) {
    let mut i = 1;
    while i < msgs.len() {
        if msgs[i].role == msgs[i - 1].role && msgs[i].role != "system" {
            let taken = msgs.remove(i);
            msgs[i - 1].content = format!("{}\n\n{}", msgs[i - 1].content, taken.content);
        } else {
            i += 1;
        }
    }
}

pub struct Transcript {
    file: Option<File>,
    path: Option<PathBuf>,
}

impl Transcript {
    /// `<root>/.loco/sessions/<stamp>.jsonl` 생성 + `.loco/.gitignore`(`*`) 보장.
    /// 같은 초에 두 세션이 열리면 `-1`, `-2`… 접미로 회피
    pub fn create_under(root: &Path) -> std::io::Result<Transcript> {
        let dir = root.join(".loco/sessions");
        std::fs::create_dir_all(&dir)?;
        let gitignore = root.join(".loco/.gitignore");
        if !gitignore.exists() {
            std::fs::write(&gitignore, "*\n")?;
        }
        let stamp = utc_stamp(now_secs());
        for suffix in 0..10 {
            let name = if suffix == 0 { format!("{stamp}.jsonl") } else { format!("{stamp}-{suffix}.jsonl") };
            let path = dir.join(&name);
            match File::create_new(&path) {
                Ok(file) => return Ok(Transcript { file: Some(file), path: Some(path) }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::other("세션 파일 이름 충돌이 반복됨"))
    }

    /// 기록 없이 동작 (기록 디렉터리 생성 실패 시 폴백)
    pub fn disabled() -> Transcript {
        Transcript { file: None, path: None }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn write(&mut self, value: serde_json::Value) {
        if let Some(f) = &mut self.file {
            let _ = writeln!(f, "{value}"); // 최선 노력 — 실패 무시
        }
    }

    /// kind: user | assistant | system (스펙 §7)
    pub fn record(&mut self, kind: &str, content: &str) {
        self.write(serde_json::json!({"ts": utc_stamp(now_secs()), "kind": kind, "content": content}));
    }

    pub fn record_tool(&mut self, tool: &str, args: &serde_json::Value, content: &str) {
        self.write(serde_json::json!({
            "ts": utc_stamp(now_secs()), "kind": "tool_result", "content": content,
            "tool": tool, "args": args,
        }));
    }

    /// 지정 경로에 트랜스크립트 생성 — eval이 리포트 디렉터리에 실행별 기록을 남긴다
    pub fn create_at(path: &Path) -> std::io::Result<Transcript> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create_new(path)?;
        Ok(Transcript { file: Some(file), path: Some(path.to_path_buf()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_stamp_known_values() {
        assert_eq!(utc_stamp(0), "19700101T000000Z");
        assert_eq!(utc_stamp(86_399), "19700101T235959Z");
        assert_eq!(utc_stamp(951_782_400), "20000229T000000Z", "윤일");
    }

    #[test]
    fn create_under_makes_sessions_dir_and_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let t = Transcript::create_under(dir.path()).unwrap();
        let p = t.path().unwrap().to_path_buf();
        assert!(p.starts_with(dir.path().join(".loco/sessions")));
        assert_eq!(p.extension().unwrap(), "jsonl");
        let gi = std::fs::read_to_string(dir.path().join(".loco/.gitignore")).unwrap();
        assert_eq!(gi.trim(), "*", "커밋 오염 방지 (스펙 §7)");
    }

    #[test]
    fn records_are_one_json_per_line() {
        let dir = tempfile::tempdir().unwrap();
        let mut t = Transcript::create_under(dir.path()).unwrap();
        t.record("user", "질문");
        t.record_tool("read_file", &serde_json::json!({"path": "a.rs"}), "내용");
        let text = std::fs::read_to_string(t.path().unwrap()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["kind"], "user");
        assert_eq!(first["content"], "질문");
        assert!(first["ts"].as_str().unwrap().ends_with('Z'));
        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["kind"], "tool_result");
        assert_eq!(second["tool"], "read_file");
        assert_eq!(second["args"]["path"], "a.rs");
    }

    #[test]
    fn disabled_transcript_swallows_records() {
        let mut t = Transcript::disabled();
        t.record("user", "x"); // 패닉/에러 없어야 함
        assert!(t.path().is_none());
    }

    #[test]
    fn same_second_sessions_get_distinct_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = Transcript::create_under(dir.path()).unwrap();
        let b = Transcript::create_under(dir.path()).unwrap();
        assert_ne!(a.path().unwrap(), b.path().unwrap());
    }

    #[test]
    fn create_at_writes_to_the_given_path() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/run-x-0.jsonl");
        let mut t = Transcript::create_at(&p).unwrap();
        t.record("user", "질문");
        assert!(std::fs::read_to_string(&p).unwrap().contains("질문"));
    }

    use crate::llm::types::ChatMessage;

    fn sess(msgs: Vec<ChatMessage>) -> Session {
        Session::new(msgs, Transcript::disabled())
    }

    fn tool_msg(body: &str) -> ChatMessage {
        tool_result_message("grep", body)
    }

    #[test]
    fn estimate_is_utf8_bytes_over_four() {
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("한글"), 1, "한글 1자=3바이트 (스펙 §6)");
    }

    #[test]
    fn pack_under_budget_is_a_noop() {
        let mut s = sess(vec![ChatMessage::system("sys"), ChatMessage::user("hi")]);
        s.pack(1_000);
        assert_eq!(s.messages().len(), 2);
    }

    #[test]
    fn pack_elides_oldest_tool_results_first() {
        let big = "x".repeat(4_000); // ≈1000토큰
        let mut s = sess(vec![
            ChatMessage::system("sys"),
            ChatMessage::user("q"),
            ChatMessage::assistant("t1"),
            tool_msg(&big),
            ChatMessage::assistant("t2"),
            tool_msg(&big),
            ChatMessage::assistant("t3"),
        ]);
        s.pack(1_200);
        let elided: Vec<_> = s.messages().iter().filter(|m| m.content.contains(ELIDED)).collect();
        assert!(!elided.is_empty(), "오래된 툴 결과부터 생략");
        assert!(elided[0].content.starts_with("<tool_result"), "래퍼 보존:\n{}", elided[0].content);
        assert_eq!(s.messages().len(), 7, "생략 단계에선 메시지를 제거하지 않음");
    }

    #[test]
    fn pack_then_drops_oldest_user_assistant_pairs_atomically() {
        let mut msgs = vec![ChatMessage::system("sys")];
        for i in 0..10 {
            msgs.push(ChatMessage::user(format!("질문{} {}", i, "y".repeat(2_000))));
            msgs.push(ChatMessage::assistant(format!("답{} {}", i, "y".repeat(2_000))));
        }
        let mut s = sess(msgs);
        s.pack(1_000);
        assert!(s.messages().len() < 21);
        assert_eq!(s.messages()[0].role, "system", "시스템 프롬프트 보존");
        // role 교대 보존 (스펙 §6 — 쌍 단위 제거)
        for w in s.messages().windows(2) {
            assert!(!(w[0].role == w[1].role && w[0].role != "system"), "인접 동일 role");
        }
        // 마지막(현재 요청)은 보존
        assert!(s.messages().last().unwrap().content.starts_with("답9"));
    }

    #[test]
    fn snapshot_rollback_restores_tail_merge() {
        let mut s = sess(vec![ChatMessage::system("sys"), tool_msg("결과")]);
        let snap = s.snapshot();
        s.push_user_request("이어서"); // 꼬리 user에 병합 (길이 불변)
        assert!(s.messages().last().unwrap().content.contains("이어서"));
        s.rollback(snap);
        assert!(!s.messages().last().unwrap().content.contains("이어서"), "병합 원복");
        assert_eq!(s.messages().len(), 2);
    }

    #[test]
    fn rollback_after_pack_restores_exactly() {
        let mut msgs = vec![ChatMessage::system("sys")];
        for i in 0..4 {
            msgs.push(ChatMessage::user(format!("질문{} {}", i, "y".repeat(2_000))));
            msgs.push(ChatMessage::assistant(format!("답{i}")));
        }
        let mut s = sess(msgs.clone());
        let snap = s.snapshot();
        s.pack(500); // 쌍 제거로 히스토리가 스냅샷 시점보다 짧아진다
        assert!(s.messages().len() < msgs.len(), "전제: pack이 실제로 줄였음");
        s.rollback(snap);
        assert_eq!(s.messages(), &msgs[..], "pack 뒤에도 정확히 원복 — {{len,tail}} 방식은 여기서 깨진다");
    }

    #[test]
    fn push_user_request_merges_after_trailing_user() {
        let mut s = sess(vec![ChatMessage::system("sys"), tool_msg("결과")]);
        s.push_user_request("추가 요청");
        assert_eq!(s.messages().len(), 2, "연속 user 금지 — 병합 (스펙 §3)");
        let mut s2 = sess(vec![ChatMessage::system("sys")]);
        s2.push_user_request("첫 요청");
        assert_eq!(s2.messages().len(), 2);
    }

    #[test]
    fn push_tool_result_with_note_appends_in_same_message() {
        let mut s = sess(vec![ChatMessage::system("sys")]);
        s.push_tool_result("grep", &serde_json::json!({}), "body", Some("NOTE"));
        let last = s.messages().last().unwrap();
        assert!(last.content.contains("</tool_result>") && last.content.ends_with("NOTE"));
    }

    #[test]
    fn elision_preserves_merged_user_request() {
        let big = "x".repeat(4_000); // ≈1000토큰 — 생략 대상 크기
        let mut s = sess(vec![ChatMessage::system("sys")]);
        s.push(tool_msg(&big)); // MaxTurns 종료 시점의 마지막 tool_result라고 가정
        s.push_user_request("이어서 이것도 해줘"); // 후속 요청이 꼬리 tool_result에 병합 (스펙 §3)
        assert!(s.messages().last().unwrap().content.contains("이어서 이것도 해줘"), "전제: 병합됨");
        // 세션이 이어지며 메시지가 더 쌓여, 병합된 메시지가 더 이상 "마지막"이 아니게 된다
        s.push(ChatMessage::assistant("계속"));
        s.push_tool_result("grep", &serde_json::json!({}), &"z".repeat(4_000), None);
        s.push(ChatMessage::assistant("finish"));

        s.pack(800); // 예산을 낮춰 오래된 tool_result 본문을 생략시킨다

        let merged = s
            .messages()
            .iter()
            .find(|m| m.content.contains("이어서 이것도 해줘"))
            .expect("병합된 유저 요청이 담긴 메시지가 살아있어야 한다");
        assert!(merged.content.contains(ELIDED), "본문은 생략되어야 한다: {}", merged.content);
    }

    #[test]
    fn remove_status_note_strips_only_the_status_block() {
        let mut s = sess(vec![ChatMessage::system("sys")]);
        s.push_tool_result(
            "edit_file",
            &serde_json::json!({}),
            "Edited a.rs",
            Some("note: fix your args.\n\n[status] files edited: 1 (a.rs)\n         verification: none since your last edit\n         turns: 3 of 25 used"),
        );
        s.remove_status_note();
        let last = s.messages().last().unwrap();
        assert!(!last.content.contains("[status]"), "{}", last.content);
        assert!(last.content.contains("note: fix your args."), "교정 노트 보존: {}", last.content);
        assert!(last.content.contains("Edited a.rs"), "body 보존: {}", last.content);
    }

    #[test]
    fn remove_status_note_ignores_marker_inside_tool_body() {
        // loco 자기 소스 grep 도그푸딩 — body 안의 가짜 마커는 제거 대상이 아니다 (§4 블록 경계 핀)
        let mut s = sess(vec![ChatMessage::system("sys")]);
        s.push_tool_result("grep", &serde_json::json!({}), "src/x.rs:1:[status] files edited: ...", None);
        let before = s.messages().last().unwrap().content.clone();
        s.remove_status_note();
        assert_eq!(s.messages().last().unwrap().content, before, "tool_result 구조 불변");
    }

    #[test]
    fn remove_status_note_preserves_merged_user_request_after_block() {
        // MaxTurns 후 후속 요청이 상태선 블록 뒤에 병합된 경우 (§4 — truncate-to-end 금지)
        let mut s = sess(vec![ChatMessage::system("sys")]);
        s.push_tool_result(
            "read_file",
            &serde_json::json!({}),
            "body",
            Some("[status] files edited: none yet | turns: 25 of 25 used"),
        );
        s.push_user_request("이어서 이것도 해줘");
        s.remove_status_note();
        let last = s.messages().last().unwrap();
        assert!(!last.content.contains("[status]"), "{}", last.content);
        assert!(last.content.contains("이어서 이것도 해줘"), "병합 요청 보존: {}", last.content);
    }

    #[test]
    fn remove_status_note_survives_elided_messages() {
        // pack() 생략 후에도(접미 보존 — session.rs 실의미론) 옛 상태선을 제거할 수 있다
        let big = "x".repeat(4_000);
        let mut s = sess(vec![ChatMessage::system("sys")]);
        s.push_tool_result("read_file", &serde_json::json!({}), &big,
            Some("[status] files edited: none yet | turns: 5 of 25 used"));
        s.push(ChatMessage::assistant("t"));
        s.push_tool_result("read_file", &serde_json::json!({}), &big, None);
        // 예산 1200: 생략 후 총 ≈1041토큰 ≤ 1200이라 쌍 제거 단계 미진입 — 검증 대상
        // 메시지가 살아남는다 (800이면 drain(1..=2)이 메시지째 지워 테스트 전제가 깨짐).
        // 기존 pack_elides_oldest_tool_results_first와 같은 검증된 상수
        s.pack(1_200); // 첫 tool_result 본문 생략 — 접미(상태선)는 pack이 보존한다
        assert!(s.messages().iter().any(|m| m.content.contains(ELIDED) && m.content.contains("[status]")));
        s.remove_status_note();
        assert!(!s.messages().iter().any(|m| m.content.contains("[status]")), "생략 메시지의 접미도 제거");
    }

    #[test]
    fn recovery_notice_is_not_duplicated_when_appended_back_to_back() {
        let mut s = sess(vec![ChatMessage::system("sys"), ChatMessage::user("TASK: fix it")]);
        s.push_recovery_notice("CUT OFF");
        s.push_recovery_notice("CUT OFF");
        s.push_recovery_notice("CUT OFF");
        let joined = s.messages().iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n");
        assert_eq!(joined.matches("CUT OFF").count(), 1, "연속 주입은 1벌만: {joined}");
    }

    #[test]
    fn recovery_notice_merges_into_a_trailing_user_message() {
        let mut s = sess(vec![ChatMessage::system("sys"), ChatMessage::user("TASK")]);
        s.push_recovery_notice("CUT OFF");
        assert_eq!(s.messages().len(), 2, "새 메시지가 아니라 병합이어야 role 교대가 유지된다");
        assert!(s.messages()[1].content.ends_with("CUT OFF"));
    }

    #[test]
    fn recovery_notice_pushes_when_the_tail_is_an_assistant_message() {
        let mut s = sess(vec![ChatMessage::system("sys"), ChatMessage::assistant("a")]);
        s.push_recovery_notice("CUT OFF");
        assert_eq!(s.messages().len(), 3);
        assert_eq!(s.messages()[2].role, "user");
    }
}
