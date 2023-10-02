use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ChatCompletionResponse {
    id: String,
    object: String,
    created: i64,
    model: String,
    usage: Usage,
    choices: Vec<Choice>,
}

impl ChatCompletionResponse {
    pub fn response(&self) -> Option<String> {
        self.choices
            .last()
            .and_then(|choice| choice.message.content.to_owned())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Choice {
    message: Message,
    finish_reason: String,
    index: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    role: String,
    content: Option<String>,
    function_call: Option<FunctionCallResponse>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FunctionCallResponse {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiError {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub param: String,
    pub code: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorResponse {
    pub error: ApiError,
}
