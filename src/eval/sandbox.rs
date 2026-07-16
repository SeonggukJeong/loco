//! 과제 샌드박스 — fixture 복사, protected 동기화 (스펙 §8), 임시 디렉터리 관리.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};

/// 프로세스 내 샌드박스 일련번호 — pid와 조합해 고유 이름을 만든다
static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Debug)]
pub struct Sandbox {
    pub root: PathBuf,
}

impl Sandbox {
    /// fixture를 새 임시 디렉터리로 복사한다. tempfile 크레이트는 dev-dependency —
    /// 의존성 고정(스펙) 때문에 본체로 승격하지 않고 pid+카운터로 고유 이름을 만든다
    pub fn create(fixture: &Path) -> anyhow::Result<Sandbox> {
        let base = std::env::temp_dir();
        loop {
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let root = base.join(format!("loco-eval-{}-{n}", std::process::id()));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    if let Err(e) = copy_tree(fixture, &root) {
                        // 부분 복사 잔재를 남기지 않는다 — 에러 경로 샌드박스 누수 방지
                        let _ = std::fs::remove_dir_all(&root);
                        return Err(e);
                    }
                    return Ok(Sandbox { root });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    return Err(e).with_context(|| format!("샌드박스 생성 실패: {}", root.display()));
                }
            }
        }
    }

    /// protected 경로를 fixture 원본과 정확히 일치시킨다 (스펙 §8):
    /// 샌드박스 쪽을 통째로 지우고 fixture에서 새로 복사 —
    /// 수정 복원 + 에이전트가 추가한 파일 삭제를 한 번에 처리
    pub fn sync_protected(&self, fixture: &Path, protected: &[String]) -> anyhow::Result<()> {
        for rel in protected {
            let src = fixture.join(rel);
            let dst = self.root.join(rel);
            if dst.symlink_metadata().is_ok() {
                remove_any(&dst).with_context(|| format!("protected 정리 실패: {}", dst.display()))?;
            }
            if src.is_dir() {
                std::fs::create_dir_all(&dst)?;
                // read+write 오버레이 — fs::copy는 macOS에서 원본 mtime을 보존해(clonefile)
                // 변조된 protected로 빌드된 캐시가 재사용되는 스테일 판정 벡터가 된다 (M6 §4)
                overlay_tree(&src, &dst)?;
            } else if src.exists() {
                if let Some(parent) = dst.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let bytes = std::fs::read(&src)?;
                std::fs::write(&dst, bytes)?;
            }
        }
        Ok(())
    }

    /// 최선 노력 정리 — 실패해도 하네스를 죽이지 않는다
    pub fn cleanup(self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn copy_tree(src: &Path, dst: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&from)?;
        if meta.is_symlink() {
            bail!("fixture에 심링크가 있음 (지원 안 함): {}", from.display());
        }
        if meta.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// src 트리를 dst에 덮어쓴다 — fs::copy 대신 read+write. macOS의 fs::copy는
/// 원본 mtime을 보존해(clonefile) 기존 빌드 산출물보다 과거가 되고, cargo가
/// 재빌드를 건너뛰어 스테일 테스트 바이너리로 판정하는 벡터가 된다 (M6 §4)
pub(crate) fn overlay_tree(src: &Path, dst: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&from)?;
        if meta.is_symlink() {
            bail!("오버레이 원본에 심링크가 있음 (지원 안 함): {}", from.display());
        }
        if meta.is_dir() {
            std::fs::create_dir_all(&to)?;
            overlay_tree(&from, &to)?;
        } else {
            let bytes = std::fs::read(&from)?;
            std::fs::write(&to, bytes)
                .with_context(|| format!("오버레이 쓰기 실패: {}", to.display()))?;
        }
    }
    Ok(())
}

fn remove_any(p: &Path) -> std::io::Result<()> {
    // is_dir()은 심링크를 따라간다 — 모델이 protected 경로를 심링크로 바꿔치기해도
    // 하네스가 죽지 않게 symlink_metadata의 파일타입으로 분기한다 (심링크 자체는
    // remove_file 대상; remove_dir_all은 심링크 루트를 거부한다)
    let meta = p.symlink_metadata()?;
    if meta.file_type().is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (rel, content) in files {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        dir
    }

    #[test]
    fn create_copies_nested_tree() {
        let fx = fixture_with(&[("src/lib.rs", "code"), ("tests/t.rs", "test"), ("Cargo.toml", "manifest")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        assert_eq!(std::fs::read_to_string(sb.root.join("src/lib.rs")).unwrap(), "code");
        assert_eq!(std::fs::read_to_string(sb.root.join("tests/t.rs")).unwrap(), "test");
        sb.cleanup();
    }

    #[test]
    fn two_sandboxes_get_distinct_roots() {
        let fx = fixture_with(&[("a.txt", "x")]);
        let a = Sandbox::create(fx.path()).unwrap();
        let b = Sandbox::create(fx.path()).unwrap();
        assert_ne!(a.root, b.root);
        a.cleanup();
        b.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_in_fixture_is_an_error() {
        let fx = fixture_with(&[("real.txt", "x")]);
        std::os::unix::fs::symlink(fx.path().join("real.txt"), fx.path().join("link.txt")).unwrap();
        assert!(Sandbox::create(fx.path()).unwrap_err().to_string().contains("심링크"));
    }

    #[cfg(unix)]
    #[test]
    fn sync_replaces_symlinked_protected_path() {
        // 보상 해킹 변형: 모델이 run_command로 protected 디렉터리를 심링크로 바꿔치기
        let fx = fixture_with(&[("tests/t.rs", "ORIGINAL"), ("decoy.txt", "D")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::remove_dir_all(sb.root.join("tests")).unwrap();
        std::os::unix::fs::symlink(sb.root.join("decoy.txt"), sb.root.join("tests")).unwrap();
        sb.sync_protected(fx.path(), &["tests".to_string()]).unwrap();
        assert_eq!(std::fs::read_to_string(sb.root.join("tests/t.rs")).unwrap(), "ORIGINAL");
        sb.cleanup();
    }

    #[test]
    fn sync_restores_modified_and_deletes_added() {
        let fx = fixture_with(&[("tests/t.rs", "ORIGINAL"), ("src/lib.rs", "code")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        // 에이전트의 보상 해킹 시뮬레이션: protected 수정 + protected 아래 파일 추가
        std::fs::write(sb.root.join("tests/t.rs"), "HACKED").unwrap();
        std::fs::write(sb.root.join("tests/extra.rs"), "sneak").unwrap();
        // protected 밖 산출물은 보존돼야 한다
        std::fs::write(sb.root.join("answer.txt"), "42").unwrap();

        sb.sync_protected(fx.path(), &["tests".to_string()]).unwrap();

        assert_eq!(std::fs::read_to_string(sb.root.join("tests/t.rs")).unwrap(), "ORIGINAL", "수정 복원");
        assert!(!sb.root.join("tests/extra.rs").exists(), "추가 파일 삭제 (스펙 §8)");
        assert_eq!(std::fs::read_to_string(sb.root.join("answer.txt")).unwrap(), "42", "작업 산출물 보존");
        sb.cleanup();
    }

    #[test]
    fn sync_restores_single_protected_file() {
        let fx = fixture_with(&[("Cargo.toml", "ORIGINAL")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::write(sb.root.join("Cargo.toml"), "HACKED").unwrap();
        sb.sync_protected(fx.path(), &["Cargo.toml".to_string()]).unwrap();
        assert_eq!(std::fs::read_to_string(sb.root.join("Cargo.toml")).unwrap(), "ORIGINAL");
        sb.cleanup();
    }

    #[test]
    fn sync_removes_protected_dir_the_agent_deleted_and_recreated_wrong() {
        let fx = fixture_with(&[("tests/a.rs", "A"), ("tests/sub/b.rs", "B")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::remove_dir_all(sb.root.join("tests")).unwrap();
        std::fs::create_dir_all(sb.root.join("tests")).unwrap();
        std::fs::write(sb.root.join("tests/a.rs"), "TAMPERED").unwrap();
        sb.sync_protected(fx.path(), &["tests".to_string()]).unwrap();
        assert_eq!(std::fs::read_to_string(sb.root.join("tests/a.rs")).unwrap(), "A");
        assert_eq!(std::fs::read_to_string(sb.root.join("tests/sub/b.rs")).unwrap(), "B", "중첩 복원");
        sb.cleanup();
    }

    #[test]
    fn cleanup_removes_the_sandbox() {
        let fx = fixture_with(&[("a.txt", "x")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        let root = sb.root.clone();
        sb.cleanup();
        assert!(!root.exists());
    }

    fn age_file(p: &Path) -> std::time::SystemTime {
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        let f = std::fs::File::options().write(true).open(p).unwrap();
        f.set_modified(old).unwrap();
        old
    }

    #[test]
    fn overlay_tree_refreshes_mtime() {
        // fs::copy였다면 macOS에서 원본 mtime(1시간 전)이 보존돼 실패한다
        let src = fixture_with(&[("a.rs", "new")]);
        let dst = fixture_with(&[("a.rs", "stale")]);
        let old = age_file(&src.path().join("a.rs"));
        overlay_tree(src.path(), dst.path()).unwrap();
        let copied = std::fs::metadata(dst.path().join("a.rs")).unwrap().modified().unwrap();
        assert!(copied > old + std::time::Duration::from_secs(1800), "오버레이는 mtime을 갱신해야 함 — 스테일 빌드 캐시 방지 (M6 §4)");
        assert_eq!(std::fs::read_to_string(dst.path().join("a.rs")).unwrap(), "new");
    }

    #[test]
    fn sync_protected_refreshes_mtime() {
        // 에이전트가 protected를 변조·빌드해 둔 뒤 복원돼도 check가 재빌드하게
        let fx = fixture_with(&[("tests/t.rs", "ORIGINAL")]);
        let old = age_file(&fx.path().join("tests/t.rs"));
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::write(sb.root.join("tests/t.rs"), "HACKED").unwrap();
        sb.sync_protected(fx.path(), &["tests".to_string()]).unwrap();
        let restored = std::fs::metadata(sb.root.join("tests/t.rs")).unwrap().modified().unwrap();
        assert!(restored > old + std::time::Duration::from_secs(1800), "protected 복원도 mtime 갱신 (M6 §4)");
        assert_eq!(std::fs::read_to_string(sb.root.join("tests/t.rs")).unwrap(), "ORIGINAL");
        sb.cleanup();
    }
}
