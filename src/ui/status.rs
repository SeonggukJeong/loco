use std::io::{IsTerminal, Write};

use crate::agent::AgentEvent;

/// 에이전트 턴 대기 표시 (스펙 §3 — 구조화 출력은 스트리밍 불가라 스피너).
/// stderr에 그린다. stdout이 TTY가 아니면(-p 파이프 등) 아무것도 그리지 않는다 (스펙 §7)
pub struct Spinner {
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Spinner {
    pub fn start(label: &str) -> Self {
        if !(std::io::stdout().is_terminal() && std::io::stderr().is_terminal()) {
            return Self { task: None };
        }
        let label = label.to_string();
        let task = tokio::spawn(async move {
            // ASCII 프레임 — 한국어 Windows 콘솔(CP949)에서도 안 깨진다
            const FRAMES: [char; 4] = ['|', '/', '-', '\\'];
            for i in 0.. {
                eprint!("\r{} {label}", FRAMES[i % FRAMES.len()]);
                let _ = std::io::stderr().flush();
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            }
        });
        Self { task: Some(task) }
    }

    pub fn is_active(&self) -> bool {
        self.task.is_some()
    }

    pub fn stop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
            // 스피너 줄을 공백으로 덮어 지운다 (ANSI 없이 — 레거시 콘솔 호환)
            eprint!("\r{:60}\r", "");
            let _ = std::io::stderr().flush();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 읽기 툴 자동 실행 알림 한 줄 (스펙 §5: "→ read_file src/main.rs")
pub fn format_action(tool: &str, args: &serde_json::Value) -> String {
    let detail = match tool {
        "read_file" | "list_files" => args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".")
            .to_string(),
        "grep" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            match args.get("path").and_then(|v| v.as_str()) {
                Some(p) => format!("{pattern:?} {p}"),
                None => format!("{pattern:?}"),
            }
        }
        "write_file" | "edit_file" => args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string(),
        "run_command" => args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string(),
        _ => args.to_string(),
    };
    format!("→ {tool} {detail}")
}

/// main(-p, stderr)과 repl(stdout)이 공유하는 이벤트 한 줄 렌더링
pub fn render_event(ev: &AgentEvent<'_>, to_stderr: bool) {
    let line = match ev {
        AgentEvent::Thought(t) => format!("· {t}"),
        AgentEvent::Action { tool, args } => format_action(tool, args),
        AgentEvent::Notice(n) => n.clone(),
    };
    if to_stderr { eprintln!("{line}") } else { println!("{line}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_lines_are_compact() {
        assert_eq!(
            format_action("read_file", &serde_json::json!({"path": "src/main.rs"})),
            "→ read_file src/main.rs"
        );
        assert_eq!(
            format_action("list_files", &serde_json::json!({})),
            "→ list_files ."
        );
        assert_eq!(
            format_action("grep", &serde_json::json!({"pattern": "fn load", "path": "src"})),
            "→ grep \"fn load\" src"
        );
        assert_eq!(
            format_action("grep", &serde_json::json!({"pattern": "x"})),
            "→ grep \"x\""
        );
        // 모르는 툴은 인자 원문 (M3에서 툴 늘어나도 동작)
        assert_eq!(
            format_action("teleport", &serde_json::json!({"to": "moon"})),
            "→ teleport {\"to\":\"moon\"}"
        );
        assert_eq!(
            format_action("write_file", &serde_json::json!({"path": "a.rs", "content": "..."})),
            "→ write_file a.rs"
        );
        assert_eq!(
            format_action("edit_file", &serde_json::json!({"path": "a.rs", "search": "x", "replace": "y"})),
            "→ edit_file a.rs"
        );
        assert_eq!(
            format_action("run_command", &serde_json::json!({"command": "cargo test"})),
            "→ run_command cargo test"
        );
    }

    #[tokio::test]
    async fn spinner_activity_follows_stdout_tty() {
        // libtest의 출력 캡처는 매크로 수준이라 fd는 그대로다 — 터미널에서 직접
        // cargo test를 치면 TTY일 수 있으므로, 절대값 대신 is_terminal()과의 일치를 검증
        use std::io::IsTerminal;
        let mut s = Spinner::start("생각 중");
        assert_eq!(
            s.is_active(),
            std::io::stdout().is_terminal() && std::io::stderr().is_terminal()
        );
        s.stop();
        assert!(!s.is_active(), "stop 후에는 항상 비활성");
    }

    #[tokio::test]
    async fn spinner_stop_is_idempotent() {
        let mut s = Spinner::start("x");
        s.stop();
        s.stop(); // 두 번째 stop은 no-op — 패닉/잔상 없음
        assert!(!s.is_active());
    }
}
