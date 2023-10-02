use std::time::Duration;

use chrono::Utc;
use libmemex::db;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
pub struct DocumentSegment {
    pub _id: String,
    pub document_id: String,
    pub segment: i64,
    pub content: String,
    pub score: f32,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub results: Vec<DocumentSegment>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResult {
    task_id: i64,
    collection: String,
    status: String,
    created_at: chrono::DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
}

impl From<db::queue::Model> for TaskResult {
    fn from(value: db::queue::Model) -> Self {
        TaskResult {
            task_id: value.id,
            collection: value.collection,
            status: value.status.to_string(),
            created_at: value.created_at,
            result: value.task_output,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiResponseStatus {
    Ok,
    Error,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    /// Execution time in seconds
    pub time: f32,
    pub status: ApiResponseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
}

impl<ErrorMessage> ApiResponse<ErrorMessage> {
    pub fn error(error: ErrorMessage) -> ApiResponse<ErrorMessage> {
        ApiResponse {
            time: 0.0,
            status: ApiResponseStatus::Error,
            result: Some(error),
        }
    }
}

impl<T> ApiResponse<T> {
    pub fn success(elapsed: &Duration, result: Option<T>) -> Self {
        Self {
            time: elapsed.as_secs_f32(),
            status: ApiResponseStatus::Ok,
            result,
        }
    }
}
