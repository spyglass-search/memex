use crate::endpoints::json_body;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use warp::Filter;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SingleQuestion {
    pub text: String,
    pub query: String,
    pub json_sample: Option<Value>,
    pub json_schema: Option<Value>,
}

fn extract() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("action" / "ask")
        .and(warp::post())
        .and(json_body::<SingleQuestion>(1024 * 16))
        .and_then(super::handlers::handle_extract)
}

pub fn build() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    extract()
}
