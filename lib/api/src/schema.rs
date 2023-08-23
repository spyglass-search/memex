use chrono::Utc;
use libmemex::db;
use serde::{Deserialize, Serialize};

/// An API error serializable to JSON.
#[derive(Serialize)]
pub struct ErrorMessage {
    pub code: u16,
    pub message: String,
}

#[derive(Deserialize)]
pub struct InsertDocumentRequest {
    pub content: String,
}

#[derive(Deserialize, Default)]
pub struct SearchDocsRequest {
    pub query: String,
    #[serde(default = "SearchDocsRequest::default_limit")]
    pub limit: u64,
}

impl SearchDocsRequest {
    fn default_limit() -> u64 {
        10
    }
}

#[derive(Serialize)]
pub struct Document {
    pub _id: String,
    pub task_id: String,
    pub segment: i64,
    pub content: String,
    pub score: f32,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub results: Vec<Document>,
}

#[derive(Serialize)]
pub struct TaskResult {
    task_id: i64,
    collection: String,
    status: String,
    created_at: chrono::DateTime<Utc>,
}

impl From<db::queue::Model> for TaskResult {
    fn from(value: db::queue::Model) -> Self {
        TaskResult {
            task_id: value.id,
            collection: value.collection,
            status: value.status.to_string(),
            created_at: value.created_at,
        }
    }
}
