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
    pub id: String,
    pub segment: i64,
    pub content: String,
    pub score: f32,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub results: Vec<Document>,
}
