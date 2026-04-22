pub mod model_caps;
pub mod openai;
pub mod vision;

#[derive(Debug, Clone)]
pub struct LLMOptions {
    pub temperature: f32,
    pub max_tokens: usize,
    pub stream: bool,
    pub think_mode: Option<bool>,
}

impl Default for LLMOptions {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 2048,
            stream: false,
            think_mode: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum StreamToken {
    Reasoning(String),
    Content(String),
}

#[derive(Debug, Clone, Default)]
pub struct StreamResult {
    pub reasoning: String,
    pub content: String,
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

    fn complete_with_history(
        &self,
        system_prompt: &str,
        history: &[(String, String)],
        user_query: &str,
        options: LLMOptions,
    ) -> impl std::future::Future<Output = Result<String, LLMError>> + Send;

    fn complete_stream(
        &self,
        system_prompt: &str,
        history: &[(String, String)],
        user_query: &str,
        options: LLMOptions,
        on_token: impl Fn(StreamToken) + Send + 'static,
    ) -> impl std::future::Future<Output = Result<StreamResult, LLMError>> + Send;
}
