use crate::{
    schema::{self, Document},
    ServerError,
};
use embedder::{ModelConfig, SentenceEmbedder};
use qdrant_client::{
    prelude::*,
    qdrant::{value::Kind, with_payload_selector::SelectorOptions, WithPayloadSelector},
};
use sea_orm::DatabaseConnection;
use shared::db::queue;
use uuid::Uuid;

pub async fn handle_add_document(
    req: schema::InsertDocumentRequest,
    db: DatabaseConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Add to job queue
    let job_id = Uuid::new_v4().to_string();
    if let Err(err) = queue::enqueue(&db, &job_id, &req.content).await {
        return Err(warp::reject::custom(ServerError::Other(err.to_string())));
    }

    // Create an UUID for this document & add to queue
    Ok(warp::reply::json(&serde_json::json!({ "id": job_id })))
}

pub async fn handle_search_docs(
    req: schema::SearchDocsRequest,
    _db: DatabaseConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (_handle, embedder) = SentenceEmbedder::spawn(&ModelConfig::default());

    let collection = std::env::var("QDRANT_COLLECTION").expect("QDRANT_COLLECTION not set");

    let qdrant_host = std::env::var("QDRANT_ENDPOINT").expect("QDRANT_ENDPOINT env var not set");

    let config = QdrantClientConfig::from_url(&qdrant_host);
    let client = QdrantClient::new(Some(config)).expect("Unable to create qdrant");

    let vector = match embedder.encode(req.query).await {
        Ok(vector) => vector,
        Err(err) => return Err(warp::reject::custom(ServerError::Other(err.to_string()))),
    };
    let vector = vector.get(0).unwrap();

    let search_result = match client
        .search_points(&SearchPoints {
            collection_name: collection,
            vector: vector.vector.to_owned(),
            filter: None,
            limit: req.limit,
            with_vectors: None,
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(SelectorOptions::Enable(true)),
            }),
            params: None,
            score_threshold: None,
            offset: None,
            ..Default::default()
        })
        .await
    {
        Ok(result) => result,
        Err(err) => return Err(warp::reject::custom(ServerError::Other(err.to_string()))),
    };

    let results = search_result
        .result
        .iter()
        .map(|doc| {
            let id = doc
                .id
                .as_ref()
                .and_then(|x| x.point_id_options.clone())
                .map(|x| match x {
                    point_id::PointIdOptions::Num(id) => id.to_string(),
                    point_id::PointIdOptions::Uuid(id) => id,
                })
                .unwrap_or(String::from("UNK"));

            let segment_id = if let Some(sid) = doc
                .payload
                .get("segment_id")
                .and_then(|x| x.kind.to_owned())
            {
                match sid {
                    Kind::DoubleValue(val) => val.to_string(),
                    Kind::IntegerValue(val) => val.to_string(),
                    Kind::StringValue(val) => val,
                    _ => String::from("Unk"),
                }
            } else {
                String::from("Unk")
            };

            let content = if let Some(Kind::StringValue(content)) =
                doc.payload.get("content").and_then(|x| x.kind.to_owned())
            {
                content
            } else {
                "".into()
            };

            Document {
                id,
                segment_id,
                content,
                score: doc.score,
            }
        })
        .collect::<Vec<schema::Document>>();

    let result = schema::SearchResult { results };
    Ok(warp::reply::json(&result))
}
