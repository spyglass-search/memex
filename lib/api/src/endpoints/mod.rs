use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use warp::Filter;

mod actions;
mod collections;
mod tasks;

const LIMIT_1_MB: u64 = 1000 * 1024;
const LIMIT_10_MB: u64 = 10 * LIMIT_1_MB;

pub fn json_body<T: std::marker::Send + DeserializeOwned>(
    limit: u64,
) -> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(limit).and(warp::body::json())
}

pub fn build(
    db: &DatabaseConnection,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    actions::filters::build()
        .or(collections::filters::build(db))
        .or(tasks::filters::build(db))
}
