//! Notes key normalization and code-path → ancestor/dirty mapping (spec §3-1).

use std::path::{Path, PathBuf};

/// Root notes key — only this spelling is the root key (`root` alone is not).
pub const ROOT_KEY: &str = "_root";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// `.` / `..` segment, NUL, absolute path, or empty after normalize.
    Invalid(String),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::Invalid(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for PathError {}

/// Normalize a notes key (tool `path` arg).
///
/// Allowed: collapse `//`, strip leading `./`, `\`→`/`, strip trailing `.md`.
/// Models often pass a storage-relative path (`.loco/notes/_root`); strip that
/// prefix so the key is `_root` / `src`, not `.loco/notes/_root` (which would
/// write under `.loco/notes/.loco/notes/` and never satisfy the mut-gate).
/// Rejected: `.`/`..` segments, NUL, absolute/escape, empty.
/// Only `_root` is the root key — bare `root` stays the ordinary key `"root"`.
pub fn normalize_key(raw: &str) -> Result<String, PathError> {
    if raw.contains('\0') {
        return Err(PathError::Invalid(format!("NUL in notes key: {raw:?}")));
    }
    let mut s = raw.replace('\\', "/");
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    while s.starts_with("./") {
        s = s[2..].to_string();
    }
    if s.starts_with('/') {
        return Err(PathError::Invalid(format!(
            "absolute notes key not allowed: {raw}"
        )));
    }
    // Windows drive / UNC-ish
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        return Err(PathError::Invalid(format!(
            "absolute notes key not allowed: {raw}"
        )));
    }
    // Strip storage prefix (once, after ./ collapse). Case-sensitive on purpose.
    for prefix in [".loco/notes/", "loco/notes/"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
            break;
        }
    }
    if let Some(stripped) = s.strip_suffix(".md") {
        s = stripped.to_string();
    }
    while s.ends_with('/') {
        s.pop();
    }
    if s.is_empty() {
        return Err(PathError::Invalid("empty notes key".into()));
    }
    for seg in s.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(PathError::Invalid(format!(
                "invalid segment in notes key: {raw}"
            )));
        }
    }
    // Bare "root" is NOT rewritten to "_root" (§3-1).
    Ok(s)
}

/// Lexical normalize of a project-relative **code** path (no `.md` strip).
fn normalize_code_path(raw: &str) -> Result<String, PathError> {
    if raw.contains('\0') {
        return Err(PathError::Invalid(format!("NUL in path: {raw:?}")));
    }
    let mut s = raw.replace('\\', "/");
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    while s.starts_with("./") {
        s = s[2..].to_string();
    }
    if s.starts_with('/') {
        return Err(PathError::Invalid(format!(
            "absolute path not allowed: {raw}"
        )));
    }
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        return Err(PathError::Invalid(format!(
            "absolute path not allowed: {raw}"
        )));
    }
    while s.ends_with('/') {
        s.pop();
    }
    if s.is_empty() {
        return Err(PathError::Invalid("empty path".into()));
    }
    for seg in s.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(PathError::Invalid(format!("invalid segment in path: {raw}")));
        }
    }
    Ok(s)
}

/// Gate ancestor notes keys for a code path, most-specific → parent (excl. root).
///
/// Root-level files yield `[]` (root-only mut-gate special case, §3-5).
pub fn ancestor_keys(code_path: &str) -> Result<Vec<String>, PathError> {
    let p = normalize_code_path(code_path)?;
    let Some(parent) = parent_dir(&p) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    let mut cur = parent.to_string();
    loop {
        out.push(cur.clone());
        match parent_dir(&cur) {
            Some(up) => cur = up.to_string(),
            None => break,
        }
    }
    Ok(out)
}

/// Dirty key after a successful code mutation: most-specific dir ancestor, or `_root`.
pub fn dirty_key(code_path: &str) -> Result<String, PathError> {
    let p = normalize_code_path(code_path)?;
    Ok(match parent_dir(&p) {
        None => ROOT_KEY.to_string(),
        Some(d) => d.to_string(),
    })
}

fn parent_dir(path: &str) -> Option<&str> {
    path.rsplit_once('/')
        .map(|(parent, _)| parent)
        .filter(|p| !p.is_empty())
}

/// Filesystem path for a **normalized** notes key: `{root}/.loco/notes/{key}.md`.
pub fn notes_fs_path(project_root: &Path, key: &str) -> PathBuf {
    let mut p = project_root.join(".loco").join("notes");
    // Nested keys (`src/walk`) become nested files under notes/.
    p.push(format!("{key}.md"));
    p
}

/// True when `path` is under `{project_root}/.loco/notes` (lexical).
///
/// Used to block `edit_file`/`write_file` on notes (option A). Accepts absolute
/// paths under the project root or project-relative paths.
pub fn is_under_notes_dir(project_root: &Path, path: &Path) -> bool {
    let notes = project_root.join(".loco").join("notes");
    if path.is_absolute() {
        return path.starts_with(&notes);
    }
    // Relative: normalize separators and check prefix `.loco/notes`.
    let s = path.to_string_lossy().replace('\\', "/");
    let s = s.trim_start_matches("./");
    s == ".loco/notes" || s.starts_with(".loco/notes/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn mapping_vectors_from_spec() {
        // §3-1 PR1 vectors
        let cases: &[(&str, &[&str], &str)] = &[
            ("Cargo.toml", &[], "_root"),
            ("build.rs", &[], "_root"),
            ("src/main.rs", &["src"], "src"),
            ("src/exec/job.rs", &["src/exec", "src"], "src/exec"),
            ("crates/core/app.rs", &["crates/core", "crates"], "crates/core"),
        ];
        for &(code, want_anc, want_dirty) in cases {
            let anc = ancestor_keys(code).unwrap_or_else(|e| panic!("{code}: {e}"));
            assert_eq!(
                anc,
                want_anc.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "ancestors for {code}"
            );
            assert_eq!(dirty_key(code).unwrap(), want_dirty, "dirty for {code}");
        }
    }

    #[test]
    fn normalize_collapses_slashes_and_dot_slash() {
        assert_eq!(normalize_key("src//walk").unwrap(), "src/walk");
        assert_eq!(normalize_key("./src/walk").unwrap(), "src/walk");
        assert_eq!(normalize_key("src\\walk").unwrap(), "src/walk");
        assert_eq!(normalize_key("src/walk.md").unwrap(), "src/walk");
        assert_eq!(normalize_key("_root.md").unwrap(), "_root");
    }

    #[test]
    fn normalize_strips_loco_notes_storage_prefix() {
        // Smoke regression: model passed path=".loco/notes/_root" → dual-dir write + dead cert
        assert_eq!(normalize_key(".loco/notes/_root").unwrap(), "_root");
        assert_eq!(normalize_key(".loco/notes/src").unwrap(), "src");
        assert_eq!(normalize_key(".loco/notes/src/walk.md").unwrap(), "src/walk");
        assert_eq!(normalize_key("./.loco/notes/_root.md").unwrap(), "_root");
        assert_eq!(normalize_key("loco/notes/src").unwrap(), "src");
        // bare key still works
        assert_eq!(normalize_key("_root").unwrap(), "_root");
    }

    #[test]
    fn root_alone_is_not_root_key() {
        assert_eq!(normalize_key("root").unwrap(), "root");
        assert_ne!(normalize_key("root").unwrap(), ROOT_KEY);
        assert_eq!(normalize_key("_root").unwrap(), "_root");
    }

    #[test]
    fn reject_dot_dot_nul_absolute() {
        assert!(normalize_key("src/../evil").is_err());
        assert!(normalize_key("../evil").is_err());
        assert!(normalize_key("src/./x").is_err()); // bare `.` segment after normalize
        assert!(normalize_key("a\0b").is_err());
        assert!(normalize_key("/abs").is_err());
        assert!(normalize_key("").is_err());
        assert!(normalize_key(".md").is_err());
        assert!(ancestor_keys("../x").is_err());
        assert!(dirty_key("src/foo/../bar.rs").is_err());
    }

    #[test]
    fn notes_fs_path_layout() {
        let root = PathBuf::from("/proj");
        assert_eq!(
            notes_fs_path(&root, "_root"),
            PathBuf::from("/proj/.loco/notes/_root.md")
        );
        assert_eq!(
            notes_fs_path(&root, "src"),
            PathBuf::from("/proj/.loco/notes/src.md")
        );
        assert_eq!(
            notes_fs_path(&root, "src/walk"),
            PathBuf::from("/proj/.loco/notes/src/walk.md")
        );
    }

    #[test]
    fn is_under_notes_dir_relative_and_absolute() {
        let root = PathBuf::from("/proj");
        assert!(is_under_notes_dir(&root, Path::new(".loco/notes/_root.md")));
        assert!(is_under_notes_dir(&root, Path::new(".loco/notes/src.md")));
        assert!(is_under_notes_dir(
            &root,
            Path::new("/proj/.loco/notes/src.md")
        ));
        assert!(!is_under_notes_dir(&root, Path::new("src/main.rs")));
        assert!(!is_under_notes_dir(&root, Path::new(".loco/config.toml")));
        assert!(!is_under_notes_dir(&root, Path::new("/proj/src/main.rs")));
    }

    #[test]
    fn code_path_normalize_accepts_backslash() {
        assert_eq!(dirty_key("src\\main.rs").unwrap(), "src");
        assert_eq!(
            ancestor_keys("src\\exec\\job.rs").unwrap(),
            vec!["src/exec".to_string(), "src".to_string()]
        );
    }
}
