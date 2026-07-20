//! 실레포 과제의 조달·오라클 메타데이터 (M15 H11).
//!
//! `task.toml`이 아니라 `<task_dir>/procure.toml`에 사는 이유는 둘이다:
//! ① `TaskSpec`이 `deny_unknown_fields`라 키를 더하면 파싱이 죽는다
//! ② `Sandbox::create`가 `fixture/`만 복사하므로 이 파일은 **샌드박스에 안 실린다**
//!    — 모델이 정답 커밋과 오라클 파일 목록을 읽을 수 없다.
//!
//! `load_tasks`는 이 파일을 모른다(무변경). 읽는 곳은 `run_eval`의 리포트
//! 조립 지점 하나이며, 조달 스크립트(`scripts/procure_real.sh`)가 같은 TOML을
//! 입력으로 쓴다 — **형식이 두 소비자의 계약이다.**

use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// `<task_dir>/procure.toml`. 미지 키는 오타로 간주해 거부 — task.toml과 동일 정책
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProcureSpec {
    /// pristine 클론 디렉터리 이름 (예: "ripgrep")
    pub repo: String,
    /// 원 이슈 URL — 사전등록 항목 4(표본 동결)의 좌표
    pub issue_url: String,
    /// 이 이슈를 고친 커밋
    pub fix_sha: String,
    /// 픽스처의 출처 = fix_sha의 부모. 조달은 이 트리를 뽑는다
    pub parent_sha: String,
    /// 오라클 = 정답 커밋의 **비테스트 소스** 파일 (§5-4 제약 2).
    /// CHANGELOG·문서를 배제한 **명시 목록**이다 — 레포마다 관례가 달라
    /// 자동 규칙으로는 못 좁힌다. 리포트에 동결돼 사후 변경이 막힌다
    #[serde(default)]
    pub oracle_files: Vec<String>,
}

/// `<task_dir>/procure.toml`을 읽는다. 파일이 없으면 `Ok(None)` — 기존 두 트리의
/// 15개 과제가 그 상태다. **파싱 실패는 에러다**: 조용히 무시하면 오라클 목록이
/// 빈 채로 배치가 돌고 항해/수선 지표가 전부 "해당 없음"이 되는 fail-open이 된다
pub fn load(task_dir: &Path) -> anyhow::Result<Option<ProcureSpec>> {
    let path = task_dir.join("procure.toml");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("procure.toml 읽기 실패: {}", path.display()))?;
    let spec: ProcureSpec = toml::from_str(&text)
        .with_context(|| format!("procure.toml 파싱 실패: {}", path.display()))?;
    Ok(Some(spec))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
repo = "ripgrep"
issue_url = "https://github.com/BurntSushi/ripgrep/issues/1234"
fix_sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
parent_sha = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
oracle_files = ["crates/core/flags/hiargs.rs"]
"#;

    #[test]
    fn missing_file_is_none_not_an_error() {
        // 기존 두 트리의 15개 과제가 이 경로를 탄다
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().is_none());
    }

    #[test]
    fn reads_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("procure.toml"), SAMPLE).unwrap();
        let s = load(dir.path()).unwrap().unwrap();
        assert_eq!(s.repo, "ripgrep");
        assert_eq!(s.fix_sha.len(), 40);
        assert_eq!(s.oracle_files, vec!["crates/core/flags/hiargs.rs".to_string()]);
    }

    #[test]
    fn oracle_files_defaults_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let no_oracle = SAMPLE.lines().filter(|l| !l.starts_with("oracle_files")).collect::<Vec<_>>().join("\n");
        std::fs::write(dir.path().join("procure.toml"), no_oracle).unwrap();
        assert!(load(dir.path()).unwrap().unwrap().oracle_files.is_empty());
    }

    #[test]
    fn unknown_key_is_rejected() {
        // 오타가 조용히 무시되면 오라클이 빈 채로 배치가 돈다 (fail-open)
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("procure.toml"),
            format!("{SAMPLE}oracle_file = [\"typo.rs\"]\n"),
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("procure.toml"), "{err:#}");
    }

    #[test]
    fn malformed_toml_is_an_error_not_a_silent_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("procure.toml"), "repo = \n").unwrap();
        assert!(load(dir.path()).is_err());
    }
}
