use std::collections::HashMap;

use handlebars::RenderError;
use serde::Serialize;

use super::openai::ChatMessage;

pub fn build_prompt<T>(template: &str, data: &T) -> Result<String, RenderError>
where
    T: Serialize,
{
    let mut reg = handlebars::Handlebars::new();
    reg.register_escape_fn(handlebars::no_escape);
    reg.render_template(template, data)
}

pub fn quick_question(user_request: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage::new("system", "You are a helpful assistant"),
        ChatMessage::new("user", user_request),
    ]
}

pub fn json_schema_extraction(
    input_data: &str,
    user_request: &str,
    output_schema: &str,
) -> Vec<ChatMessage> {
    let mut data: HashMap<String, String> = HashMap::new();
    data.insert("user_request".to_string(), user_request.to_string());
    data.insert("json_schema".to_string(), output_schema.to_string());

    vec![
        ChatMessage::new(
            "system",
            include_str!("../../prompts/json_schema/system.txt"),
        ),
        ChatMessage::new("user", input_data),
        ChatMessage::new(
            "user",
            &build_prompt(include_str!("../../prompts/json_schema/prompt.txt"), &data).unwrap(),
        ),
    ]
}
