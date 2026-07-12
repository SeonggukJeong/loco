use serde::Deserialize;

use super::path::confine;
use super::{Tool, ToolCtx, ToolError};

/// 한 번에 읽는 최대 줄 수 (스펙 §4). limit 인자로도 이 값을 넘을 수 없다
pub const MAX_LINES: usize = 200;

pub struct ReadFile;

#[derive(Deserialize)]
struct Args {
    path: String,
    /// 1-기준 시작 줄
    offset: Option<usize>,
    limit: Option<usize>,
}

impl Tool for ReadFile {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn doc(&self) -> &'static str {
        "read_file(path, offset?, limit?): Read a UTF-8 text file. Returns up to 200 lines starting at line `offset` (1-based). If the file is longer, the output ends with how to continue."
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::BadArgs(e.to_string()))?;
        let path = confine(&ctx.root, &args.path)?;
        if path.is_dir() {
            return Err(ToolError::BadArgs(format!(
                "{} is a directory, not a file - use list_files for directories",
                args.path
            )));
        }
        let bytes = std::fs::read(&path)?;
        let text =
            String::from_utf8(bytes).map_err(|_| ToolError::NotUtf8(args.path.clone()))?;
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len();
        if total == 0 {
            return Ok("(empty file)".to_string());
        }
        let offset = args.offset.unwrap_or(1).max(1);
        let limit = args.limit.unwrap_or(MAX_LINES).clamp(1, MAX_LINES);
        if offset > total {
            return Err(ToolError::BadArgs(format!(
                "offset {offset} is past the end of the file ({total} lines)"
            )));
        }
        let start = offset - 1;
        let end = (start + limit).min(total);
        let mut out = lines[start..end].join("\n");
        if end < total {
            out.push_str(&format!(
                "\n[showing lines {offset}-{end} of {total}; call read_file again with offset={} to continue]",
                end + 1
            ));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::tools::{Tool, ToolCtx, ToolError};

    use super::ReadFile;

    fn setup(content: &str) -> (tempfile::TempDir, ToolCtx) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), content).unwrap();
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        (dir, ctx)
    }

    fn run(ctx: &ToolCtx, args: serde_json::Value) -> Result<String, ToolError> {
        ReadFile.run(&args, ctx)
    }

    #[test]
    fn reads_content_without_line_numbers() {
        let (_d, ctx) = setup("fn main() {}\nline two");
        let out = run(&ctx, serde_json::json!({"path": "f.txt"})).unwrap();
        assert_eq!(out, "fn main() {}\nline two"); // 라인 번호 없음 (스펙 §4)
    }

    #[test]
    fn caps_at_200_lines_and_tells_how_to_continue() {
        let content: String = (1..=250).map(|i| format!("line{i}\n")).collect();
        let (_d, ctx) = setup(&content);
        let out = run(&ctx, serde_json::json!({"path": "f.txt"})).unwrap();
        assert!(out.contains("line200"));
        assert!(!out.contains("line201\n"));
        assert!(out.contains("offset=201"), "이어 읽기 안내: {out}");
    }

    #[test]
    fn offset_continues_reading() {
        let content: String = (1..=250).map(|i| format!("line{i}\n")).collect();
        let (_d, ctx) = setup(&content);
        let out = run(&ctx, serde_json::json!({"path": "f.txt", "offset": 201})).unwrap();
        assert!(out.starts_with("line201"));
        assert!(out.contains("line250"));
        assert!(!out.contains("[showing"), "끝까지 읽으면 안내 없음");
    }

    #[test]
    fn crlf_file_reads_fine() {
        let (_d, ctx) = setup("a\r\nb\r\n");
        let out = run(&ctx, serde_json::json!({"path": "f.txt"})).unwrap();
        assert_eq!(out, "a\nb");
    }

    #[test]
    fn non_utf8_is_a_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bin.dat"), [0xFF, 0xFE, 0x00, 0x01]).unwrap();
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        let err = run(&ctx, serde_json::json!({"path": "bin.dat"})).unwrap_err();
        assert!(matches!(err, ToolError::NotUtf8(_)));
    }

    #[test]
    fn empty_file_says_so() {
        let (_d, ctx) = setup("");
        assert_eq!(run(&ctx, serde_json::json!({"path": "f.txt"})).unwrap(), "(empty file)");
    }

    #[test]
    fn limit_is_clamped_to_max() {
        let content: String = (1..=250).map(|i| format!("line{i}\n")).collect();
        let (_d, ctx) = setup(&content);
        let out = run(&ctx, serde_json::json!({"path": "f.txt", "limit": 9999})).unwrap();
        assert!(out.contains("line200") && !out.contains("line201\n"), "limit도 200 상한");
    }

    #[test]
    fn offset_and_limit_combine() {
        let content: String = (1..=50).map(|i| format!("line{i}\n")).collect();
        let (_d, ctx) = setup(&content);
        let out = run(&ctx, serde_json::json!({"path": "f.txt", "offset": 10, "limit": 3})).unwrap();
        assert!(out.starts_with("line10"));
        assert!(out.contains("line12") && !out.contains("line13"));
        assert!(out.contains("offset=13"), "이어 읽기 안내: {out}");
    }

    #[test]
    fn directory_path_gets_a_helpful_error_not_os_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        let err = run(&ctx, serde_json::json!({"path": "src"})).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("is a directory"), "{msg}");
        assert!(msg.contains("list_files"), "대안 안내: {msg}");
        assert!(!msg.contains("os error"), "날 OS 에러 노출 금지: {msg}");
    }

    #[test]
    fn missing_file_and_escape_and_bad_args() {
        let (_d, ctx) = setup("x");
        assert!(matches!(
            run(&ctx, serde_json::json!({"path": "nope.txt"})).unwrap_err(),
            ToolError::NotFound(_)
        ));
        assert!(matches!(
            run(&ctx, serde_json::json!({"path": "../f.txt"})).unwrap_err(),
            ToolError::PathViolation(_)
        ));
        assert!(matches!(
            run(&ctx, serde_json::json!({})).unwrap_err(),
            ToolError::BadArgs(_)
        ));
    }
}
