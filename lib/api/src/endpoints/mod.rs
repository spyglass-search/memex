use libmemex::llm::openai::OpenAIClient;
use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use warp::Filter;

mod actions;
mod collections;
mod fetch;
mod tasks;

const LIMIT_1_MB: u64 = 1000 * 1024;
const LIMIT_10_MB: u64 = 10 * LIMIT_1_MB;

#[cfg(not(debug_assertions))]
pub const UPLOAD_DATA_DIR: &str = "/tmp";
#[cfg(debug_assertions)]
pub const UPLOAD_DATA_DIR: &str = "./uploads";

pub fn json_body<T: std::marker::Send + DeserializeOwned>(
    limit: u64,
) -> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(limit).and(warp::body::json())
}

pub fn build(
    db: &DatabaseConnection,
    llm: &OpenAIClient,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    actions::filters::build(llm, db)
        .or(collections::filters::build(db))
        .or(fetch::filters::build())
        .or(tasks::filters::build(db))
}
