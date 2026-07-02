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
    /// M1에서는 파싱만 하고 사용하지 않음 (M3의 --auto 가드레일용).
    /// 스펙 §7에 문서화된 키를 deny_unknown_fields가 거부하지 않도록 지금 받는다.
    pub auto_deny_patterns: Vec<String>,
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
            auto_deny_patterns: Vec::new(),
        }
    }
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
        assert!(c.auto_deny_patterns.is_empty()); // 기본 차단 목록은 M3에서 도입
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
