//! Run-scoped certified / dirty notes state (M16 §3-5 · §3-6).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::path::{ancestor_keys, dirty_key, ROOT_KEY};
use super::schema::validate;

/// Marker prefix for mut-gate rejections (exp_metrics `notes_mut_gate`).
pub const NOTES_MUT_GATE_MARK: &str = "repo notes mut gate:";
/// Marker prefix for NOTES_STALE finish reject (exp_metrics `notes_stale_finish`).
pub const NOTES_STALE_MARK: &str = "repo notes stale:";

/// Full NOTES_STALE body first line shape (keys filled at runtime).
///
/// Exact contract: `repo notes stale: you edited code but did not update notes for: {keys}. Call update_repo_notes on each listed key, then finish.`
pub fn notes_stale_nudge(dirty: &BTreeSet<String>) -> String {
    let keys = dirty.iter().cloned().collect::<Vec<_>>().join(", ");
    format!(
        "{NOTES_STALE_MARK} you edited code but did not update notes for: {keys}. \
         Call update_repo_notes on each listed key, then finish."
    )
}

/// Transcript extra kind for max certified-note file size (§5-3).
pub const NOTES_BYTES_MAX_KIND: &str = "notes_bytes_max";

/// Run-scoped notes bookkeeping. Built only when `repo_notes` is true.
#[derive(Debug, Default)]
pub struct NotesState {
    /// Keys whose schema passed at start-scan or a successful `update_repo_notes`.
    pub certified: BTreeSet<String>,
    /// Dir keys dirtied by successful code `edit_file`/`write_file`.
    pub dirty: BTreeSet<String>,
    /// Once-latch for NOTES_STALE finish rejection.
    pub notes_stale_nudged: bool,
    /// Per-key byte lengths of last certified write/scan content.
    key_bytes: BTreeMap<String, usize>,
}

impl NotesState {
    /// Scan `.loco/notes/**/*.md`, certify schema-OK keys, track bytes.
    pub fn scan(project_root: &Path) -> Self {
        let mut state = Self::default();
        let notes_dir = project_root.join(".loco").join("notes");
        if !notes_dir.is_dir() {
            return state;
        }
        let mut stack = vec![notes_dir.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                let Ok(rel) = path.strip_prefix(&notes_dir) else {
                    continue;
                };
                let Some(key) = rel_to_key(rel) else {
                    continue;
                };
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if validate(&key, &content).is_ok() {
                    state.certify(&key, content.len());
                }
            }
        }
        state
    }

    /// Mark a key certified and record its content length.
    pub fn certify(&mut self, key: &str, bytes: usize) {
        self.certified.insert(key.to_string());
        self.key_bytes.insert(key.to_string(), bytes);
    }

    /// Max over certified note file lengths (0 if none).
    pub fn bytes_max(&self) -> usize {
        self.key_bytes.values().copied().max().unwrap_or(0)
    }

    /// Mut-gate predicate (§3-5): `_root` certified AND (ancestor ∩ cert nonempty OR root-file).
    pub fn gate_ok(&self, code_path: &str) -> bool {
        if !self.certified.contains(ROOT_KEY) {
            return false;
        }
        match ancestor_keys(code_path) {
            Ok(anc) if anc.is_empty() => true, // root-level file special case
            Ok(anc) => anc.iter().any(|k| self.certified.contains(k)),
            Err(_) => false,
        }
    }

    /// Record a successful code mutation into the dirty set.
    pub fn mark_dirty_for_path(&mut self, code_path: &str) {
        if let Ok(k) = dirty_key(code_path) {
            self.dirty.insert(k);
        }
    }

    /// Clear dirty for an exact notes key (successful `update_repo_notes`).
    pub fn clear_dirty_key(&mut self, key: &str) {
        self.dirty.remove(key);
    }

    /// Mut-gate rejection body: mark + short prescription (no full templates —
    /// long templates drove length-cutoff loops when models pasted them into notes).
    pub fn mut_gate_body(&self, code_path: &str) -> String {
        let mut missing = Vec::new();
        if !self.certified.contains(ROOT_KEY) {
            missing.push(format!("`{ROOT_KEY}`"));
        }
        if let Ok(anc) = ancestor_keys(code_path)
            && !anc.is_empty()
            && !anc.iter().any(|k| self.certified.contains(k))
        {
            missing.push(format!(
                "an ancestor of `{code_path}` ({})",
                anc.join(" | ")
            ));
        }
        let need = if missing.is_empty() {
            "certified notes (path could not be mapped)".to_string()
        } else {
            missing.join(" and ")
        };
        format!(
            "{NOTES_MUT_GATE_MARK} code edit of `{code_path}` blocked — need certified {need}. \
             Next: `update_repo_notes` with key `_root` and/or dir key only (not `.loco/notes/...`). \
             Keep content tiny: root = short summary + ≤3 routes; dir = 1-line role + ≤3 entrypoints. \
             No file lists, no pasted templates."
        )
    }
}

fn rel_to_key(rel: &Path) -> Option<String> {
    let s = rel.to_string_lossy().replace('\\', "/");
    let s = s.strip_suffix(".md")?;
    if s.is_empty() || s.contains("..") {
        return None;
    }
    Some(s.to_string())
}

/// Error when edit_file/write_file targets `.loco/notes/**`.
pub fn notes_path_ban_body(tool: &str) -> String {
    format!(
        "Error: `{tool}` cannot write under `.loco/notes/`. Use `update_repo_notes` instead."
    )
}

/// Helper for tests / diagnostics.
#[allow(dead_code)]
pub fn notes_dir(project_root: &Path) -> PathBuf {
    project_root.join(".loco").join("notes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::{DIR_TEMPLATE, ROOT_TEMPLATE};

    #[test]
    fn empty_scan_has_zero_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let s = NotesState::scan(dir.path());
        assert!(s.certified.is_empty());
        assert_eq!(s.bytes_max(), 0);
    }

    #[test]
    fn scan_certifies_schema_ok_files() {
        let dir = tempfile::tempdir().unwrap();
        let notes = dir.path().join(".loco/notes");
        std::fs::create_dir_all(notes.join("src")).unwrap();
        std::fs::write(notes.join("_root.md"), ROOT_TEMPLATE).unwrap();
        std::fs::write(notes.join("src.md"), DIR_TEMPLATE).unwrap();
        std::fs::write(notes.join("src/walk.md"), DIR_TEMPLATE).unwrap();
        std::fs::write(notes.join("bad.md"), "not valid").unwrap();

        let s = NotesState::scan(dir.path());
        assert!(s.certified.contains("_root"));
        assert!(s.certified.contains("src"));
        assert!(s.certified.contains("src/walk"));
        assert!(!s.certified.contains("bad"));
        assert!(s.bytes_max() >= ROOT_TEMPLATE.len());
    }

    #[test]
    fn gate_ok_root_file_needs_only_root() {
        let mut s = NotesState::default();
        assert!(!s.gate_ok("Cargo.toml"));
        s.certify(ROOT_KEY, 10);
        assert!(s.gate_ok("Cargo.toml"));
        assert!(s.gate_ok("build.rs"));
        assert!(!s.gate_ok("src/x.rs"), "nested needs ancestor");
    }

    #[test]
    fn gate_ok_nested_needs_ancestor() {
        let mut s = NotesState::default();
        s.certify(ROOT_KEY, 10);
        assert!(!s.gate_ok("src/x.rs"));
        s.certify("src", 20);
        assert!(s.gate_ok("src/x.rs"));
        assert!(s.gate_ok("src/exec/job.rs"), "ancestor src counts");
    }

    #[test]
    fn stale_nudge_lists_sorted_keys() {
        let mut dirty = BTreeSet::new();
        dirty.insert("src".into());
        dirty.insert("_root".into());
        let body = notes_stale_nudge(&dirty);
        assert!(body.starts_with(NOTES_STALE_MARK), "{body}");
        assert!(
            body.starts_with(
                "repo notes stale: you edited code but did not update notes for: _root, src. "
            ),
            "{body}"
        );
        assert!(body.contains("Call update_repo_notes on each listed key, then finish."));
    }

    #[test]
    fn mut_gate_body_starts_with_mark_and_stays_short() {
        let s = NotesState::default();
        let body = s.mut_gate_body("src/main.rs");
        assert!(body.starts_with(NOTES_MUT_GATE_MARK), "{body}");
        assert!(body.contains("update_repo_notes"), "{body}");
        assert!(body.contains("_root"), "{body}");
        assert!(
            !body.contains("## summary"),
            "no full root template: {body}"
        );
        assert!(!body.contains("## role\n(one line"), "no full dir template: {body}");
        assert!(body.len() < 500, "mut-gate body must stay short: {}", body.len());
    }

    #[test]
    fn dirty_mark_and_exact_clear() {
        let mut s = NotesState::default();
        s.mark_dirty_for_path("src/main.rs");
        s.mark_dirty_for_path("Cargo.toml");
        assert!(s.dirty.contains("src"));
        assert!(s.dirty.contains("_root"));
        s.clear_dirty_key("src");
        assert!(!s.dirty.contains("src"));
        assert!(s.dirty.contains("_root"), "only exact key cleared");
    }
}
