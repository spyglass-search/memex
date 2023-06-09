use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use warp::Filter;

use crate::handlers;
use crate::{schema, with_db};

const LIMIT_1_MB: u64 = 1000 * 1024;
const LIMIT_10_MB: u64 = 10 * LIMIT_1_MB;

pub fn json_body<T: std::marker::Send + DeserializeOwned>(
    limit: u64,
) -> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(limit).and(warp::body::json())
}

pub fn add_document(
    db: &DatabaseConnection,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("collections" / String)
        .and(warp::post())
        .and(json_body::<schema::InsertDocumentRequest>(LIMIT_10_MB))
        .and(with_db(db.clone()))
        .and_then(handlers::handle_add_document)
}

pub fn search_docs(
    db: &DatabaseConnection,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("collections" / String / "search")
        .and(warp::get())
        .and(json_body::<schema::SearchDocsRequest>(LIMIT_1_MB))
        .and(with_db(db.clone()))
        .and_then(handlers::handle_search_docs)
}

pub fn build(
    db: &DatabaseConnection,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    add_document(db).or(search_docs(db))
}
