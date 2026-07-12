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
        let (re, fallback_reason) = match regex::Regex::new(&args.pattern) {
            Ok(re) => (re, None),
            Err(e) => {
                // 코드 조각({user_name} 등)이 정규식 파싱에 실패하면 리터럴로 폴백 (M5 §5.3)
                let literal = regex::Regex::new(&regex::escape(&args.pattern))
                    .map_err(|e2| ToolError::BadArgs(format!("invalid regex: {e2}")))?;
                let reason: String = e.to_string().split_whitespace().collect::<Vec<_>>().join(" ");
                // char 경계 패닉 방지 — regex 에러 메시지는 ASCII지만 방어적으로 chars() 사용
                let reason = if reason.chars().count() > 120 {
                    format!("{}...", reason.chars().take(120).collect::<String>())
                } else {
                    reason
                };
                (literal, Some(reason))
            }
        };
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
        if matches == 0 && fallback_reason.is_none() {
            return Ok("no matches".to_string());
        }
        if truncated {
            out.push_str(&format!("[more matches truncated at {MAX_MATCHES}]\n"));
        }
        let body = out.trim_end().to_string();
        if let Some(reason) = fallback_reason {
            let header = format!(
                "invalid regex ({reason}); searched for the literal text instead - {matches} matches"
            );
            return Ok(if body.is_empty() { header } else { format!("{header}\n{body}") });
        }
        Ok(body)
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
        let ctx = ToolCtx::new(dir.path().to_path_buf());
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
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        let out = run(&ctx, serde_json::json!({"pattern": "hit"})).unwrap();
        assert_eq!(out.matches("many.txt:").count(), MAX_MATCHES, "{out}");
        assert!(out.contains("[more matches truncated at 50]"), "{out}");
    }

    #[test]
    fn invalid_regex_falls_back_to_literal_search() {
        // gemma multiline-string-edit 실측: 리터럴 {user_name}가 정규식 파싱 실패
        let (_d, ctx) = setup();
        std::fs::write(_d.path().join("src/c.rs"), "user: {user_name}\n").unwrap();
        let out = Grep.run(&serde_json::json!({"pattern": "{user_name}"}), &ctx).unwrap();
        assert!(out.contains("invalid regex"), "{out}");
        assert!(out.contains("literal"), "{out}");
        assert!(out.contains("c.rs"), "리터럴 매치 발견: {out}");
    }

    #[test]
    fn invalid_regex_with_zero_matches_still_reports_the_fallback() {
        let (_d, ctx) = setup();
        let out = Grep.run(&serde_json::json!({"pattern": "{no_such_thing}"}), &ctx).unwrap();
        assert!(out.contains("invalid regex"), "{out}");
        assert!(out.contains("0 matches"), "0매치도 원인 병기 (스펙 §5.3): {out}");
    }

    #[test]
    fn invalid_regex_is_no_longer_an_error_but_a_literal_fallback() {
        let (_d, ctx) = setup();
        let out = Grep.run(&serde_json::json!({"pattern": "["}), &ctx).unwrap();
        assert!(out.starts_with("invalid regex"), "{out}");
        assert!(out.contains("literal"), "{out}");
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

    #[test]
    fn adjacent_matches_each_get_their_context_window() {
        let dir = tempfile::tempdir().unwrap();
        let content: String = (1..=8).map(|i| format!("line{i}\n")).collect();
        std::fs::write(dir.path().join("f.txt"), content.replace("line3", "hit3").replace("line4", "hit4")).unwrap();
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        let out = Grep.run(&serde_json::json!({"pattern": "hit"}), &ctx).unwrap();
        assert!(out.contains("f.txt:3:") && out.contains("f.txt:4:"), "{out}");
        assert!(out.contains("--"), "매치 블록 구분자: {out}");
    }

    #[test]
    fn separator_between_non_adjacent_match_groups() {
        let dir = tempfile::tempdir().unwrap();
        // Two matches separated enough that their context blocks (2 lines each) don't overlap
        let lines = vec![
            "filler 0",
            "filler 1",
            "filler 2",
            "MATCH_A",      // index 3, context: indices 1-5 (lines 2-6 in 1-indexed)
            "filler 4",
            "filler 5",
            // Gap to ensure no overlap
            "filler 6", "filler 7", "filler 8", "filler 9",
            "filler 10", "filler 11", "filler 12", "filler 13",
            "filler 14",
            "filler 15",
            "MATCH_B",      // index 16, context: indices 14-18 (lines 15-19 in 1-indexed)
            "filler 17",
            "filler 18",
        ];
        std::fs::write(dir.path().join("multi.txt"), lines.join("\n")).unwrap();
        let ctx = ToolCtx::new(dir.path().to_path_buf());

        let out = run(&ctx, serde_json::json!({"pattern": "MATCH"})).unwrap();

        // Both matches in expected format
        assert!(out.contains("multi.txt:4: MATCH_A"), "first match: {out}");
        assert!(out.contains("multi.txt:17: MATCH_B"), "second match: {out}");

        // Separator line exists
        assert!(out.lines().any(|l| l == "--"), "separator line: {out}");

        // Separator is between the two groups
        let all_lines: Vec<&str> = out.lines().collect();
        let sep_idx = all_lines.iter().position(|l| *l == "--").expect("separator not found");
        assert!(
            all_lines[..sep_idx].iter().any(|l| l.contains("MATCH_A")),
            "MATCH_A before separator: {out}"
        );
        assert!(
            all_lines[sep_idx + 1..].iter().any(|l| l.contains("MATCH_B")),
            "MATCH_B after separator: {out}"
        );
    }
}
