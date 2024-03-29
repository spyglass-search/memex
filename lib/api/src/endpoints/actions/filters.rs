use std::sync::Arc;

use crate::{endpoints::json_body, with_db, with_llm};
use libmemex::llm::LLM;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use warp::Filter;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AskRequest {
    /// Input text
    pub text: String,
    /// User request
    pub query: String,
    /// Output schema (if provided).
    pub json_schema: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SummarizeRequest {
    /// Input text to summarize
    pub text: String,
}

fn extract(
    llm: &Arc<Box<dyn LLM>>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("action" / "ask")
        .and(warp::post())
        .and(with_llm(llm.clone()))
        .and(json_body::<AskRequest>(1024 * 1024 * 10))
        .and_then(super::handlers::handle_extract)
}

fn summarize(
    db: &DatabaseConnection,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("action" / "summarize" / "task")
        .and(warp::post())
        .and(with_db(db.clone()))
        .and(json_body::<SummarizeRequest>(1024 * 1024 * 10))
        .and_then(super::handlers::handle_summarize)
}

pub fn build(
    llm: &Arc<Box<dyn LLM>>,
    db: &DatabaseConnection,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    extract(llm).or(summarize(db))
}
