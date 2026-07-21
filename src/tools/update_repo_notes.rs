//! `update_repo_notes` — write hierarchical repo notes under `.loco/notes/` (M16 §3-3).

use serde::Deserialize;

use crate::notes::path::ROOT_KEY;
use crate::notes::schema::SchemaError;
use crate::notes::{
    normalize_key, notes_fs_path, validate, DIR_TEMPLATE, ROOT_TEMPLATE,
};

use super::{Tool, ToolCtx, ToolError};

/// Success body prefix (exp_metrics / MARKS). Design §3-3.
pub const NOTES_UPDATE_OK_PREFIX: &str = "repo notes updated:";
/// Schema (or path) reject body prefix. Design §5-3.
pub const NOTES_SCHEMA_REJECT_PREFIX: &str = "repo notes schema:";

/// Injected by the agent after 2 consecutive schema rejects (once per run).
/// Model-facing English — thrifty in-place, not more files.
pub const NOTES_SCHEMA_CORRECTION: &str = "\
Your `update_repo_notes` failed schema again. Shorten the **same** key in place: \
root ≤1200 bytes, dir ≤800; `## role` one line; ≤3 entrypoint bullets; no file bodies, \
test logs, or issue text. Do **not** split topics into extra note files. \
Do **not** paste rejection templates into `content`.";

pub struct UpdateRepoNotes;

#[derive(Deserialize)]
struct Args {
    path: String,
    content: String,
}

fn parse(args: &serde_json::Value) -> Result<Args, ToolError> {
    serde_json::from_value(args.clone()).map_err(|e| ToolError::BadArgs(e.to_string()))
}

/// Path/normalize failures: short hint + root template (key shape unknown).
fn schema_reject_path(reason: &str) -> ToolError {
    ToolError::EditFailed(format!(
        "{NOTES_SCHEMA_REJECT_PREFIX} {reason}. \
Use notes key `_root` or `src` (not a `.loco/notes/...` filesystem path).\n\n{ROOT_TEMPLATE}"
    ))
}

/// Schema validation failures.
/// Size / soft-reject: **no** full thrifty template (models paste it and grow).
/// Structural gaps: attach root/dir template as before.
fn schema_reject_validate(key: &str, err: SchemaError) -> ToolError {
    let size_like = matches!(
        err,
        SchemaError::TooLarge { .. }
            | SchemaError::SoftRejectFence
            | SchemaError::SoftRejectTooManyLines { .. }
    );
    let mut body = format!("{NOTES_SCHEMA_REJECT_PREFIX} {err}");
    if let SchemaError::TooLarge { max, got } = err {
        let over = got.saturating_sub(max);
        body.push_str(&format!(
            ". Cut ~{over} bytes **in this same key** (role 1 line, ≤3 entrypoints). \
Do not split topics into more note files — shorten. Do not paste templates into content."
        ));
    }
    if !size_like {
        let template = if key == ROOT_KEY {
            ROOT_TEMPLATE
        } else {
            DIR_TEMPLATE
        };
        body.push_str("\n\n");
        body.push_str(template);
    }
    ToolError::EditFailed(body)
}

impl Tool for UpdateRepoNotes {
    fn name(&self) -> &'static str {
        "update_repo_notes"
    }

    fn doc(&self) -> &'static str {
        "update_repo_notes(path, content): `path` is notes key `_root` or `src` (not `.loco/notes/...`). \
Full replace; root≤1200B, dir≤800B; thrifty — no file bodies."
    }

    fn is_mutating(&self) -> bool {
        true
    }

    fn preview(&self, args: &serde_json::Value, _ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let key = normalize_key(&args.path).map_err(|e| schema_reject_path(&e.to_string()))?;
        // Dry-run schema so the approval gate sees the same reject the model would get
        validate(&key, &args.content).map_err(|e| schema_reject_validate(&key, e))?;
        let n = args.content.len();
        Ok(format!("update_repo_notes {key} ({n} bytes)"))
    }

    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError> {
        let args = parse(args)?;
        let key = normalize_key(&args.path).map_err(|e| schema_reject_path(&e.to_string()))?;
        validate(&key, &args.content).map_err(|e| schema_reject_validate(&key, e))?;
        let fs_path = notes_fs_path(&ctx.root, &key);
        if let Some(parent) = fs_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = args.content.as_bytes();
        std::fs::write(&fs_path, bytes)?;
        let rel = format!(".loco/notes/{key}.md");
        Ok(format!(
            "{NOTES_UPDATE_OK_PREFIX} {rel} ({} bytes)",
            bytes.len()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::ROOT_TEMPLATE;
    use crate::tools::{Tool, ToolCtx};

    fn ctx(dir: &tempfile::TempDir) -> ToolCtx {
        ToolCtx::new(dir.path().to_path_buf())
    }

    fn valid_root() -> String {
        ROOT_TEMPLATE.to_string()
    }

    fn valid_dir() -> String {
        crate::notes::DIR_TEMPLATE.to_string()
    }

    #[test]
    fn is_mutating() {
        assert!(UpdateRepoNotes.is_mutating());
    }

    #[test]
    fn doc_is_short() {
        let d = UpdateRepoNotes.doc();
        assert!(d.lines().count() <= 2, "doc must be ≤~2 lines: {d}");
        assert!(!d.contains("## summary"), "templates must not live in doc()");
        assert!(!d.contains(ROOT_TEMPLATE));
    }

    #[test]
    fn writes_root_notes_with_ok_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let out = UpdateRepoNotes
            .run(
                &serde_json::json!({"path": "_root", "content": valid_root()}),
                &ctx(&dir),
            )
            .unwrap();
        assert!(
            out.starts_with(NOTES_UPDATE_OK_PREFIX),
            "success prefix: {out}"
        );
        assert!(out.contains(".loco/notes/_root.md"), "{out}");
        let written = std::fs::read_to_string(dir.path().join(".loco/notes/_root.md")).unwrap();
        assert_eq!(written, valid_root());
    }

    #[test]
    fn normalizes_key_before_write() {
        let dir = tempfile::tempdir().unwrap();
        UpdateRepoNotes
            .run(
                &serde_json::json!({"path": "./src//walk.md", "content": valid_dir()}),
                &ctx(&dir),
            )
            .unwrap();
        assert!(dir.path().join(".loco/notes/src/walk.md").is_file());
    }

    #[test]
    fn storage_prefix_path_writes_under_single_notes_dir() {
        let dir = tempfile::tempdir().unwrap();
        let out = UpdateRepoNotes
            .run(
                &serde_json::json!({
                    "path": ".loco/notes/_root",
                    "content": valid_root()
                }),
                &ctx(&dir),
            )
            .unwrap();
        assert!(out.contains(".loco/notes/_root.md"), "{out}");
        assert!(
            !out.contains(".loco/notes/.loco/notes/"),
            "must not dual-prefix: {out}"
        );
        assert!(dir.path().join(".loco/notes/_root.md").is_file());
        assert!(!dir
            .path()
            .join(".loco/notes/.loco/notes/_root.md")
            .exists());
    }

    #[test]
    fn schema_fail_prefixes_and_includes_template() {
        let dir = tempfile::tempdir().unwrap();
        let err = UpdateRepoNotes
            .run(
                &serde_json::json!({"path": "_root", "content": "not a valid notes body"}),
                &ctx(&dir),
            )
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(NOTES_SCHEMA_REJECT_PREFIX),
            "schema prefix: {msg}"
        );
        assert!(msg.contains("## summary"), "root template on reject: {msg}");
        assert!(!dir.path().join(".loco/notes/_root.md").exists());
    }

    #[test]
    fn dir_schema_fail_uses_dir_template() {
        let dir = tempfile::tempdir().unwrap();
        let err = UpdateRepoNotes
            .run(
                &serde_json::json!({"path": "src", "content": "nope"}),
                &ctx(&dir),
            )
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(NOTES_SCHEMA_REJECT_PREFIX), "{msg}");
        assert!(msg.contains("## role"), "dir template: {msg}");
    }

    #[test]
    fn too_large_reject_has_no_full_template() {
        let dir = tempfile::tempdir().unwrap();
        // Valid dir shape but over DIR_MAX_BYTES (800)
        let mut body = String::from("## role\ncli entry\n\n## entrypoints\n");
        while body.len() < 850 {
            body.push_str("- Symbol — why this is long enough to blow the cap\n");
        }
        let err = UpdateRepoNotes
            .run(
                &serde_json::json!({"path": "src", "content": body}),
                &ctx(&dir),
            )
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(NOTES_SCHEMA_REJECT_PREFIX), "{msg}");
        assert!(msg.contains("too large"), "{msg}");
        assert!(msg.contains("same key") || msg.contains("Cut"), "{msg}");
        assert!(
            !msg.contains("## role\n(one line:"),
            "must not attach DIR_TEMPLATE body: {msg}"
        );
        assert!(!msg.contains("Do not paste issue text"), "no template dump: {msg}");
    }

    #[test]
    fn invalid_key_is_schema_reject() {
        let dir = tempfile::tempdir().unwrap();
        let err = UpdateRepoNotes
            .run(
                &serde_json::json!({"path": "../evil", "content": valid_root()}),
                &ctx(&dir),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains(NOTES_SCHEMA_REJECT_PREFIX),
            "{err}"
        );
    }

    #[test]
    fn preview_matches_run_validation() {
        let dir = tempfile::tempdir().unwrap();
        assert!(UpdateRepoNotes
            .preview(
                &serde_json::json!({"path": "_root", "content": "bad"}),
                &ctx(&dir),
            )
            .is_err());
        let p = UpdateRepoNotes
            .preview(
                &serde_json::json!({"path": "_root", "content": valid_root()}),
                &ctx(&dir),
            )
            .unwrap();
        assert!(p.contains("_root"), "{p}");
    }
}
