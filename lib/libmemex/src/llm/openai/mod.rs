use reqwest::{header, Response, StatusCode};
use serde::Serialize;
use strum_macros::{AsRefStr, Display};
use thiserror::Error;
use tiktoken_rs::cl100k_base;

use self::schema::ErrorResponse;

mod schema;

const CONTEXT_LENGTH_ERROR: &str = "context_length_exceeded";
// Max context - response length - prompt length
pub const MAX_TOKENS: usize = 4_097 - 1_024 - 100;
pub const MAX_16K_TOKENS: usize = 16_384 - 2_048 - 100;

#[derive(AsRefStr, Display, Clone)]
pub enum OpenAIModel {
    // Most capable GPT-3.5 model and optimized for chat at 1/10th the cost of text-davinci-003.
    // Will be updated with our latest model iteration 2 weeks after it is released.
    #[strum(serialize = "gpt-3.5-turbo")]
    GPT35,
    // Same capabilities as the standard gpt-3.5-turbo model but with 4 times the context
    #[strum(serialize = "gpt-3.5-turbo-16k")]
    GPT35_16K,
    // Snapshot of gpt-3.5-turbo from June 13th 2023 with function calling data.
    // Unlike gpt-3.5-turbo, this model will not receive updates, and will be deprecated
    // 3 months after a new version is released.
    // Supports new functions
    #[strum(serialize = "gpt-3.5-turbo-0613")]
    GPT35_0613,
    #[strum(serialize = "gpt-4")]
    GPT4_8K,
}

#[derive(Debug, Error)]
pub enum OpenAIError {
    #[error("Context length exceeded: {0}")]
    ContextLengthExceeded(String),
    #[error("No response received")]
    NoResponse,
    #[error("Request Error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Unable to deserialize: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Invalid Request: {0}")]
    Other(String),
}

impl From<ErrorResponse> for OpenAIError {
    fn from(value: ErrorResponse) -> Self {
        if value.error.code == CONTEXT_LENGTH_ERROR {
            OpenAIError::ContextLengthExceeded(value.error.message)
        } else {
            OpenAIError::Other(value.error.message)
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct ChatMessage {
    role: String,
    content: String,
}

impl ChatMessage {
    pub fn new(role: &str, content: &str) -> Self {
        Self {
            role: role.to_string(),
            content: content.to_string(),
        }
    }
}

#[derive(Serialize, Debug)]
struct CompletionRequest {
    max_tokens: i32,
    n: i32,
    temperature: f32,
    frequency_penalty: f32,
    presence_penalty: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<String>,
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

impl CompletionRequest {
    pub fn new(model: &OpenAIModel, msgs: &[ChatMessage]) -> Self {
        Self {
            max_tokens: 1024,
            n: 1,
            // Make more deterministic
            temperature: 0.2,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            stop: None,
            model: model.to_string(),
            messages: msgs.to_vec(),
            stream: false,
        }
    }
}

/// Helper function to parse error messages from the OpenAI API response.
async fn check_api_error(response: Response) -> OpenAIError {
    // Grab the raw response body
    let raw_body = match response.text().await {
        Ok(raw) => raw,
        Err(err) => return OpenAIError::Other(format!("Invalid response: {err}")),
    };
    // Attempt to parse into an error object, otherwise return the raw message.
    match serde_json::from_str::<schema::ErrorResponse>(&raw_body) {
        Ok(error) => error.into(),
        Err(err) => OpenAIError::Other(format!("Error: {err}, raw response: {raw_body}")),
    }
}

#[derive(Clone)]
pub struct OpenAIClient {
    client: reqwest::Client,
}

impl OpenAIClient {
    pub fn new(api_key: &str) -> Self {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {api_key}")).expect("Invalid api_key"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("Unable to build HTTP client");

        Self { client }
    }

    pub async fn chat_completion(
        &self,
        model: &OpenAIModel,
        msgs: &[ChatMessage],
    ) -> anyhow::Result<String, OpenAIError> {
        log::debug!(
            "[OpenAI] chat completion w/ {} | {} messages",
            model,
            msgs.len()
        );

        let request_body = CompletionRequest::new(model, msgs);
        let response = self
            .client
            .post(&"https://api.openai.com/v1/chat/completions".to_string())
            .json(&request_body)
            .send()
            .await?;

        let status = &response.status();
        if StatusCode::is_success(status) {
            let completion = response
                .json::<schema::ChatCompletionResponse>()
                .await
                .map_err(OpenAIError::RequestError)?;

            match completion.response() {
                Some(msg) => Ok(msg),
                None => Err(OpenAIError::NoResponse),
            }
        } else if StatusCode::is_client_error(status) || StatusCode::is_server_error(status) {
            Err(check_api_error(response).await)
        } else {
            let warning = format!("OpenAI response not currently supported {:?}", response);
            log::warn!("{}", &warning);
            Err(OpenAIError::Other(warning))
        }
    }
}

pub fn segment(content: &str) -> (Vec<String>, OpenAIModel) {
    let cl = cl100k_base().unwrap();
    let size = cl.encode_with_special_tokens(content).len();

    log::debug!("Context Size {:?}", size);
    if size < MAX_TOKENS {
        log::debug!("Using standard model");
        (vec![content.to_string()], OpenAIModel::GPT35)
    } else if size > MAX_TOKENS && size < MAX_16K_TOKENS {
        log::debug!("Using 16k model");
        (vec![content.to_string()], OpenAIModel::GPT35_16K)
    } else {
        let splits = split_text(content, MAX_16K_TOKENS);
        log::debug!("Spliting with 16K model splits {:?}", splits.len());
        (splits, OpenAIModel::GPT35_16K)
    }
}

/// Truncates a blob of text to the max token size
pub fn truncate_text(text: &str) -> (String, OpenAIModel) {
    let cl = cl100k_base().unwrap();
    let total_tokens: usize = cl.encode_with_special_tokens(text).len();

    if total_tokens <= MAX_TOKENS {
        (text.to_string(), OpenAIModel::GPT35)
    } else if total_tokens <= MAX_16K_TOKENS {
        (text.to_string(), OpenAIModel::GPT35_16K)
    } else {
        let mut buffer = String::new();
        for txt in text.split(' ') {
            let with_txt = buffer.clone() + txt;
            let current_size = cl.encode_with_special_tokens(&with_txt).len();
            if current_size > MAX_16K_TOKENS {
                break;
            } else {
                buffer.push_str(txt);
            }
        }

        (buffer, OpenAIModel::GPT35_16K)
    }
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

#[cfg(test)]
mod test {
    use crate::llm::prompter::{json_schema_extraction, summarize};

    use super::{ChatMessage, OpenAIClient, OpenAIModel};

    #[ignore]
    #[tokio::test]
    pub async fn test_completion_api() {
        dotenv::dotenv().ok();
        let client = OpenAIClient::new(&std::env::var("OPENAI_API_KEY").unwrap());
        let msgs = vec![
            ChatMessage::new("system", "You are a helpful assistant"),
            ChatMessage::new("user", "Who won the world series in 2020?"),
        ];

        let resp = client.chat_completion(&OpenAIModel::GPT35, &msgs).await;
        // dbg!(&resp);
        assert!(resp.is_ok());
    }

    #[ignore]
    #[tokio::test]
    pub async fn test_json_prompting() {
        dotenv::dotenv().ok();
        let client = OpenAIClient::new(&std::env::var("OPENAI_API_KEY").unwrap());

        let msgs = json_schema_extraction(
            include_str!("../../../../../fixtures/sample_yelp_review.txt"),
            "extract the sentiment and complaints from this review",
            include_str!("../../../../../fixtures/sample_json_schema.json"),
        );

        let resp = client
            .chat_completion(&OpenAIModel::GPT35, &msgs)
            .await
            .unwrap();
        dbg!(&resp);
        assert!(!resp.is_empty());
    }

    #[ignore]
    #[tokio::test]
    pub async fn test_summarize() {
        dotenv::dotenv().ok();
        let client = OpenAIClient::new(&std::env::var("OPENAI_API_KEY").unwrap());

        let msgs = summarize(include_str!(
            "../../../../../fixtures/sample_yelp_review.txt"
        ));
        let resp = client
            .chat_completion(&OpenAIModel::GPT35, &msgs)
            .await
            .unwrap();

        let resp = resp.split('\n').into_iter().collect::<Vec<_>>();
        assert!(!resp.is_empty());
        dbg!(&resp);
    }
}
