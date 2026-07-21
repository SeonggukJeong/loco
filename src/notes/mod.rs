//! M16 hierarchical repo notes — pure path mapping, schema, thrifty templates.
//! Disk layout, mut-gate, and tools land in later tasks.

pub mod path;
pub mod schema;
pub mod templates;

pub use path::{
    ancestor_keys, dirty_key, is_under_notes_dir, normalize_key, notes_fs_path, PathError,
};
pub use schema::{
    validate, validate_dir, validate_root, SchemaError, DIR_MAX_BYTES, ROOT_MAX_BYTES,
};
pub use templates::{DIR_TEMPLATE, ROOT_TEMPLATE};
