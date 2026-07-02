use std::path::{Component, Path, PathBuf};

use super::ToolError;

/// 모델이 준 경로를 프로젝트 루트 안으로 확인(confine)한다 (스펙 §4).
///
/// - `\` 구분자도 수용 (`/`로 정규화 — 스펙: 받을 때는 둘 다 허용)
/// - 절대 경로, Windows 드라이브 문자(`C:` 등), UNC(`\\server`) 거부
/// - 렉시컬 정규화에서 `..`가 루트를 벗어나면 거부
/// - canonicalize 후 루트 prefix 재검사 — 루트 밖을 가리키는 심볼릭 링크 거부
/// - 반환은 canonicalize된 실제 경로. 존재하지 않으면 NotFound
///   (M2 툴은 전부 읽기라 대상이 존재해야 함; M3 write_file은 부모 canonicalize로 확장 예정)
pub fn confine(root: &Path, raw: &str) -> Result<PathBuf, ToolError> {
    let normalized = raw.replace('\\', "/");
    if normalized.starts_with('/') || has_drive_prefix(&normalized) {
        return Err(ToolError::PathViolation(format!(
            "absolute paths are not allowed: {raw}"
        )));
    }
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for comp in Path::new(&normalized).components() {
        match comp {
            Component::Normal(c) => parts.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(ToolError::PathViolation(format!(
                        "path escapes the project root: {raw}"
                    )));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ToolError::PathViolation(format!(
                    "absolute paths are not allowed: {raw}"
                )));
            }
        }
    }
    let mut joined = root.to_path_buf();
    for p in &parts {
        joined.push(p);
    }
    let canon_root = root.canonicalize()?;
    let canon = joined
        .canonicalize()
        .map_err(|_| ToolError::NotFound(raw.to_string()))?;
    if !canon.starts_with(&canon_root) {
        return Err(ToolError::PathViolation(format!(
            "path resolves outside the project root (symlink?): {raw}"
        )));
    }
    Ok(canon)
}

/// "C:/..." 또는 "c:x" 같은 드라이브 문자 접두를 감지 (Unix에서도 거부해야 함)
fn has_drive_prefix(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/sub")).unwrap();
        std::fs::write(dir.path().join("src/sub/a.txt"), "x").unwrap();
        dir
    }

    #[test]
    fn accepts_relative_path() {
        let dir = root();
        let p = confine(dir.path(), "src/sub/a.txt").unwrap();
        assert!(p.ends_with("src/sub/a.txt"));
    }

    #[test]
    fn accepts_backslash_separators() {
        let dir = root();
        assert!(confine(dir.path(), "src\\sub\\a.txt").is_ok());
    }

    #[test]
    fn accepts_parent_dir_that_stays_inside() {
        let dir = root();
        // src/sub/../sub/a.txt → src/sub/a.txt (루트 안)
        assert!(confine(dir.path(), "src/sub/../sub/a.txt").is_ok());
    }

    #[test]
    fn rejects_escape_via_parent_dir() {
        let dir = root();
        for p in ["../x", "src/../../x", "..\\x"] {
            let err = confine(dir.path(), p).unwrap_err();
            assert!(matches!(err, ToolError::PathViolation(_)), "{p}");
        }
    }

    #[test]
    fn rejects_absolute_drive_and_unc_paths() {
        let dir = root();
        for p in ["/etc/passwd", "C:/x", "C:\\x", "c:x", "\\\\server\\share", "//server/share"] {
            let err = confine(dir.path(), p).unwrap_err();
            assert!(matches!(err, ToolError::PathViolation(_)), "{p}");
        }
    }

    #[test]
    fn missing_file_is_not_found() {
        let dir = root();
        let err = confine(dir.path(), "no/such.txt").unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_pointing_outside_root() {
        let dir = root();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "s").unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), dir.path().join("link.txt"))
            .unwrap();
        let err = confine(dir.path(), "link.txt").unwrap_err();
        assert!(matches!(err, ToolError::PathViolation(_)));
    }
}
