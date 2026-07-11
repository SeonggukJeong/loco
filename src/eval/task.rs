//! 과제 정의 로드·검증 (설계 §3). 과제 하나 = 디렉터리 하나 (task.toml + fixture/).
//! 정의 오류는 실행 시작 전 하네스 에러로 일괄 보고한다 (스펙 §8 종료 코드 1).

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::Deserialize;

/// task.toml 스키마 (설계 §3). 미지 키는 오타로 간주해 거부 — config와 동일 정책
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpec {
    /// 에이전트에게 줄 요청 (실사용과 같은 한국어)
    pub prompt: String,
    /// 샌드박스 루트에서 실행할 판정 명령 — 종료 코드 0이면 통과
    pub check: String,
    /// 에이전트 실행 전체(LLM 호출 포함) 상한. 파싱 재시도 탓에 최악
    /// LLM 호출 = max_turns×3회 (스펙 §8) — 넉넉하게
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// check 명령 상한 (콜드 빌드 감안)
    #[serde(default = "default_check_timeout_secs")]
    pub check_timeout_secs: u64,
    /// 설정보다 우선하는 과제별 턴 상한
    pub max_turns: Option<usize>,
    /// 판정 자산 — check 전에 fixture 원본과 정확히 일치하도록 동기화 (스펙 §8)
    pub protected: Vec<String>,
}

fn default_timeout_secs() -> u64 {
    300
}

fn default_check_timeout_secs() -> u64 {
    120
}

#[derive(Debug)]
pub struct Task {
    pub name: String,
    pub fixture: PathBuf,
    pub spec: TaskSpec,
}

pub fn load_tasks(tasks_dir: &Path) -> anyhow::Result<Vec<Task>> {
    let mut tasks = Vec::new();
    let entries = std::fs::read_dir(tasks_dir)
        .with_context(|| format!("과제 디렉터리를 열 수 없음: {}", tasks_dir.display()))?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue; // .gitattributes 등 과제 아닌 파일은 무시
        }
        let dir = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(dir.join("task.toml"))
            .with_context(|| format!("과제 {name}: task.toml 읽기 실패"))?;
        let spec: TaskSpec =
            toml::from_str(&text).with_context(|| format!("과제 {name}: task.toml 파싱 실패"))?;
        let fixture = dir.join("fixture");
        if !fixture.is_dir() {
            bail!("과제 {name}: fixture/ 디렉터리가 없음");
        }
        if spec.protected.is_empty() {
            bail!("과제 {name}: protected가 비어 있음 — 판정 자산 없이는 공정한 채점이 불가 (스펙 §8)");
        }
        for p in &spec.protected {
            if !fixture.join(p).exists() {
                bail!("과제 {name}: protected 경로가 fixture에 없음: {p}");
            }
        }
        tasks.push(Task { name, fixture, spec });
    }
    if tasks.is_empty() {
        bail!("과제가 없음: {}", tasks_dir.display());
    }
    tasks.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
prompt = "p"
check = "true"
protected = ["keep.txt"]
"#;

    fn write_task(root: &Path, name: &str, toml: &str, fixture_files: &[&str]) {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("fixture")).unwrap();
        std::fs::write(dir.join("task.toml"), toml).unwrap();
        for f in fixture_files {
            let p = dir.join("fixture").join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
    }

    #[test]
    fn loads_sorted_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "b-task", MINIMAL, &["keep.txt"]);
        write_task(dir.path(), "a-task", MINIMAL, &["keep.txt"]);
        let tasks = load_tasks(dir.path()).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "a-task", "이름순 정렬");
        assert_eq!(tasks[0].spec.timeout_secs, 300, "기본값");
        assert_eq!(tasks[0].spec.check_timeout_secs, 120);
        assert_eq!(tasks[0].spec.max_turns, None);
    }

    #[test]
    fn overrides_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let toml = r#"
prompt = "p"
check = "cargo test"
timeout_secs = 60
check_timeout_secs = 30
max_turns = 10
protected = ["keep.txt"]
"#;
        write_task(dir.path(), "t", toml, &["keep.txt"]);
        let t = &load_tasks(dir.path()).unwrap()[0];
        assert_eq!(t.spec.timeout_secs, 60);
        assert_eq!(t.spec.check_timeout_secs, 30);
        assert_eq!(t.spec.max_turns, Some(10));
    }

    #[test]
    fn unknown_key_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "t", "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\ntimout_secs = 3\n", &["keep.txt"]);
        let err = load_tasks(dir.path()).unwrap_err();
        assert!(err.to_string().contains("t"), "{err:#}");
    }

    #[test]
    fn missing_fixture_dir_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("t");
        std::fs::create_dir_all(&t).unwrap();
        std::fs::write(t.join("task.toml"), MINIMAL).unwrap();
        assert!(load_tasks(dir.path()).unwrap_err().to_string().contains("fixture"));
    }

    #[test]
    fn empty_protected_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "t", "prompt = \"p\"\ncheck = \"true\"\nprotected = []\n", &["keep.txt"]);
        assert!(load_tasks(dir.path()).unwrap_err().to_string().contains("protected"));
    }

    #[test]
    fn protected_path_must_exist_in_fixture() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "t", MINIMAL, &["other.txt"]); // keep.txt 없음
        assert!(load_tasks(dir.path()).unwrap_err().to_string().contains("keep.txt"));
    }

    #[test]
    fn empty_tasks_dir_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_tasks(dir.path()).is_err());
    }
}
