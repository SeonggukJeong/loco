use std::path::PathBuf;

use serde::Deserialize;

use super::list_files::walker;
use super::path::confine;
use super::{Tool, ToolCtx, ToolError};

/// 최대 매치 수 (스펙 §4)
pub const MAX_MATCHES: usize = 50;
/// 매치당 전후 컨텍스트 줄 수 (스펙 §4)
const CONTEXT: usize = 2;

pub struct Grep;

#[derive(Deserialize)]
struct Args {
    pattern: String,
    path: Option<String>,
}

impl Tool for Grep {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn doc(&self) -> &'static str {
        "grep(pattern, path?): Search file contents with a regex under `path` (default: project root). Shows up to 50 matching lines with 2 context lines, formatted `file:line: text`."
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::BadArgs(e.to_string()))?;
        let re = regex::Regex::new(&args.pattern)
            .map_err(|e| ToolError::BadArgs(format!("invalid regex: {e}")))?;
        let base = confine(&ctx.root, args.path.as_deref().unwrap_or(""))?;
        let canon_root = ctx.root.canonicalize()?;

        let files: Vec<PathBuf> = if base.is_file() {
            vec![base]
        } else {
            walker(&base, None)
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
                .map(|e| e.into_path())
                .collect()
        };

        let mut out = String::new();
        let mut matches = 0;
        let mut truncated = false;
        'files: for file in files {
            let Ok(bytes) = std::fs::read(&file) else { continue };
            // 바이너리/비UTF-8 파일은 조용히 건너뛴다
            let Ok(text) = String::from_utf8(bytes) else { continue };
            let rel = file
                .strip_prefix(&canon_root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            let lines: Vec<&str> = text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if !re.is_match(line) {
                    continue;
                }
                if matches == MAX_MATCHES {
                    truncated = true;
                    break 'files;
                }
                matches += 1;
                if !out.is_empty() {
                    out.push_str("--\n");
                }
                let start = i.saturating_sub(CONTEXT);
                let end = (i + CONTEXT + 1).min(lines.len());
                // 인덱스 루프는 clippy::needless_range_loop에 걸린다 (-D warnings 게이트)
                for (j, ctx_line) in lines.iter().enumerate().take(end).skip(start) {
                    let sep = if j == i { ':' } else { '-' };
                    out.push_str(&format!("{rel}{sep}{}{sep} {}\n", j + 1, ctx_line));
                }
            }
        }
        if matches == 0 {
            return Ok("no matches".to_string());
        }
        if truncated {
            out.push_str(&format!("[more matches truncated at {MAX_MATCHES}]\n"));
        }
        Ok(out.trim_end().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{Tool, ToolCtx, ToolError};

    fn setup() -> (tempfile::TempDir, ToolCtx) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/a.rs"),
            "line one\nfn target() {}\nline three\nline four\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("src/b.rs"), "nothing here\n").unwrap();
        let ctx = ToolCtx { root: dir.path().to_path_buf() };
        (dir, ctx)
    }

    fn run(ctx: &ToolCtx, args: serde_json::Value) -> Result<String, ToolError> {
        Grep.run(&args, ctx)
    }

    #[test]
    fn match_shows_line_number_and_context() {
        let (_d, ctx) = setup();
        let out = run(&ctx, serde_json::json!({"pattern": "fn target"})).unwrap();
        assert!(out.contains("src/a.rs:2: fn target() {}"), "{out}");
        assert!(out.contains("src/a.rs-1- line one"), "앞 컨텍스트: {out}");
        assert!(out.contains("src/a.rs-4- line four"), "뒤 컨텍스트 2줄: {out}");
        assert!(!out.contains("b.rs"), "매치 없는 파일 제외: {out}");
    }

    #[test]
    fn caps_matches_at_50() {
        let dir = tempfile::tempdir().unwrap();
        let body: String = (1..=60).map(|i| format!("hit {i}\n")).collect();
        std::fs::write(dir.path().join("many.txt"), body).unwrap();
        let ctx = ToolCtx { root: dir.path().to_path_buf() };
        let out = run(&ctx, serde_json::json!({"pattern": "hit"})).unwrap();
        assert_eq!(out.matches("many.txt:").count(), MAX_MATCHES, "{out}");
        assert!(out.contains("[more matches truncated at 50]"), "{out}");
    }

    #[test]
    fn invalid_regex_is_bad_args() {
        let (_d, ctx) = setup();
        let err = run(&ctx, serde_json::json!({"pattern": "["})).unwrap_err();
        assert!(matches!(err, ToolError::BadArgs(_)));
        assert!(err.to_string().contains("invalid regex"));
    }

    #[test]
    fn binary_files_are_skipped() {
        let (dir, ctx) = setup();
        std::fs::write(dir.path().join("bin.dat"), [0xFF, 0x00, b'f', b'n']).unwrap();
        let out = run(&ctx, serde_json::json!({"pattern": "fn"})).unwrap();
        assert!(!out.contains("bin.dat"), "{out}");
    }

    #[test]
    fn no_match_says_so() {
        let (_d, ctx) = setup();
        assert_eq!(run(&ctx, serde_json::json!({"pattern": "zzz_none"})).unwrap(), "no matches");
    }

    #[test]
    fn path_can_target_a_single_file() {
        let (_d, ctx) = setup();
        let out = run(&ctx, serde_json::json!({"pattern": "one", "path": "src/a.rs"})).unwrap();
        assert!(out.contains("src/a.rs:1"), "{out}");
    }
}
