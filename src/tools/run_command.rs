use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::{Tool, ToolCtx, ToolError};

/// stdout+stderr 합산 상한 (바이트). 초과분은 가운데를 잘라낸다 —
/// 명령 에코는 앞에, 에러 요약은 뒤에 있는 경우가 많다
const MAX_OUTPUT_BYTES: usize = 8_000;
/// try_wait 폴링 간격
const POLL: Duration = Duration::from_millis(50);
/// 종료 판정 후 파이프 리더 대기 상한. join()은 금지 — 백그라운드 손자가
/// 파이프를 물고 있으면(`sh -c "x &"` 또는 그룹 킬 실패) EOF가 영원히 안 와서
/// 툴이 무한 대기한다. 상한 초과 시 해당 출력은 포기하고 안내를 남긴다
const READER_GRACE: Duration = Duration::from_millis(500);

pub struct RunCommand;

#[derive(Deserialize)]
struct Args {
    command: String,
}

/// UTF-8 우선, 실패 시 EUC-KR(windows-949) 손실 디코딩 (스펙 §10 — 한국어 Windows 콘솔)
fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::EUC_KR.decode(bytes).0.into_owned(),
    }
}

fn truncate_middle(s: &str) -> String {
    if s.len() <= MAX_OUTPUT_BYTES {
        return s.to_string();
    }
    let mut head = MAX_OUTPUT_BYTES / 2;
    while !s.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = s.len() - MAX_OUTPUT_BYTES / 2;
    while !s.is_char_boundary(tail) {
        tail += 1;
    }
    format!(
        "{}\n[... output truncated ({} bytes total) ...]\n{}",
        &s[..head],
        s.len(),
        &s[tail..]
    )
}

/// 파이프 리더 — 결과를 채널로 보낸다. JoinHandle::join 대신 recv_timeout을
/// 쓸 수 있게 하기 위함 (READER_GRACE 주석 참조)
fn spawn_reader(r: Option<impl Read + Send + 'static>) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut r) = r {
            let _ = r.read_to_end(&mut buf);
        }
        let _ = tx.send(buf);
    });
    rx
}

/// (디코딩된 출력, 제시간에 EOF를 받았는지)
fn drain(rx: std::sync::mpsc::Receiver<Vec<u8>>) -> (String, bool) {
    match rx.recv_timeout(READER_GRACE) {
        Ok(bytes) => (decode(&bytes), true),
        Err(_) => (String::new(), false),
    }
}

fn shell_command(command: &str, ctx: &ToolCtx) -> Command {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut c = Command::new("sh");
        c.arg("-c").arg(command).current_dir(&ctx.root);
        // 자기만의 프로세스 그룹 — 타임아웃 킬이 손자까지 잡게 (스펙 §10)
        c.process_group(0);
        c
    }
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.args(["/C", command]).current_dir(&ctx.root);
        c
    }
}

/// 프로세스 트리 킬 (스펙 §10). 신규 크레이트 없이 시스템 유틸로:
/// Unix는 프로세스 그룹에 kill -9, Windows는 taskkill /T /F
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").args(["-9", "--", &format!("-{}", child.id())]).status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()]).status();
    }
    let _ = child.kill(); // 그룹 킬 실패 대비 직접 킬
}

enum Ended {
    Done(std::process::ExitStatus),
    TimedOut,
    Cancelled,
}

impl Tool for RunCommand {
    fn name(&self) -> &'static str {
        "run_command"
    }

    fn doc(&self) -> &'static str {
        "run_command(command): Run a shell command from the project root and return its exit code and output. Long output is truncated; commands are killed after the configured timeout."
    }

    fn is_mutating(&self) -> bool {
        true
    }

    fn preview(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone()).map_err(|e| ToolError::BadArgs(e.to_string()))?;
        Ok(format!(
            "$ {}\n(cwd: 프로젝트 루트, 타임아웃: {}초)",
            args.command,
            ctx.command_timeout.as_secs()
        ))
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone()).map_err(|e| ToolError::BadArgs(e.to_string()))?;
        let mut child = shell_command(&args.command, ctx)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let out_rx = spawn_reader(child.stdout.take());
        let err_rx = spawn_reader(child.stderr.take());

        let start = Instant::now();
        let ended = loop {
            if let Some(status) = child.try_wait()? {
                break Ended::Done(status);
            }
            if ctx.cancel.load(Ordering::SeqCst) {
                kill_tree(&mut child);
                let _ = child.wait();
                break Ended::Cancelled;
            }
            if start.elapsed() >= ctx.command_timeout {
                kill_tree(&mut child);
                let _ = child.wait();
                break Ended::TimedOut;
            }
            std::thread::sleep(POLL);
        };

        let (stdout, out_ok) = drain(out_rx);
        let (stderr, err_ok) = drain(err_rx);
        let mut body = String::new();
        if !stdout.trim().is_empty() {
            body.push_str("--- stdout ---\n");
            body.push_str(&stdout);
        }
        if !stderr.trim().is_empty() {
            if !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            body.push_str("--- stderr ---\n");
            body.push_str(&stderr);
        }
        if !out_ok || !err_ok {
            if !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            body.push_str("(some output unavailable - a background child still holds the pipe)");
        }
        let body = truncate_middle(&body);

        Ok(match ended {
            Ended::Done(status) => {
                let code = status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "(terminated by signal)".to_string());
                format!("exit code: {code}\n{body}")
            }
            Ended::TimedOut => format!(
                "command timed out after {}s and was killed\n{body}",
                ctx.command_timeout.as_secs()
            ),
            Ended::Cancelled => format!("command was cancelled by the user\n{body}"),
        })
    }
}

#[cfg(test)]
mod tests {
    // 주의: 외부 mod에는 decode/truncate_middle 테스트만 있다 — Tool/ToolCtx를
    // 여기서 import하면 unused import로 -D warnings 게이트에 걸린다 (unix 서브모듈이 자체 import)
    use super::*;

    #[test]
    fn decode_falls_back_to_euc_kr() {
        assert_eq!(decode("한글".as_bytes()), "한글");
        // "한글"의 CP949 인코딩: C7 D1 B1 DB
        assert_eq!(decode(&[0xC7, 0xD1, 0xB1, 0xDB]), "한글");
    }

    #[test]
    fn truncate_middle_keeps_head_and_tail() {
        let s = "x".repeat(20_000);
        let t = truncate_middle(&s);
        assert!(t.len() < 12_000);
        assert!(t.contains("output truncated"));
        let short = "short";
        assert_eq!(truncate_middle(short), "short");
    }

    #[cfg(unix)]
    mod unix {
        use super::super::*;
        use crate::tools::{Tool, ToolCtx};
        use std::time::{Duration, Instant};

        fn ctx() -> (tempfile::TempDir, ToolCtx) {
            let dir = tempfile::tempdir().unwrap();
            let ctx = ToolCtx::new(dir.path().to_path_buf());
            (dir, ctx)
        }

        #[test]
        fn runs_in_project_root_and_reports_exit_code() {
            let (dir, ctx) = ctx();
            std::fs::write(dir.path().join("here.txt"), "").unwrap();
            let out = RunCommand.run(&serde_json::json!({"command": "ls"}), &ctx).unwrap();
            assert!(out.contains("exit code: 0"), "{out}");
            assert!(out.contains("here.txt"), "cwd는 프로젝트 루트: {out}");
            let fail = RunCommand.run(&serde_json::json!({"command": "exit 3"}), &ctx).unwrap();
            assert!(fail.contains("exit code: 3"), "{fail}");
        }

        #[test]
        fn stderr_is_captured() {
            let (_d, ctx) = ctx();
            let out = RunCommand
                .run(&serde_json::json!({"command": "echo oops 1>&2"}), &ctx)
                .unwrap();
            assert!(out.contains("oops"), "{out}");
        }

        #[test]
        fn timeout_kills_the_process_tree_promptly() {
            let (_d, mut c) = ctx();
            c.command_timeout = Duration::from_millis(300);
            let start = Instant::now();
            let out = RunCommand.run(&serde_json::json!({"command": "sleep 30"}), &c).unwrap();
            assert!(start.elapsed() < Duration::from_secs(5), "타임아웃 후 즉시 반환");
            assert!(out.contains("timed out"), "{out}");
        }

        #[test]
        fn cancel_flag_aborts_early() {
            let (_d, ctx) = ctx();
            let cancel = ctx.cancel.clone();
            let h = std::thread::spawn(move || {
                RunCommand.run(&serde_json::json!({"command": "sleep 30"}), &ctx)
            });
            std::thread::sleep(Duration::from_millis(200));
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            let start = Instant::now();
            let out = h.join().unwrap().unwrap();
            assert!(start.elapsed() < Duration::from_secs(5));
            assert!(out.contains("cancelled"), "{out}");
        }

        #[test]
        fn background_grandchild_does_not_hang_the_tool() {
            // sh는 즉시 종료하지만 sleep이 stdout 파이프를 물고 남는다 —
            // join() 방식이면 여기서 5초(또는 영원히) 매달린다
            let (_d, ctx) = ctx();
            let start = Instant::now();
            let out = RunCommand.run(&serde_json::json!({"command": "sleep 5 &"}), &ctx).unwrap();
            assert!(start.elapsed() < Duration::from_secs(3), "READER_GRACE 내 반환");
            assert!(out.contains("exit code: 0"), "{out}");
            assert!(out.contains("output unavailable"), "파이프 점유 안내: {out}");
        }

        #[test]
        fn preview_shows_command_and_timeout() {
            let (_d, ctx) = ctx();
            let p = RunCommand.preview(&serde_json::json!({"command": "cargo test"}), &ctx).unwrap();
            assert!(p.contains("cargo test") && p.contains("60"), "{p}");
        }
    }
}
