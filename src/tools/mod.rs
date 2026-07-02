pub mod path;

use std::path::PathBuf;

/// 툴 실행 에러 — 크래시가 아니라 모델에게 되먹이는 데이터 (스펙 §9).
/// 표시 메시지는 영어: 모델 대상 텍스트이기 때문 (스펙 §4).
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("invalid arguments: {0}")]
    BadArgs(String),
    #[error("path not allowed: {0}")]
    PathViolation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("not a UTF-8 text file: {0}")]
    NotUtf8(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unknown tool: {0}")]
    UnknownTool(String),
}

/// 툴 실행 문맥. 모든 경로는 이 루트 기준 (스펙 §4 경로 확인)
pub struct ToolCtx {
    pub root: PathBuf,
}

pub trait Tool {
    /// 스키마 enum과 디스패치에 쓰이는 이름
    fn name(&self) -> &'static str;
    /// 시스템 프롬프트에 들어갈 한 줄 설명 (영어, 시그니처 포함)
    fn doc(&self) -> &'static str;
    /// M3 확인 게이트 대상 여부. M2 툴은 전부 읽기 전용
    fn is_mutating(&self) -> bool {
        false
    }
    fn run(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<String, ToolError>;
}

pub struct Registry {
    tools: Vec<Box<dyn Tool>>,
}

impl Registry {
    pub fn new(tools: Vec<Box<dyn Tool>>) -> Self {
        Self { tools }
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.tools.iter().map(|t| t.name()).collect()
    }

    /// 시스템 프롬프트용 툴 설명 목록 ("- name(args): ..." 줄들)
    pub fn docs(&self) -> String {
        self.tools
            .iter()
            .map(|t| format!("- {}", t.doc()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn dispatch(
        &self,
        name: &str,
        args: &serde_json::Value,
        ctx: &ToolCtx,
    ) -> Result<String, ToolError> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| ToolError::UnknownTool(name.to_string()))?;
        tool.run(args, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;
    impl Tool for Echo {
        fn name(&self) -> &'static str {
            "echo"
        }
        fn doc(&self) -> &'static str {
            "echo(text): Echo back `text`."
        }
        fn run(&self, args: &serde_json::Value, _ctx: &ToolCtx) -> Result<String, ToolError> {
            Ok(args["text"].as_str().unwrap_or("").to_string())
        }
    }

    fn ctx() -> ToolCtx {
        ToolCtx { root: PathBuf::from(".") }
    }

    #[test]
    fn registry_dispatches_by_name() {
        let reg = Registry::new(vec![Box::new(Echo)]);
        let out = reg
            .dispatch("echo", &serde_json::json!({"text": "hi"}), &ctx())
            .unwrap();
        assert_eq!(out, "hi");
    }

    #[test]
    fn registry_unknown_tool_is_error() {
        let reg = Registry::new(vec![Box::new(Echo)]);
        let err = reg.dispatch("teleport", &serde_json::json!({}), &ctx()).unwrap_err();
        assert!(matches!(err, ToolError::UnknownTool(_)));
        assert!(err.to_string().contains("teleport"));
    }

    #[test]
    fn registry_docs_and_names_list_tools() {
        let reg = Registry::new(vec![Box::new(Echo)]);
        assert_eq!(reg.names(), vec!["echo"]);
        assert!(reg.docs().contains("- echo(text)"));
    }
}
