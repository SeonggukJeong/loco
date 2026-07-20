//! 과제 샌드박스 — fixture 복사, protected 동기화 (스펙 §8), 임시 디렉터리 관리.

use std::path::{Path, PathBuf};

use anyhow::Context;

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
                // M15 H17 — **세 번째 read+write 사이트**. tasks-real의 protected는
                // 단일 파일(`tests/<x>.rs`)이라 배치의 모든 런이 여기를 탄다
                // M15 H5 후속: 링크를 따라가야 한다 — 위 fs::read가 이미 링크를
                // 따라가므로(대상 내용을 읽음) 메타데이터도 같은 대상 기준이어야
                // 링크 자체의 mode(예: 0o777)가 새 나가지 않는다
                let meta = std::fs::metadata(&src)?;
                restore_mode(&meta, &dst)?;
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
            // M15 H5: 스킵 + 경고. 대상 레포의 심링크는 전부 문서·패키징용이고
            // (ripgrep HomebrewFormula, just www/man/{en,zh} — 후자 둘은 dangling)
            // 탈출 위험은 confine(path.rs:43-51)이 canonicalize 후 루트 검사로 이미
            // 닫는다. 스킵 항목은 조달 로그(scripts/procure_real.sh)가 함께 남긴다
            eprintln!("(심링크 건너뜀: {})", from.display());
            continue;
        }
        if meta.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_tree(&from, &to)?;
        } else {
            // read+write — fs::copy는 macOS에서 clonefile로 원본 mtime을 보존해
            // 스테일 빌드 캐시 판정 벡터가 된다. M6가 overlay_tree에서만 막았고
            // copy_tree에는 남아 있던 잔여 결함 (M15 H6)
            let bytes = std::fs::read(&from)
                .with_context(|| format!("픽스처 읽기 실패: {}", from.display()))?;
            std::fs::write(&to, bytes)
                .with_context(|| format!("픽스처 복사 실패: {}", to.display()))?;
            restore_mode(&meta, &to)
                .with_context(|| format!("퍼미션 복원 실패: {}", to.display()))?;
        }
    }
    Ok(())
}

/// read+write 복사가 잃는 원본 퍼미션을 복원한다 (M15 H17).
/// `| 0o200`으로 소유자 쓰기 비트를 강제하는 것은 읽기 전용 픽스처 파일이
/// `sync_protected`의 덮어쓰기(remove 후 write)나 에이전트 편집을 막지 않게 하기
/// 위함 — 원본 트리의 읽기 전용은 판정 자산 보호 수단이 아니고(그 역할은
/// protected 동기화가 한다) 하네스를 죽이는 실패 모드만 만든다
#[cfg(unix)]
fn restore_mode(meta: &std::fs::Metadata, to: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = meta.permissions().mode() | 0o200;
    std::fs::set_permissions(to, std::fs::Permissions::from_mode(mode))
}

/// Windows에는 유닉스 mode가 없다 — read+write가 잃는 것도 없다
#[cfg(not(unix))]
fn restore_mode(_meta: &std::fs::Metadata, _to: &Path) -> std::io::Result<()> {
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
            // copy_tree와 같은 정책 (M15 H5). 소비자 둘 — solution/ 오버레이,
            // sync_protected 디렉터리 분기 — 이 전부가 여기를 탄다
            eprintln!("(심링크 건너뜀: {})", from.display());
            continue;
        }
        if meta.is_dir() {
            std::fs::create_dir_all(&to)?;
            overlay_tree(&from, &to)?;
        } else {
            let bytes = std::fs::read(&from)?;
            std::fs::write(&to, bytes)
                .with_context(|| format!("오버레이 쓰기 실패: {}", to.display()))?;
            // M15 H17: copy_tree와 같은 이유 — read+write는 퍼미션을 잃는다.
            // 이 함수는 sync_protected를 통해 **모든 eval 런에서 check 직전**에 돈다
            restore_mode(&meta, &to)
                .with_context(|| format!("퍼미션 복원 실패: {}", to.display()))?;
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
    fn symlinks_are_skipped_not_an_error() {
        // M15 H5: 정책 = 스킵 + 경고. ripgrep의 HomebrewFormula(정상 심링크)와
        // just의 www/man/{en,zh}(dangling) 둘 다 이 경로를 탄다. 대상은 전부
        // 문서·패키징용이라 판정에 영향이 없고, 탈출 위험은 confine이 이미 닫는다
        let fx = fixture_with(&[("real.txt", "x")]);
        std::os::unix::fs::symlink(fx.path().join("real.txt"), fx.path().join("link.txt")).unwrap();
        // dangling — 대상이 없어도 bail이 아니라 스킵이어야 한다
        std::os::unix::fs::symlink(fx.path().join("nope.txt"), fx.path().join("dangling")).unwrap();

        let sb = Sandbox::create(fx.path()).unwrap();

        assert_eq!(std::fs::read_to_string(sb.root.join("real.txt")).unwrap(), "x", "실파일은 복사");
        assert!(sb.root.join("link.txt").symlink_metadata().is_err(), "심링크는 스킵");
        assert!(sb.root.join("dangling").symlink_metadata().is_err(), "dangling도 스킵");
        sb.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn overlay_tree_skips_symlinks() {
        // 소비자 둘(solution/ 오버레이·sync_protected 디렉터리 분기)이 이 함수를 탄다
        // — copy_tree만 고치면 solution/ 오버레이가 여기서 죽는다
        let src = fixture_with(&[("a.rs", "new")]);
        std::os::unix::fs::symlink(src.path().join("a.rs"), src.path().join("alias.rs")).unwrap();
        let dst = fixture_with(&[("a.rs", "stale")]);
        overlay_tree(src.path(), dst.path()).unwrap();
        assert_eq!(std::fs::read_to_string(dst.path().join("a.rs")).unwrap(), "new");
        assert!(dst.path().join("alias.rs").symlink_metadata().is_err(), "심링크는 스킵");
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

    #[test]
    fn copy_tree_refreshes_mtime() {
        // M6는 overlay_tree에서만 fs::copy를 막았다. copy_tree에는 같은 벡터가
        // 남아 있었다 — macOS의 fs::copy는 clonefile로 원본 mtime을 보존하므로
        // 픽스처가 과거 mtime을 가지면 샌드박스의 소스가 빌드 산출물보다
        // 과거가 되고 cargo가 재빌드를 건너뛴다 (M15 H6)
        let src = fixture_with(&[("a.rs", "new")]);
        let old = age_file(&src.path().join("a.rs"));
        let sb = Sandbox::create(src.path()).unwrap();
        let copied = std::fs::metadata(sb.root.join("a.rs")).unwrap().modified().unwrap();
        assert!(
            copied > old + std::time::Duration::from_secs(1800),
            "샌드박스 복사도 mtime을 갱신해야 함 (M15 H6)"
        );
        assert_eq!(std::fs::read_to_string(sb.root.join("a.rs")).unwrap(), "new");
        sb.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn copy_tree_preserves_the_executable_bit() {
        // read+write는 퍼미션을 잃는다(3R 실측 755→644). 실레포 픽스처는
        // ci/*.sh 같은 실행 파일을 갖고, mode 회귀를 잡을 기존 테스트가
        // 하나도 없었다 (M15 H17)
        use std::os::unix::fs::PermissionsExt;
        let fx = fixture_with(&[("run.sh", "#!/bin/sh\nexit 0\n"), ("plain.txt", "x")]);
        std::fs::set_permissions(
            fx.path().join("run.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        // 읽기 전용 픽스처 파일도 샌드박스에서는 덮어쓸 수 있어야 한다
        std::fs::set_permissions(
            fx.path().join("plain.txt"),
            std::fs::Permissions::from_mode(0o444),
        )
        .unwrap();

        let sb = Sandbox::create(fx.path()).unwrap();

        let exec = std::fs::metadata(sb.root.join("run.sh")).unwrap().permissions().mode();
        assert_eq!(exec & 0o777, 0o755, "실행 비트 보존 (M15 H17)");
        let plain = std::fs::metadata(sb.root.join("plain.txt")).unwrap().permissions().mode();
        assert_eq!(plain & 0o200, 0o200, "소유자 쓰기 비트 강제 — sync_protected 덮어쓰기 경로");
        sb.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn sync_protected_preserves_the_executable_bit() {
        // M15 H17 후반부 — overlay_tree는 M6 때부터 read+write라 **이미** 실행
        // 비트를 잃고 있었다. copy_tree만 고치면 절반만 닫힌다(플랜 1R 실현 I1).
        // 이 경로는 sync_protected를 통해 **모든 eval 런에서 check 직전**에 돈다
        use std::os::unix::fs::PermissionsExt;
        let fx = fixture_with(&[("ci/run.sh", "#!/bin/sh\nexit 0\n")]);
        std::fs::set_permissions(
            fx.path().join("ci/run.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let sb = Sandbox::create(fx.path()).unwrap();
        // 에이전트가 protected를 건드렸다고 가정 → 동기화가 overlay_tree를 탄다
        std::fs::write(sb.root.join("ci/run.sh"), "#!/bin/sh\nexit 1\n").unwrap();
        sb.sync_protected(fx.path(), &["ci".to_string()]).unwrap();

        let m = std::fs::metadata(sb.root.join("ci/run.sh")).unwrap().permissions().mode();
        assert_eq!(m & 0o777, 0o755, "protected 복원도 실행 비트를 보존해야 한다 (M15 H17)");
        sb.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn sync_protected_single_file_preserves_the_executable_bit() {
        // M15 H17 **세 번째 사이트** (2R 실현 I4·측정 A-3).
        // ⚠ 위 테스트는 protected가 **디렉터리**라 overlay_tree로 라우팅된다.
        // 그런데 tasks-real의 protected는 `tests/<x>.rs` — **단일 파일**이라
        // sync_protected의 :55-61 분기를 탄다. 즉 **배치의 모든 런이 타는 경로가
        // 이쪽인데 개정 2의 테스트는 그 경로를 시험하지 않았다.**
        // 실측(2R): 복원 없이는 create=755 → sync=644
        use std::os::unix::fs::PermissionsExt;
        let fx = fixture_with(&[("run.sh", "#!/bin/sh\nexit 0\n")]);
        std::fs::set_permissions(
            fx.path().join("run.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::write(sb.root.join("run.sh"), "#!/bin/sh\nexit 1\n").unwrap();
        // protected를 **단일 파일**로 준다 — tasks-real과 같은 형태
        sb.sync_protected(fx.path(), &["run.sh".to_string()]).unwrap();

        let m = std::fs::metadata(sb.root.join("run.sh")).unwrap().permissions().mode();
        assert_eq!(m & 0o777, 0o755, "단일 파일 protected 복원도 실행 비트 보존 (M15 H17)");
        sb.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn sync_protected_single_symlink_restores_target_mode_not_link_mode() {
        // M15 H5 후속·H17 회귀 테스트. Task 3가 copy_tree의 심링크 bail을 스킵+경고로
        // 바꾸면서(H5) protected 단일 파일이 심링크인 경우가 처음으로 도달 가능해졌다:
        // copy_tree가 그 심링크를 건너뛰므로 sync_protected가 처음부터 채워야 한다.
        // sync_protected의 단일 파일 분기(:59)는 fs::read(&src)로 링크를 따라가
        // 대상 내용을 읽는데, 메타데이터(:66)가 symlink_metadata였다면 링크 자체의
        // mode를 복원해 버려 조용한 퍼미션 확대가 된다. metadata(&src)로 대상을
        // 따라가야 이 줄이 실제로 올바르다.
        use std::os::unix::fs::PermissionsExt;
        let fx = fixture_with(&[("real.txt", "TARGET")]);
        let target_mode: u32 = 0o750; // 대상 파일의 "구별되는" mode
        std::fs::set_permissions(fx.path().join("real.txt"), std::fs::Permissions::from_mode(target_mode))
            .unwrap();
        std::os::unix::fs::symlink(fx.path().join("real.txt"), fx.path().join("link.txt")).unwrap();

        // 심링크 자체의 mode는 OS/umask가 정한다(예: 이 macOS 환경은 0o777이 아니라
        // umask 반영값인 0o755) — 대상 mode와 우연히 같으면 이 테스트가 판별력을
        // 잃으므로, 실제 환경에서 서로 다름을 사전조건으로 확인해 둔다
        let link_own_mode =
            std::fs::symlink_metadata(fx.path().join("link.txt")).unwrap().permissions().mode() & 0o777;
        assert_ne!(
            link_own_mode, target_mode,
            "테스트 전제 실패: 이 환경의 심링크 자체 mode가 우연히 대상 mode와 같음 — target_mode를 바꿔야 판별력이 생김"
        );

        // Sandbox::create는 copy_tree를 거치므로(H5) link.txt는 스킵된 채로 생성된다 —
        // sync_protected가 그 자리를 처음부터 채우는, 배치가 실제로 타는 경로
        let sb = Sandbox::create(fx.path()).unwrap();
        assert!(
            sb.root.join("link.txt").symlink_metadata().is_err(),
            "생성 시점엔 copy_tree가 심링크를 스킵해야 함 (H5)"
        );

        sb.sync_protected(fx.path(), &["link.txt".to_string()]).unwrap();

        let restored = sb.root.join("link.txt");
        assert_eq!(std::fs::read_to_string(&restored).unwrap(), "TARGET", "대상 내용이 복원돼야 함");
        let mode = std::fs::metadata(&restored).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, target_mode,
            "심링크 protected 복원은 대상의 실제 mode를 써야 한다 — 링크 자체의 mode가 아니라 (M15 H5·H17)"
        );
        sb.cleanup();
    }
}
