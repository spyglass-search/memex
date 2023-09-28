use crate::{endpoints::json_body, with_llm};
use libmemex::llm::openai::OpenAIClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use warp::Filter;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SingleQuestion {
    /// Input text
    pub text: String,
    /// User request
    pub query: String,
    /// Output schema (if provided).
    pub json_schema: Option<Value>,
}

fn extract(
    llm: &OpenAIClient,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("action" / "ask")
        .and(warp::post())
        .and(with_llm(llm.clone()))
        .and(json_body::<SingleQuestion>(1024 * 16))
        .and_then(super::handlers::handle_extract)
}

pub fn build(
    llm: &OpenAIClient,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    extract(llm)
}
