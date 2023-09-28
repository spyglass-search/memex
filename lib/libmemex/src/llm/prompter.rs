use std::{collections::HashMap, path::PathBuf};

use handlebars::RenderError;
use serde::Serialize;

use super::openai::ChatMessage;

pub fn build_prompt<T>(template_path: PathBuf, data: &T) -> Result<String, RenderError>
where
    T: Serialize,
{
    let mut reg = handlebars::Handlebars::new();
    reg.register_escape_fn(handlebars::no_escape);

    let template = std::fs::read_to_string(template_path).expect("Invalid template path");
    reg.render_template(&template, data)
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
            &build_prompt("prompts/json_schema/prompt.txt".into(), &data).unwrap(),
        ),
    ]
}
