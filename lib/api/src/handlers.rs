use crate::{
    schema::{self, Document, TaskResult},
    ServerError,
};
use libmemex::{
    db::{document, queue},
    embedding::{ModelConfig, SentenceEmbedder},
    storage::get_vector_storage,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

pub async fn handle_add_document(
    collection: String,
    req: schema::InsertDocumentRequest,
    db: DatabaseConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Add to job queue
    let task = match queue::enqueue(&db, &collection, &req.content).await {
        Ok(model) => model,
        Err(err) => return Err(warp::reject::custom(ServerError::DatabaseError(err))),
    };

    // Create an UUID for this document & add to queue
    Ok(warp::reply::json(&schema::TaskResult::from(task)))
}

pub async fn handle_delete_collection(
    collection: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let vector_uri = std::env::var("VECTOR_CONNECTION").expect("VECTOR_CONNECTION env var not set");
    let client = match get_vector_storage(&vector_uri, &collection).await {
        Ok(client) => client,
        Err(err) => {
            return Err(warp::reject::custom(ServerError::Other(format!(
                "Unable to connect to vector db: {err}"
            ))))
        }
    };

    match client.delete_collection().await {
        Ok(()) => Ok(warp::reply::with_status(
            warp::reply(),
            warp::http::StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(ServerError::Other(format!(
            "Unable to remove collection {collection}: {err}"
        )))),
    }
}

pub async fn handle_search_docs(
    collection: String,
    req: schema::SearchDocsRequest,
    db: DatabaseConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (_handle, embedder) = SentenceEmbedder::spawn(&ModelConfig::default());

    let vector_uri = std::env::var("VECTOR_CONNECTION").expect("VECTOR_CONNECTION env var not set");
    let client = match get_vector_storage(&vector_uri, &collection).await {
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
    for (internal_id, score) in search_result.iter() {
        if let Ok(Some(doc)) = document::Entity::find()
            .filter(document::Column::DocumentId.eq(internal_id))
            .one(&db)
            .await
        {
            results.push(Document {
                _id: internal_id.to_string(),
                task_id: doc.task_id,
                segment: doc.segment,
                content: doc.content,
                score: *score,
            });
        }
    }

    let result = schema::SearchResult { results };
    Ok(warp::reply::json(&result))
}

pub async fn handle_check_task(
    task_id: i64,
    db: DatabaseConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    let result = match queue::Entity::find_by_id(task_id).one(&db).await {
        Ok(res) => res,
        Err(err) => return Err(warp::reject::custom(ServerError::DatabaseError(err))),
    };

    match result {
        Some(result) => Ok(warp::reply::json(&TaskResult::from(result))),
        None => Err(warp::reject::not_found()),
    }
}
