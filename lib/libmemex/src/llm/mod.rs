use serde::Serialize;
use strum_macros::Display;
use thiserror::Error;

pub mod embedding;
pub mod local;
pub mod openai;
pub mod prompter;

#[derive(Clone, Debug, Serialize, Display, Eq, PartialEq)]
pub enum ChatRole {
    #[strum(serialize = "system")]
    System,
    #[strum(serialize = "user")]
    User,
    #[strum(serialize = "assistant")]
    Assistant,
}

#[derive(Serialize, Debug, Clone)]
pub struct ChatMessage {
    role: ChatRole,
    content: String,
}

impl ChatMessage {
    pub fn assistant(content: &str) -> Self {
        Self::new(ChatRole::Assistant, content)
    }

    pub fn user(content: &str) -> Self {
        Self::new(ChatRole::User, content)
    }

    pub fn system(content: &str) -> Self {
        Self::new(ChatRole::System, content)
    }

    pub fn new(role: ChatRole, content: &str) -> Self {
        Self {
            role,
            content: content.to_string(),
        }
    }
}

#[derive(Debug, Error)]
pub enum LLMError {
    #[error("Context length exceeded: {0}")]
    ContextLengthExceeded(String),
    #[error("No response received")]
    NoResponse,
    #[error("Inference Error: {0}")]
    InferenceError(String),
    #[error("Request Error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Unable to deserialize: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Invalid Request: {0}")]
    Other(String),
}

#[async_trait::async_trait]
pub trait LLM<T> {
    async fn chat_completion(
        &self,
        model: T,
        msgs: &[ChatMessage],
    ) -> anyhow::Result<String, LLMError>;
}
