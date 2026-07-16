//! 판정 무결성 — 샌드박스 밖 cargo config 변조 감지 (M7 스펙 §5).
//! 하네스 시작 시 감시 대상 파일 상태를 스냅샷하고 매 런 check 직전에 비교한다.
//! 존재-중단(cargo_tripwire)이 아니라 **변화-감지** — 사전 존재하는 정당 config는
//! 수용하고, 측정 중의 상태 전이만 변조로 본다. 정리는 하지 않는다.

use std::path::{Path, PathBuf};

/// 파일 상태 3종 — 상태 전이 일체(내용↔부재↔읽기불가)가 변조다 (M7 §5)
#[derive(Debug, PartialEq)]
enum FileState {
    Absent,
    Unreadable,
    Content(Vec<u8>),
}

fn state_of(path: &Path) -> FileState {
    match std::fs::read(path) {
        Ok(bytes) => FileState::Content(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileState::Absent,
        Err(_) => FileState::Unreadable,
    }
}

/// env CARGO_HOME → 없으면 홈 밑 `.cargo`. 둘 다 불가면 None (호출부가 감시 생략을 알림)
pub fn resolve_cargo_home() -> Option<PathBuf> {
    // let-chain 병합 — clippy 1.97(edition 2024)의 collapsible_if가 -D warnings에서 에러
    if let Some(v) = std::env::var_os("CARGO_HOME")
        && !v.is_empty()
    {
        return Some(PathBuf::from(v));
    }
    std::env::home_dir().map(|h| h.join(".cargo"))
}

#[derive(Debug)]
pub struct CargoConfigSnapshot {
    entries: Vec<(PathBuf, FileState)>,
}

impl CargoConfigSnapshot {
    /// 감시 대상: ① cargo_home의 config.toml·config(레거시명), ② temp_dir의 **상위**
    /// 조상(루트까지) 각각의 .cargo/config.toml·.cargo/config. temp_dir 자체는 기존
    /// 트립와이어(존재-중단) 관할이라 제외. 조상 열거는 canonicalize 기준 — cargo의
    /// 상향 걷기는 심링크 해소된 cwd 기준이다 (macOS /var→/private/var, M7 §5)
    pub fn take(cargo_home: Option<&Path>, temp_dir: &Path) -> Self {
        let mut paths = Vec::new();
        if let Some(home) = cargo_home {
            paths.push(home.join("config.toml"));
            paths.push(home.join("config"));
        }
        let canon = temp_dir.canonicalize().unwrap_or_else(|_| temp_dir.to_path_buf());
        let mut cur = canon.parent();
        while let Some(dir) = cur {
            let dot_cargo = dir.join(".cargo");
            paths.push(dot_cargo.join("config.toml"));
            paths.push(dot_cargo.join("config"));
            cur = dir.parent();
        }
        let entries = paths
            .into_iter()
            .map(|p| {
                let s = state_of(&p);
                (p, s)
            })
            .collect();
        Self { entries }
    }

    /// 스냅샷 대비 상태 전이가 있으면 변조로 판단해 에러 (하네스 중단 — exit 1)
    pub fn verify_unchanged(&self) -> anyhow::Result<()> {
        for (path, then) in &self.entries {
            if state_of(path) != *then {
                anyhow::bail!(
                    "판정 무결성 경고: 측정 시작 후 cargo 설정 파일이 변경되었습니다 ({}) — check가 오염된 설정을 읽을 수 있어 중단합니다",
                    path.display()
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// cargo_home 주입용 임시 구조: <tmp>/cargo-home + 조상 열거용 <tmp>/a/b/T
    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let cargo_home = dir.path().join("cargo-home");
        std::fs::create_dir_all(&cargo_home).unwrap();
        let deep_temp = dir.path().join("a/b/T");
        std::fs::create_dir_all(&deep_temp).unwrap();
        (dir, cargo_home, deep_temp)
    }

    #[test]
    fn unchanged_passes() {
        let (_d, ch, t) = setup();
        std::fs::write(ch.join("config.toml"), "[build]\n").unwrap();
        let snap = CargoConfigSnapshot::take(Some(&ch), &t);
        assert!(snap.verify_unchanged().is_ok(), "사전 존재 config는 수용 (변화-감지)");
    }

    #[test]
    fn content_change_is_detected() {
        let (_d, ch, t) = setup();
        std::fs::write(ch.join("config.toml"), "[build]\n").unwrap();
        let snap = CargoConfigSnapshot::take(Some(&ch), &t);
        std::fs::write(ch.join("config.toml"), "[target.'cfg(all())']\nrunner = \"evil\"\n").unwrap();
        let err = snap.verify_unchanged().unwrap_err();
        assert!(err.to_string().contains("config.toml"), "{err}");
    }

    #[test]
    fn creation_is_detected() {
        let (_d, ch, t) = setup();
        let snap = CargoConfigSnapshot::take(Some(&ch), &t);
        std::fs::write(ch.join("config"), "poison\n").unwrap();
        assert!(snap.verify_unchanged().is_err(), "부재→존재 전이도 변조 (레거시명 포함)");
    }

    #[test]
    fn deletion_is_detected() {
        let (_d, ch, t) = setup();
        std::fs::write(ch.join("config.toml"), "[build]\n").unwrap();
        let snap = CargoConfigSnapshot::take(Some(&ch), &t);
        std::fs::remove_file(ch.join("config.toml")).unwrap();
        assert!(snap.verify_unchanged().is_err(), "존재→부재 전이도 변조");
    }

    #[test]
    fn ancestors_above_temp_dir_are_watched() {
        // temp_dir(<tmp>/a/b/T)의 상위 조상 <tmp>/a에 config를 심으면 감지 (M7 §5 ②)
        let (d, _ch, t) = setup();
        let snap = CargoConfigSnapshot::take(None, &t);
        std::fs::create_dir_all(d.path().join("a/.cargo")).unwrap();
        std::fs::write(d.path().join("a/.cargo/config.toml"), "runner poison\n").unwrap();
        let err = snap.verify_unchanged().unwrap_err();
        assert!(err.to_string().contains(".cargo"), "{err}");
    }
}
