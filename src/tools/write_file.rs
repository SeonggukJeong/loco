use serde::Deserialize;

use super::diff::{render_diff, render_diff_for_model};
use super::eol::{dominant_crlf, normalize_eol, restore_eol};
use super::path::confine_for_write;
use super::{Tool, ToolCtx, ToolError};

pub struct WriteFile;

#[derive(Deserialize)]
struct Args {
    path: String,
    content: String,
}

fn parse(args: &serde_json::Value) -> Result<Args, ToolError> {
    serde_json::from_value(args.clone()).map_err(|e| ToolError::BadArgs(e.to_string()))
}

/// 기존 파일이 UTF-8 텍스트면 Some(내용), 없거나 비UTF-8이면 None
fn existing_text(path: &std::path::Path) -> Option<String> {
    std::fs::read(path).ok().and_then(|b| String::from_utf8(b).ok())
}

impl Tool for WriteFile {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn doc(&self) -> &'static str {
        "write_file(path, content): Create a new file or overwrite an existing one with `content`. Prefer edit_file for small changes to existing files."
    }

    fn is_mutating(&self) -> bool {
        true
    }

    fn preview(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let path = confine_for_write(&ctx.root, &args.path)?;
        let new = normalize_eol(&args.content);
        Ok(match existing_text(&path) {
            Some(old) => format!(
                "write_file {} (덮어쓰기)\n{}",
                args.path,
                render_diff(&normalize_eol(&old), &new)
            ),
            None if path.exists() => {
                format!("write_file {} — 기존 비UTF-8 파일을 덮어씁니다", args.path)
            }
            None => format!("write_file {} (새 파일)\n{}", args.path, render_diff("", &new)),
        })
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let path = confine_for_write(&ctx.root, &args.path)?;
        let normalized = normalize_eol(&args.content);
        let old_text = existing_text(&path);
        // 덮어쓰기: 기존 지배적 EOL 유지. 새 파일: \n (스펙 §4)
        let crlf = old_text.as_deref().map(dominant_crlf).unwrap_or(false);
        let text = restore_eol(&normalized, crlf);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &text)?;
        // 신규 파일과 비UTF-8은 existing_text()가 둘 다 None을 준다(주석 참조).
        // 신규는 전 줄이 추가라 신호가 0이고 비UTF-8은 diff를 낼 원문이 없다 —
        // 둘 다 현행 요약 줄을 유지한다 (스펙 §3-5-2)
        Ok(match old_text {
            Some(old) => format!(
                "Wrote {} ({} lines)\n{}",
                args.path,
                normalized.lines().count(),
                render_diff_for_model(&normalize_eol(&old), &normalized)
            ),
            None => format!("Wrote {} ({} lines)", args.path, normalized.lines().count()),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::tools::{Tool, ToolCtx};
    use super::WriteFile;

    fn ctx(dir: &tempfile::TempDir) -> ToolCtx {
        ToolCtx::new(dir.path().to_path_buf())
    }

    #[test]
    fn creates_new_file_with_lf_and_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let out = WriteFile
            .run(&serde_json::json!({"path": "a/b/new.txt", "content": "one\r\ntwo"}), &ctx(&dir))
            .unwrap();
        assert!(out.contains("a/b/new.txt"));
        let written = std::fs::read_to_string(dir.path().join("a/b/new.txt")).unwrap();
        assert_eq!(written, "one\ntwo", "새 파일은 \\n (스펙 §4)");
    }

    #[test]
    fn overwrite_keeps_dominant_crlf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), "a\r\nb\r\n").unwrap();
        WriteFile
            .run(&serde_json::json!({"path": "f.txt", "content": "x\ny\n"}), &ctx(&dir))
            .unwrap();
        let written = std::fs::read(dir.path().join("f.txt")).unwrap();
        assert_eq!(String::from_utf8(written).unwrap(), "x\r\ny\r\n");
    }

    #[test]
    fn preview_is_a_diff_for_overwrite_and_lists_new_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), "old\n").unwrap();
        let p = WriteFile
            .preview(&serde_json::json!({"path": "f.txt", "content": "new\n"}), &ctx(&dir))
            .unwrap();
        assert!(p.contains("-old") && p.contains("+new"), "{p}");
        let p2 = WriteFile
            .preview(&serde_json::json!({"path": "fresh.txt", "content": "hello\n"}), &ctx(&dir))
            .unwrap();
        assert!(p2.contains("새 파일") && p2.contains("+hello"), "{p2}");
    }

    #[test]
    fn run_result_carries_a_diff_for_overwrite_but_not_for_a_new_file() {
        // Step 6 배선의 run() 쪽 회귀 커버리지 — preview()는 이미 diff 형태를
        // 검증하지만 run()의 반환 문자열은 이 테스트 이전엔 파일 바이트만
        // 간접 확인됐다 (M14 A-3)
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), "old\n").unwrap();
        let out = WriteFile
            .run(&serde_json::json!({"path": "f.txt", "content": "new\n"}), &ctx(&dir))
            .unwrap();
        assert!(out.contains("-old") && out.contains("+new"), "덮어쓰기는 diff를 실어야 한다: {out}");

        let out2 = WriteFile
            .run(&serde_json::json!({"path": "fresh.txt", "content": "hello\n"}), &ctx(&dir))
            .unwrap();
        assert!(!out2.contains("-0 lines"), "신규 파일은 diff 헤더 없이 요약 줄만 유지한다: {out2}");
    }

    #[test]
    fn is_mutating_and_rejects_escape() {
        let dir = tempfile::tempdir().unwrap();
        assert!(WriteFile.is_mutating());
        assert!(WriteFile
            .run(&serde_json::json!({"path": "../x", "content": ""}), &ctx(&dir))
            .is_err());
    }
}
