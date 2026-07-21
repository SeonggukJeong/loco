//! M16 hierarchical repo notes — path mapping, schema, thrifty templates, run state.

pub mod path;
pub mod schema;
pub mod state;
pub mod templates;

pub use path::{
    ancestor_keys, dirty_key, is_under_notes_dir, normalize_key, notes_fs_path, PathError, ROOT_KEY,
};
pub use schema::{
    validate, validate_dir, validate_root, SchemaError, DIR_MAX_BYTES, ROOT_MAX_BYTES,
};
pub use state::{
    notes_path_ban_body, notes_stale_nudge, NotesState, NOTES_BYTES_MAX_KIND, NOTES_MUT_GATE_MARK,
    NOTES_STALE_MARK,
};
pub use templates::{DIR_TEMPLATE, ROOT_TEMPLATE};
