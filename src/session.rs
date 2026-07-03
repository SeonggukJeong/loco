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

fn now_secs() -> u64 {
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
                m.content = format!("{first_line}\n{ELIDED}\n</tool_result>");
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
}
