use crate::{
    schema::{self, Document},
    ServerError,
};
use embedder::{ModelConfig, SentenceEmbedder};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use shared::{
    db::{document, queue},
    vector::get_vector_storage,
};
use uuid::Uuid;

pub async fn handle_add_document(
    cname: String,
    req: schema::InsertDocumentRequest,
    db: DatabaseConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Add to job queue
    let job_id = Uuid::new_v4().to_string();
    if let Err(err) = queue::enqueue(&db, &job_id, &req.content).await {
        return Err(warp::reject::custom(ServerError::Other(err.to_string())));
    }

    // Create an UUID for this document & add to queue
    Ok(warp::reply::json(
        &serde_json::json!({ "id": job_id, "status": "Queued", "collection": cname }),
    ))
}

pub async fn handle_search_docs(
    cname: String,
    req: schema::SearchDocsRequest,
    db: DatabaseConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (_handle, embedder) = SentenceEmbedder::spawn(&ModelConfig::default());

    let vector_uri = std::env::var("VECTOR_CONNECTION").expect("VECTOR_CONNECTION env var not set");
    let client = match get_vector_storage(&vector_uri).await {
        Ok(client) => client,
        Err(err) => {
            return Err(warp::reject::custom(ServerError::Other(format!(
                "Unable to connect to vector db: {err}"
            ))))
        }
    };

    let vector = match embedder.encode_single(req.query).await {
        Ok(Some(vector)) => vector,
        _ => {
            return Err(warp::reject::custom(ServerError::Other(
                "Invalid query".into(),
            )))
        }
    };

    let search_result = match client.search(&vector.vector, req.limit as usize).await {
        Ok(result) => result,
        Err(err) => return Err(warp::reject::custom(ServerError::Other(err.to_string()))),
    };

    // Grab the document data for each search result
    let mut results = Vec::new();
    for (doc_id, score) in search_result.iter() {
        if let Ok(Some(doc)) = document::Entity::find()
            .filter(document::Column::DocumentId.eq(doc_id))
            .one(&db)
            .await
        {
            results.push(Document {
                id: doc_id.to_string(),
                segment: doc.segment,
                content: doc.content,
                score: *score,
            });
        }
    }

    let result = schema::SearchResult { results };
    Ok(warp::reply::json(&result))
}
