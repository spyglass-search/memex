use std::collections::HashMap;

use handlebars::RenderError;
use serde::Serialize;

use super::ChatMessage;

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
        ChatMessage::system("You are a helpful assistant"),
        ChatMessage::user(user_request),
    ]
}

pub fn summarize(input_data: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(include_str!("../../prompts/summarize/system.txt")),
        ChatMessage::user(input_data),
        ChatMessage::user(include_str!("../../prompts/summarize/prompt.txt")),
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
        ChatMessage::system(include_str!("../../prompts/json_schema/system.txt")),
        ChatMessage::user(input_data),
        ChatMessage::user(
            &build_prompt(include_str!("../../prompts/json_schema/prompt.txt"), &data).unwrap(),
        ),
    ]
}
