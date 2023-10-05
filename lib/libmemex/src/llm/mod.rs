use serde::Serialize;
use strum_macros::Display;
use thiserror::Error;
use tiktoken_rs::cl100k_base;

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
pub trait LLM: Send + Sync {
    async fn chat_completion(
        &self,
        model: &str,
        msgs: &[ChatMessage],
    ) -> anyhow::Result<String, LLMError>;

    fn segment_text(&self, text: &str) -> (Vec<String>, String);
    fn truncate_text(&self, text: &str) -> (String, String);
}

pub fn split_text(text: &str, max_tokens: usize) -> Vec<String> {
    let cl = cl100k_base().unwrap();

    let total_tokens: usize = cl.encode_with_special_tokens(text).len();
    let mut doc_parts = Vec::new();
    if total_tokens <= max_tokens {
        doc_parts.push(text.into());
    } else {
        let split_count = total_tokens
            .checked_div(max_tokens)
            .map(|val| val + 2)
            .unwrap_or(1);
        let split_size = text.len().checked_div(split_count).unwrap_or(text.len());
        if split_size == text.len() {
            doc_parts.push(text.into());
        } else {
            let mut part = Vec::new();
            let mut size = 0;
            for txt in text.split(' ') {
                if (size + txt.len()) > split_size {
                    doc_parts.push(part.join(" "));
                    let mut end = part.len();
                    if part.len() > 10 {
                        end = part.len() - 10;
                    }
                    part.drain(0..end);
                    size = part.join(" ").len();
                }
                size += txt.len() + 1;
                part.push(txt);
            }
            if !part.is_empty() {
                doc_parts.push(part.join(" "));
            }
        }
    }

    doc_parts
        .iter()
        .map(|pt| pt.to_string())
        .collect::<Vec<String>>()
}
