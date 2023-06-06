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
