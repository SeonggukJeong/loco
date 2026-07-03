use std::ffi::OsStr;
use std::path::Path;

use serde::Deserialize;

use super::path::confine;
use super::{Tool, ToolCtx, ToolError};

/// 한 번에 나열하는 최대 항목 수 (스펙 §4 "항목 수 상한")
pub const MAX_ENTRIES: usize = 200;

pub struct ListFiles;

#[derive(Deserialize)]
struct Args {
    path: Option<String>,
    depth: Option<usize>,
}

/// gitignore를 존중하는 공용 워커 (grep과 프롬프트 트리 주입이 재사용).
/// require_git(false): git repo가 아니어도 .gitignore를 적용 (테스트 픽스처 포함).
/// 정렬은 출력 결정성 때문에 필요하다.
pub(crate) fn walker(base: &Path, depth: Option<usize>) -> ignore::Walk {
    let mut b = ignore::WalkBuilder::new(base);
    b.require_git(false)
        .sort_by_file_name(|a: &OsStr, b: &OsStr| a.cmp(b));
    if let Some(d) = depth {
        b.max_depth(Some(d));
    }
    b.build()
}

/// base 아래 항목을 루트 기준 상대 경로로 나열한다. 디렉터리는 `/` 접미,
/// 구분자는 `/`로 정규화 (스펙 §4: 모델에게 보여줄 때는 `/`). 최대 max_entries개.
pub fn walk_entries(
    root: &Path,
    base: &Path,
    depth: Option<usize>,
    max_entries: usize,
) -> Vec<String> {
    // 표시 경로 계산과 시작점 비교가 일관되도록 양쪽 다 canonicalize
    // (macOS의 /tmp → /private/tmp 심링크 등으로 걷기 경로와 어긋나는 것 방지)
    let canon_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let mut out = Vec::new();
    for entry in walker(&base, depth) {
        let Ok(entry) = entry else { continue }; // 읽기 실패 항목은 건너뜀
        let p = entry.path();
        if p == base {
            continue; // 시작점 자신은 제외
        }
        let rel = p.strip_prefix(&canon_root).unwrap_or(p);
        let mut s = rel.to_string_lossy().replace('\\', "/");
        if entry.file_type().is_some_and(|t| t.is_dir()) {
            s.push('/');
        }
        out.push(s);
        if out.len() >= max_entries {
            break;
        }
    }
    out
}

impl Tool for ListFiles {
    fn name(&self) -> &'static str {
        "list_files"
    }

    fn doc(&self) -> &'static str {
        "list_files(path?, depth?): List files under `path` (default: project root), honoring .gitignore. Directories end with `/`."
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::BadArgs(e.to_string()))?;
        let base = confine(&ctx.root, args.path.as_deref().unwrap_or(""))?;
        let mut entries = walk_entries(&ctx.root, &base, args.depth, MAX_ENTRIES + 1);
        if entries.is_empty() {
            return Ok("(empty)".to_string());
        }
        let truncated = entries.len() > MAX_ENTRIES;
        entries.truncate(MAX_ENTRIES);
        let mut out = entries.join("\n");
        if truncated {
            out.push_str(&format!(
                "\n[truncated at {MAX_ENTRIES} entries; pass `path` or `depth` to narrow]"
            ));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{Tool, ToolCtx};

    fn setup() -> (tempfile::TempDir, ToolCtx) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/deep/deeper")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
        std::fs::write(dir.path().join("src/deep/a.rs"), "").unwrap();
        std::fs::write(dir.path().join("src/deep/deeper/b.rs"), "").unwrap();
        std::fs::write(dir.path().join("README.md"), "").unwrap();
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        (dir, ctx)
    }

    #[test]
    fn lists_files_and_dirs_with_slash_suffix() {
        let (_d, ctx) = setup();
        let out = ListFiles.run(&serde_json::json!({}), &ctx).unwrap();
        assert!(out.contains("README.md"));
        assert!(out.lines().any(|l| l == "src/"), "디렉터리는 / 접미: {out}");
        assert!(out.contains("src/deep/deeper/b.rs"));
    }

    #[test]
    fn respects_gitignore_without_git_repo() {
        let (dir, ctx) = setup();
        std::fs::create_dir_all(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/junk.o"), "").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "/target\n").unwrap();
        let out = ListFiles.run(&serde_json::json!({}), &ctx).unwrap();
        assert!(!out.contains("junk.o"), "{out}");
    }

    #[test]
    fn depth_limits_recursion() {
        let (_d, ctx) = setup();
        let out = ListFiles.run(&serde_json::json!({"depth": 1}), &ctx).unwrap();
        assert!(out.contains("src/"));
        assert!(!out.contains("src/main.rs"), "depth=1이면 루트 항목만: {out}");
    }

    #[test]
    fn path_narrows_the_listing() {
        let (_d, ctx) = setup();
        let out = ListFiles.run(&serde_json::json!({"path": "src/deep"}), &ctx).unwrap();
        assert!(out.contains("src/deep/a.rs"));
        assert!(!out.contains("README.md"));
    }

    #[test]
    fn caps_entries_with_notice() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..210 {
            std::fs::write(dir.path().join(format!("f{i:03}.txt")), "").unwrap();
        }
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        let out = ListFiles.run(&serde_json::json!({}), &ctx).unwrap();
        assert_eq!(out.lines().filter(|l| l.ends_with(".txt")).count(), MAX_ENTRIES);
        assert!(out.contains("[truncated at 200 entries"), "{out}");
    }

    #[test]
    fn escape_is_rejected() {
        let (_d, ctx) = setup();
        assert!(ListFiles.run(&serde_json::json!({"path": "../"}), &ctx).is_err());
    }
}
