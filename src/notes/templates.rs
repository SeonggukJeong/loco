//! Thrifty template bodies for schema/mut-gate reject injection only (spec §3-4).
//! Never paste these into SYSTEM or tool `doc()`.

/// Root notes template (full body, reject path only).
pub const ROOT_TEMPLATE: &str = "\
## summary
(1-3 lines: what this repo/binary is)

## routes
- src/cli.rs → CLI flags
- src/walk.rs → directory walk
(only high-traffic paths; one line each)

## do_not
- paste issue text, test names, diffs, or full file bodies
";

/// Directory-layer notes template (full body, reject path only).
pub const DIR_TEMPLATE: &str = "\
## role
(one line: what this directory owns)

## entrypoints
- SymbolOrFile — one-line why (max 5)

## notes
- optional convention bullets (max 3)
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::schema::{validate_dir, validate_root};

    #[test]
    fn templates_match_spec_headers() {
        assert!(ROOT_TEMPLATE.contains("## summary"));
        assert!(ROOT_TEMPLATE.contains("## routes"));
        assert!(ROOT_TEMPLATE.contains("## do_not"));
        assert!(ROOT_TEMPLATE.contains("src/cli.rs → CLI flags"));
        assert!(DIR_TEMPLATE.contains("## role"));
        assert!(DIR_TEMPLATE.contains("## entrypoints"));
        assert!(DIR_TEMPLATE.contains("## notes"));
        assert!(DIR_TEMPLATE.contains("SymbolOrFile"));
    }

    #[test]
    fn templates_are_schema_shaped_examples() {
        // Spec bodies are instructional but should still pass format schema
        // so a model that copies them wholesale is not immediately rejected
        // for structure (only for being placeholder-y — out of schema scope).
        assert!(
            validate_root(ROOT_TEMPLATE).is_ok(),
            "ROOT_TEMPLATE should satisfy root schema: {:?}",
            validate_root(ROOT_TEMPLATE)
        );
        assert!(
            validate_dir(DIR_TEMPLATE).is_ok(),
            "DIR_TEMPLATE should satisfy dir schema: {:?}",
            validate_dir(DIR_TEMPLATE)
        );
    }
}
