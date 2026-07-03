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
}
