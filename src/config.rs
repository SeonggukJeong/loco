use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub temperature: f32,
    pub context_tokens: usize,
    pub max_output_tokens: usize,
    pub max_turns: usize,
    pub command_timeout_secs: u64,
    /// --auto 가드레일 차단 패턴 (스펙 §5). 기본값은 [`default_deny_patterns`] 참고
    pub auto_deny_patterns: Vec<String>,
    /// Hierarchical repo notes onboarding (M16). Product default on; eval forces
    /// off for non-`tasks-real` dirs (see `eval::apply_eval_repo_notes_policy`).
    pub repo_notes: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:1234/v1".into(),
            api_key: None,
            model: String::new(),
            temperature: 0.1,
            context_tokens: 8192,
            max_output_tokens: 2048,
            max_turns: 25,
            command_timeout_secs: 60,
            auto_deny_patterns: default_deny_patterns(),
            repo_notes: true,
        }
    }
}

/// --auto 가드레일 기본 차단 목록 (스펙 §5 원문 그대로 — 크로스플랫폼, 최선 노력).
/// 대소문자 무시로 컴파일된다 (PowerShell/cmd 관례)
pub fn default_deny_patterns() -> Vec<String> {
    [
        // Unix
        "sudo", r"rm\s+-\w*[rf]", "mkfs", r"dd\s+if=", "shutdown",
        // Windows
        r"rd\s+/s", r"del\s+/[fsq]", r"format\s", r"Remove-Item\s+.*-Recurse", r"reg\s+delete",
        // 공통
        r"git\s+push",
    ]
    .map(String::from)
    .to_vec()
}

/// 설정 파일 하나에서 읽는 부분 설정. 없는 키는 이전 레이어 값을 유지한다.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialConfig {
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    temperature: Option<f32>,
    context_tokens: Option<usize>,
    max_output_tokens: Option<usize>,
    max_turns: Option<usize>,
    command_timeout_secs: Option<u64>,
    auto_deny_patterns: Option<Vec<String>>,
    repo_notes: Option<bool>,
}

impl Config {
    fn apply(&mut self, p: PartialConfig) {
        if let Some(v) = p.base_url {
            self.base_url = v;
        }
        if let Some(v) = p.api_key {
            self.api_key = Some(v);
        }
        if let Some(v) = p.model {
            self.model = v;
        }
        if let Some(v) = p.temperature {
            self.temperature = v;
        }
        if let Some(v) = p.context_tokens {
            self.context_tokens = v;
        }
        if let Some(v) = p.max_output_tokens {
            self.max_output_tokens = v;
        }
        if let Some(v) = p.max_turns {
            self.max_turns = v;
        }
        if let Some(v) = p.command_timeout_secs {
            self.command_timeout_secs = v;
        }
        if let Some(v) = p.auto_deny_patterns {
            self.auto_deny_patterns = v;
        }
        if let Some(v) = p.repo_notes {
            self.repo_notes = v;
        }
    }

    pub fn load(paths: &[PathBuf]) -> anyhow::Result<Config> {
        let mut cfg = Config::default();
        for path in paths {
            if !path.exists() {
                continue;
            }
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("설정 파일 읽기 실패: {}", path.display()))?;
            let partial: PartialConfig = toml::from_str(&text)
                .with_context(|| format!("설정 파일 파싱 실패: {}", path.display()))?;
            cfg.apply(partial);
        }
        Ok(cfg)
    }

    /// 전역 설정 파일 경로 (플랫폼별 config_dir)
    pub fn default_global_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("dev", "loco", "loco")
            .map(|d| d.config_dir().join("config.toml"))
    }

    pub fn load_default() -> anyhow::Result<Config> {
        let mut paths = Vec::new();
        if let Some(g) = Self::default_global_path() {
            paths.push(g);
        }
        paths.push(PathBuf::from(".loco/config.toml"));
        Self::load(&paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn defaults_match_spec() {
        let c = Config::default();
        assert_eq!(c.base_url, "http://localhost:1234/v1");
        assert_eq!(c.api_key, None);
        assert_eq!(c.model, "");
        assert_eq!(c.temperature, 0.1);
        assert_eq!(c.context_tokens, 8192);
        assert_eq!(c.max_output_tokens, 2048);
        assert_eq!(c.max_turns, 25);
        assert_eq!(c.command_timeout_secs, 60);
        // 기본 차단 목록 내장 (스펙 §5 — M3)
        assert!(c.auto_deny_patterns.iter().any(|p| p.contains("sudo")));
        assert!(c.auto_deny_patterns.iter().any(|p| p.contains("git")));
        assert!(c.auto_deny_patterns.len() >= 11);
        // M16: product default on (eval forces off for non-tasks-real)
        assert!(c.repo_notes);
    }

    #[test]
    fn repo_notes_false_in_toml_applies() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("c.toml");
        std::fs::write(&f, "repo_notes = false\n").unwrap();
        let c = Config::load(&[f]).unwrap();
        assert!(!c.repo_notes);
        // other defaults preserved
        assert_eq!(c.max_turns, 25);
    }

    #[test]
    fn later_file_overrides_earlier() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("global.toml");
        let project = dir.path().join("project.toml");
        std::fs::write(&global, "model = \"gemma-4b\"\ntemperature = 0.5\n").unwrap();
        std::fs::write(&project, "temperature = 0.2\n").unwrap();

        let c = Config::load(&[global, project]).unwrap();
        assert_eq!(c.model, "gemma-4b"); // global에서
        assert_eq!(c.temperature, 0.2); // project가 덮어씀
        assert_eq!(c.max_turns, 25); // 어디에도 없으면 기본값
    }

    #[test]
    fn missing_files_are_skipped() {
        let c = Config::load(&[PathBuf::from("/nonexistent/nowhere.toml")]).unwrap();
        assert_eq!(c, Config::default());
    }

    #[test]
    fn unknown_key_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("bad.toml");
        let mut file = std::fs::File::create(&f).unwrap();
        writeln!(file, "modell = \"typo\"").unwrap();
        let err = Config::load(&[f]).unwrap_err();
        assert!(err.to_string().contains("bad.toml"));
    }
}
