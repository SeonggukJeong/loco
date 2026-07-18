use serde::Deserialize;

use super::exec::{exec_shell, ExecEnd};
use super::{Tool, ToolCtx, ToolError};

pub struct RunCommand;

#[derive(Deserialize)]
struct Args {
    command: String,
}

/// M11 §5: 따옴표 밖 파이프 존재 판정 — `||`(OR)는 파이프가 아니고, 따옴표
/// 안·백슬래시 이스케이프된 `|`는 무시한다(grep 패턴 상용 — 오발 방지).
/// 잔여 이스케이프 엣지 케이스의 오발은 허용 오차(정보 한 줄, 무해)
fn has_unquoted_pipe(cmd: &str) -> bool {
    let bytes = cmd.as_bytes();
    let (mut in_single, mut in_double) = (false, false);
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if !in_single => {
                i += 2; // 다음 문자는 이스케이프됨 (single quote 안에서만 리터럴)
                continue;
            }
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'|' if !in_single && !in_double => {
                if bytes.get(i + 1) == Some(&b'|') {
                    i += 2; // `||` — OR 연산자
                    continue;
                }
                return true;
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// M12 §2-2 — 필터가 아무 테스트도 못 맞힌 exit 0을 "검증"으로 읽지 못하게 하는 노트.
/// filtered_out > 0 조건이 "테스트가 원래 없는 크레이트"(정상)와 구분한다
fn empty_test_note(body: &str, exit_code: &str) -> Option<String> {
    if exit_code != "0" {
        return None;
    }
    let s = crate::test_summary::parse_test_summary(body)?;
    (s.ran == 0 && s.filtered_out > 0).then(|| {
        format!(
            "note: 0 tests ran ({} filtered out) - cargo test filters by test NAME, \
             not file name; this exit 0 did not verify anything",
            s.filtered_out
        )
    })
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
                let mut out = format!("exit code: {code}\n{}", exec.body);
                if has_unquoted_pipe(&args.command) {
                    out.push_str(
                        "\nnote: this command is a pipeline - the exit code reflects only the last command in the pipe",
                    );
                }
                if let Some(note) = empty_test_note(&out, &code) {
                    out.push('\n');
                    out.push_str(&note);
                }
                out
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

        #[test]
        fn pipeline_gets_exit_code_provenance_note() {
            let (_d, ctx) = ctx();
            let out = RunCommand
                .run(&serde_json::json!({"command": "false | cat"}), &ctx)
                .unwrap();
            assert!(out.starts_with("exit code: 0"), "파이프 exit는 마지막 명령의 것: {out}");
            assert!(out.contains("note: this command is a pipeline"), "{out}");
        }

        #[test]
        fn non_pipeline_commands_get_no_note() {
            let (_d, ctx) = ctx();
            for cmd in ["echo hi", "grep -c 'a\\|b' /dev/null || true", "true || false"] {
                let out = RunCommand.run(&serde_json::json!({"command": cmd}), &ctx).unwrap();
                assert!(!out.contains("note: this command"), "{cmd} → {out}");
            }
        }

        #[test]
        fn timed_out_command_gets_no_pipeline_note() {
            let (_d, mut c) = ctx();
            c.command_timeout = Duration::from_millis(300);
            let out = RunCommand
                .run(&serde_json::json!({"command": "sleep 30 | cat"}), &c)
                .unwrap();
            assert!(out.contains("timed out") && !out.contains("note: this command"), "{out}");
        }
    }

    #[test]
    fn unquoted_pipe_detection() {
        assert!(super::has_unquoted_pipe("cargo test 2>&1 | tail -50"));
        assert!(!super::has_unquoted_pipe(r#"grep "a\|b" f.rs"#));
        assert!(!super::has_unquoted_pipe("grep 'x|y' f.rs"));
        assert!(!super::has_unquoted_pipe(r"grep a\|b f.rs"), "따옴표 밖 백슬래시 이스케이프");
        assert!(
            super::has_unquoted_pipe(r"echo 'a\' | cat"),
            "단일따옴표 안 백슬래시는 리터럴 — 따옴표가 닫히고 파이프는 실파이프"
        );
        assert!(!super::has_unquoted_pipe("a || b"), "OR 연산자는 파이프가 아님");
        assert!(super::has_unquoted_pipe("a | b || c"), "혼합 — 실파이프가 있으면 참");
        assert!(!super::has_unquoted_pipe("echo x"));
    }

    #[test]
    fn empty_test_run_with_exit_zero_gets_the_invalidation_note() {
        let body = "exit code: 0\n\
running 0 tests\n\
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.00s\n";
        let note = super::empty_test_note(body, "0");
        assert_eq!(
            note.as_deref(),
            Some("note: 0 tests ran (13 filtered out) - cargo test filters by test NAME, not file name; this exit 0 did not verify anything")
        );
    }

    #[test]
    fn a_crate_with_no_tests_at_all_gets_no_note() {
        // filtered_out == 0 이면 "원래 테스트가 없는 크레이트" — 정상이므로 침묵
        let body = "exit code: 0\n\
running 0 tests\n\
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        assert!(super::empty_test_note(body, "0").is_none());
    }

    #[test]
    fn a_real_test_run_gets_no_note() {
        let body = "exit code: 0\n\
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s\n";
        assert!(super::empty_test_note(body, "0").is_none());
    }

    #[test]
    fn non_zero_exit_gets_no_note() {
        // 실패 exit는 자체 신호가 이미 있다 (§2-2)
        let body = "exit code: 101\n\
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s\n";
        assert!(super::empty_test_note(body, "101").is_none());
    }

    #[test]
    fn non_cargo_output_gets_no_note() {
        assert!(super::empty_test_note("exit code: 0\nhello\n", "0").is_none());
    }
}
