use serde::Serialize;

/// An API error serializable to JSON.
#[derive(Serialize)]
pub struct ErrorMessage {
    pub code: u16,
    pub message: String,
}
