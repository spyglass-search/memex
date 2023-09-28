use crate::ServerError;
use jsonschema::JSONSchema;
use serde_json::json;
use warp::reject::Rejection;

use super::filters::SingleQuestion;
use libmemex::llm::{
    openai::{OpenAIClient, OpenAIModel},
    prompter,
};

pub async fn handle_extract(
    llm: OpenAIClient,
    request: SingleQuestion,
) -> Result<impl warp::Reply, Rejection> {
    let _time = std::time::Instant::now();

    // Build prompt
    let prompt = if let Some(schema) = &request.json_schema {
        JSONSchema::options()
            .compile(schema)
            .map_err(|err| ServerError::ClientRequestError(err.to_string()))?;
        prompter::json_schema_extraction(&request.text, &request.query, &schema.to_string())
    } else {
        prompter::quick_question(&request.query)
    };

    let response = llm
        .chat_completion(&OpenAIModel::GPT35, &prompt)
        .await
        .map_err(|err| ServerError::Other(err.to_string()))?;

    Ok(warp::reply::json(&json!({ "response": response })))
}
