use serde::Deserialize;

use super::exec::{exec_shell, ExecEnd};
use super::{Tool, ToolCtx, ToolError};

pub struct RunCommand;

#[derive(Deserialize)]
struct Args {
    command: String,
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
        let exec = exec_shell(&args.command, &ctx.root, ctx.command_timeout, &ctx.cancel)?;
        Ok(match exec.end {
            ExecEnd::Done(status) => {
                let code = status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "(terminated by signal)".to_string());
                format!("exit code: {code}\n{}", exec.body)
            }
            ExecEnd::TimedOut => format!(
                "command timed out after {}s and was killed\n{}",
                ctx.command_timeout.as_secs(),
                exec.body
            ),
            ExecEnd::Cancelled => format!("command was cancelled by the user\n{}", exec.body),
        })
    }
}

#[cfg(test)]
mod tests {
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
