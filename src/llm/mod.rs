pub mod client;
pub mod sse;
pub mod types;

use client::LlmError;
use types::{ChatRequest, ChatResponse};

/// agent 루프가 의존하는 최소 경계 (스펙 §3 핵심 트레이트).
/// 테스트에서 스크립트된 가짜 클라이언트를 주입할 수 있게 한다.
/// 크레이트 내부 전용 트레이트라 AFIT(Send 바운드 없음) 경고는 무시한다.
#[allow(async_fn_in_trait)]
pub trait LlmClient {
    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError>;
}

/// &OpenAiClient / &Scripted 형태로도 Agent에 넣을 수 있게 하는 블랭킷 impl
impl<T: LlmClient> LlmClient for &T {
    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        (**self).chat(req).await
    }
}
