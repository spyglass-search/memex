use sea_orm::DatabaseConnection;
use warp::Filter;

use crate::endpoints::{json_body, LIMIT_10_MB, LIMIT_1_MB};
use crate::handlers;
use crate::{schema, with_db};

fn add_document(
    db: &DatabaseConnection,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("collections" / String)
        .and(warp::post())
        .and(json_body::<schema::InsertDocumentRequest>(LIMIT_10_MB))
        .and(with_db(db.clone()))
        .and_then(handlers::handle_add_document)
}

fn delete_collection() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    warp::path!("collections" / String)
        .and(warp::delete())
        .and_then(handlers::handle_delete_collection)
}

fn search_docs(
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
    add_document(db)
        .or(delete_collection())
        .or(search_docs(db))
        .boxed()
}
