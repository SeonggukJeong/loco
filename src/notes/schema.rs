//! Format-only notes schema validation (spec §3-2). No semantic checks.

use super::path::ROOT_KEY;

/// Hard size cap for `_root.md` (bytes, not chars).
pub const ROOT_MAX_BYTES: usize = 1200;
/// Hard size cap for directory-layer notes.
pub const DIR_MAX_BYTES: usize = 800;
/// Soft-reject: non-blank line count at or above this → reject.
pub const SOFT_REJECT_NONBLANK_LINES: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    TooLarge { max: usize, got: usize },
    SoftRejectFence,
    SoftRejectTooManyLines { nonblank: usize },
    MissingSection(&'static str),
    /// Summary non-blank lines outside 1..=3.
    SummaryLineCount(usize),
    /// `## routes` has no `- path → role` bullets.
    EmptyRoutes,
    /// Dir notes: neither entrypoints nor notes has a bullet.
    BodyMissing,
    /// `## role` has no non-blank content.
    EmptyRole,
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::TooLarge { max, got } => {
                write!(f, "notes too large: {got} bytes (max {max})")
            }
            SchemaError::SoftRejectFence => {
                write!(f, "notes soft-reject: fenced code block present")
            }
            SchemaError::SoftRejectTooManyLines { nonblank } => {
                write!(
                    f,
                    "notes soft-reject: {nonblank} non-blank lines (max {})",
                    SOFT_REJECT_NONBLANK_LINES - 1
                )
            }
            SchemaError::MissingSection(name) => {
                write!(f, "missing ## {name} section")
            }
            SchemaError::SummaryLineCount(n) => {
                write!(f, "summary must be 1-3 non-blank lines (got {n})")
            }
            SchemaError::EmptyRoutes => {
                write!(f, "routes need at least one `- path → role` bullet")
            }
            SchemaError::BodyMissing => {
                write!(
                    f,
                    "need at least one bullet under ## entrypoints or ## notes"
                )
            }
            SchemaError::EmptyRole => write!(f, "role needs at least one non-blank line"),
        }
    }
}

impl std::error::Error for SchemaError {}

/// Validate notes body for key (`_root` → root schema, else dir schema).
pub fn validate(key: &str, text: &str) -> Result<(), SchemaError> {
    if key == ROOT_KEY {
        validate_root(text)
    } else {
        validate_dir(text)
    }
}

pub fn validate_root(text: &str) -> Result<(), SchemaError> {
    check_size_and_soft(text, ROOT_MAX_BYTES)?;
    let secs = parse_sections(text);
    let summary = secs
        .get("summary")
        .ok_or(SchemaError::MissingSection("summary"))?;
    let n = nonblank_count(summary);
    if n == 0 || n > 3 {
        return Err(SchemaError::SummaryLineCount(n));
    }
    let routes = secs
        .get("routes")
        .ok_or(SchemaError::MissingSection("routes"))?;
    if !routes.iter().any(|l| is_route_bullet(l)) {
        return Err(SchemaError::EmptyRoutes);
    }
    Ok(())
}

pub fn validate_dir(text: &str) -> Result<(), SchemaError> {
    check_size_and_soft(text, DIR_MAX_BYTES)?;
    let secs = parse_sections(text);
    let role = secs
        .get("role")
        .ok_or(SchemaError::MissingSection("role"))?;
    if nonblank_count(role) == 0 {
        return Err(SchemaError::EmptyRole);
    }
    let ep_ok = secs
        .get("entrypoints")
        .is_some_and(|lines| lines.iter().any(|l| is_bullet(l)));
    let notes_ok = secs
        .get("notes")
        .is_some_and(|lines| lines.iter().any(|l| is_bullet(l)));
    if !ep_ok && !notes_ok {
        return Err(SchemaError::BodyMissing);
    }
    Ok(())
}

fn check_size_and_soft(text: &str, max: usize) -> Result<(), SchemaError> {
    let got = text.len();
    if got > max {
        return Err(SchemaError::TooLarge { max, got });
    }
    if text.lines().any(is_fence_line) {
        return Err(SchemaError::SoftRejectFence);
    }
    let nonblank = text.lines().filter(|l| !l.trim().is_empty()).count();
    if nonblank >= SOFT_REJECT_NONBLANK_LINES {
        return Err(SchemaError::SoftRejectTooManyLines { nonblank });
    }
    Ok(())
}

fn is_fence_line(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

fn nonblank_count(lines: &[String]) -> usize {
    lines.iter().filter(|l| !l.trim().is_empty()).count()
}

fn is_bullet(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("- ") && t.len() > 2
}

/// `- <path> → <role>` with non-empty sides (§3-2).
fn is_route_bullet(line: &str) -> bool {
    let t = line.trim();
    let Some(rest) = t.strip_prefix("- ") else {
        return false;
    };
    let Some((left, right)) = rest.split_once('→') else {
        return false;
    };
    !left.trim().is_empty() && !right.trim().is_empty()
}

/// Map section name → body lines (in order). Later duplicate headers overwrite.
fn parse_sections(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut map = std::collections::BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("##") {
            // Require a space or end after ## so `###` is not a section (treat as body).
            let name = rest.trim();
            if !name.is_empty() && !name.starts_with('#') {
                current = Some(name.to_string());
                map.insert(name.to_string(), Vec::new());
                continue;
            }
        }
        if let Some(ref name) = current {
            map.get_mut(name).expect("section inserted").push(line.to_string());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_root() -> String {
        "## summary\n\
         a small CLI\n\
         \n\
         ## routes\n\
         - src/main.rs → entrypoint\n"
            .to_string()
    }

    fn valid_dir() -> String {
        "## role\n\
         owns the walk\n\
         \n\
         ## entrypoints\n\
         - walk_dir — recursive walk\n"
            .to_string()
    }

    #[test]
    fn root_valid_minimal() {
        assert!(validate_root(&valid_root()).is_ok());
        assert!(validate("_root", &valid_root()).is_ok());
    }

    #[test]
    fn root_summary_bounds() {
        let zero = "## summary\n\n## routes\n- a → b\n";
        assert!(matches!(
            validate_root(zero),
            Err(SchemaError::SummaryLineCount(0))
        ));

        let four = "## summary\n\
                    one\n\
                    two\n\
                    three\n\
                    four\n\
                    ## routes\n\
                    - a → b\n";
        assert!(matches!(
            validate_root(four),
            Err(SchemaError::SummaryLineCount(4))
        ));

        let three = "## summary\n\
                     one\n\
                     two\n\
                     three\n\
                     ## routes\n\
                     - a → b\n";
        assert!(validate_root(three).is_ok());
    }

    #[test]
    fn root_empty_routes_rejected() {
        let t = "## summary\nhi\n## routes\n\n";
        assert!(matches!(validate_root(t), Err(SchemaError::EmptyRoutes)));
        let ascii_arrow = "## summary\nhi\n## routes\n- a -> b\n";
        assert!(
            matches!(validate_root(ascii_arrow), Err(SchemaError::EmptyRoutes)),
            "ASCII -> is not the routes arrow"
        );
    }

    #[test]
    fn root_size_and_soft_reject() {
        let big = format!("## summary\nhi\n## routes\n- a → b\n{}", "x".repeat(1200));
        assert!(matches!(
            validate_root(&big),
            Err(SchemaError::TooLarge { max: ROOT_MAX_BYTES, .. })
        ));

        let fence = "## summary\nhi\n## routes\n- a → b\n```\ncode\n```\n";
        assert!(matches!(
            validate_root(fence),
            Err(SchemaError::SoftRejectFence)
        ));

        let mut many = String::from("## summary\nhi\n## routes\n- a → b\n");
        for i in 0..40 {
            many.push_str(&format!("extra line {i}\n"));
        }
        // summary(1) + routes bullet(1) + 40 extras = 42 non-blank, but even
        // fewer still hits ≥40 with enough lines under an extra section.
        assert!(matches!(
            validate_root(&many),
            Err(SchemaError::SoftRejectTooManyLines { .. })
        ));
    }

    #[test]
    fn dir_valid_entrypoints_or_notes() {
        assert!(validate_dir(&valid_dir()).is_ok());
        let notes_only = "## role\nx\n## notes\n- convention\n";
        assert!(validate_dir(notes_only).is_ok());
        assert!(validate("src", notes_only).is_ok());
    }

    #[test]
    fn dir_rejects_empty_body_and_role() {
        let no_body = "## role\nx\n## entrypoints\n\n";
        assert!(matches!(
            validate_dir(no_body),
            Err(SchemaError::BodyMissing)
        ));
        let no_role = "## role\n\n## entrypoints\n- a — b\n";
        assert!(matches!(validate_dir(no_role), Err(SchemaError::EmptyRole)));
    }

    #[test]
    fn dir_extra_do_not_allowed() {
        let t = "## role\nx\n\
                 ## entrypoints\n\
                 - a — b\n\
                 ## do_not\n\
                 - no dumps\n";
        assert!(validate_dir(t).is_ok());
    }

    #[test]
    fn dir_size_cap() {
        let big = format!(
            "## role\nx\n## entrypoints\n- a — b\n{}",
            "y".repeat(800)
        );
        assert!(matches!(
            validate_dir(&big),
            Err(SchemaError::TooLarge {
                max: DIR_MAX_BYTES,
                ..
            })
        ));
    }

    #[test]
    fn caps_are_fixed() {
        assert_eq!(ROOT_MAX_BYTES, 1200);
        assert_eq!(DIR_MAX_BYTES, 800);
    }
}
