pub mod openai;
pub mod vision;

#[derive(Debug, Clone)]
pub struct LLMOptions {
    pub temperature: f32,
    pub max_tokens: usize,
    pub stream: bool,
}

impl Default for LLMOptions {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 2048,
            stream: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LLMError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Not configured")]
    NotConfigured,
}

pub trait LLMProvider: Send + Sync {
    fn complete(
        &self,
        prompt: &str,
        options: LLMOptions,
    ) -> impl std::future::Future<Output = Result<String, LLMError>> + Send;

    fn complete_json(
        &self,
        prompt: &str,
        options: LLMOptions,
    ) -> impl std::future::Future<Output = Result<serde_json::Value, LLMError>> + Send;

    /// 多輪對話：system prompt + history[(role,content)] + 本輪 user query
    fn complete_with_history(
        &self,
        system_prompt: &str,
        history: &[(String, String)],
        user_query: &str,
        options: LLMOptions,
    ) -> impl std::future::Future<Output = Result<String, LLMError>> + Send;

    /// Streaming 版本：每個 token 透過 on_token 回調推送
    fn complete_stream(
        &self,
        system_prompt: &str,
        history: &[(String, String)],
        user_query: &str,
        options: LLMOptions,
        on_token: impl Fn(String) + Send + 'static,
    ) -> impl std::future::Future<Output = Result<String, LLMError>> + Send;
}
