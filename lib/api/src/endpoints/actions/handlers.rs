use crate::{
    schema::{ApiResponse, TaskResult},
    ServerError,
};
use jsonschema::JSONSchema;
use sea_orm::DatabaseConnection;
use warp::reject::Rejection;

use super::filters;
use libmemex::{
    db::queue,
    llm::{
        openai::{truncate_text, OpenAIClient},
        prompter,
    },
};

pub async fn handle_extract(
    llm: OpenAIClient,
    request: filters::AskRequest,
) -> Result<impl warp::Reply, Rejection> {
    let time = std::time::Instant::now();

    let (content, model) = truncate_text(&request.text);

    // Build prompt
    let prompt = if let Some(schema) = &request.json_schema {
        JSONSchema::options()
            .compile(schema)
            .map_err(|err| ServerError::ClientRequestError(err.to_string()))?;
        prompter::json_schema_extraction(&content, &request.query, &schema.to_string())
    } else {
        prompter::quick_question(&request.query)
    };

    let response = llm
        .chat_completion(&model, &prompt)
        .await
        .map_err(|err| ServerError::Other(err.to_string()))?;

    let val = serde_json::from_str::<serde_json::Value>(&response)
        .map_err(|err| ServerError::Other(err.to_string()))?;

    Ok(warp::reply::json(&ApiResponse::success(
        time.elapsed(),
        Some(serde_json::json!({ "jsonResponse": val })),
    )))
}

pub async fn handle_summarize(
    db: DatabaseConnection,
    request: filters::SummarizeRequest,
) -> Result<impl warp::Reply, Rejection> {
    let time = std::time::Instant::now();
    // Add to job queue
    let task = match queue::enqueue(&db, "tasks", &request.text, queue::TaskType::Summarize).await {
        Ok(model) => model,
        Err(err) => return Err(warp::reject::custom(ServerError::DatabaseError(err))),
    };

    let result = TaskResult::from(task);
    Ok(warp::reply::json(&ApiResponse::success(
        time.elapsed(),
        Some(result),
    )))
}
