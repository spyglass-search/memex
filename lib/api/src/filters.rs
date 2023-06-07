use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use uuid::Uuid;
use warp::Filter;

use crate::{schema::InsertDocumentRequest, with_db, ServerError};
use embedder::{ModelConfig, SentenceEmbedder};

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
    warp::path!("collection")
        .and(warp::post())
        .and(json_body::<InsertDocumentRequest>(LIMIT_10_MB))
        .and(with_db(db.clone()))
        .and_then(handle_add_document)
}

async fn handle_add_document(
    req: InsertDocumentRequest,
    _db: DatabaseConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    let model_config = ModelConfig::default();
    let (_handle, embedder) = SentenceEmbedder::spawn(&model_config);

    let embeddings = match embedder.encode(req.content).await {
        Ok(res) => res,
        Err(err) => return Err(warp::reject::custom(ServerError::Other(err.to_string()))),
    };

    for embedding in embeddings {
        println!("embedding: {}", embedding.content);
    }

    // Create an UUID for this document & add to queue
    let doc_id = Uuid::new_v4();
    Ok(warp::reply::json(&serde_json::json!({ "id": doc_id })))
}

pub fn build(
    db: &DatabaseConnection,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    add_document(db)
}
