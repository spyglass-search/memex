use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use tokio::sync::Mutex;
use url::Url;

use self::local::HnswStore;

pub mod local;
pub mod opensearch;
pub mod qdrant;

#[derive(Debug, Clone)]
pub struct VectorData {
    pub doc_id: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("File IO error: {0}")]
    FileIOError(#[from] std::io::Error),
    #[error("Unable to insert vector: {0}")]
    InsertionError(String),
    #[error("Unable to deserialize: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Unable to save db file: {0}")]
    SaveError(String),
    #[error("Unsupported vector db: {0}")]
    Unsupported(String),
}

pub type VectorSearchResult = (String, f32);

pub trait VectorStore {
    /// Delete a single document from the vector store.
    fn delete(&mut self, doc_id: &str) -> Result<(), VectorStoreError>;
    /// Delete ALL documents from the vector store.
    fn delete_all(&mut self) -> Result<(), VectorStoreError>;

    fn insert(&mut self, doc_id: &str, vec: &[f32]) -> Result<(), VectorStoreError>;
    fn search(
        &self,
        vec: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, VectorStoreError>;
}

#[derive(Clone)]
pub struct VectorStorage {
    pub client: Arc<Mutex<dyn VectorStore + Send>>,
}

impl VectorStorage {
    pub async fn add_vectors(&self, points: Vec<VectorData>) -> Result<(), VectorStoreError> {
        let mut client = self.client.lock().await;
        for point in points {
            if let Err(err) = client.insert(&point.doc_id, &point.vector) {
                return Err(VectorStoreError::InsertionError(err.to_string()));
            }
        }

        Ok(())
    }

    pub async fn delete_collection(&self) -> Result<(), VectorStoreError> {
        let mut client = self.client.lock().await;
        client.delete_all()
    }

    pub async fn search(
        &self,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, VectorStoreError> {
        let client = self.client.lock().await;
        client.search(query, limit)
    }
}

pub async fn get_vector_storage(
    uri: &str,
    collection: &str,
) -> Result<VectorStorage, VectorStoreError> {
    let parsed_uri = match Url::parse(uri) {
        Ok(uri) => uri,
        Err(_) => return Err(VectorStoreError::Unsupported(uri.to_string())),
    };

    let scheme = parsed_uri.scheme();

    // Only support one right now
    let client = if scheme == "hnsw" {
        let storage: PathBuf = uri.strip_prefix("hnsw://").unwrap_or_default().into();
        // Collections are stored as folders
        let storage = storage.join(collection);
        if !storage.exists() {
            std::fs::create_dir_all(storage.clone())?;
        }

        if HnswStore::has_store(&storage) {
            HnswStore::load(&storage)?
        } else {
            HnswStore::new(&storage)
        }
    } else {
        return Err(VectorStoreError::Unsupported(uri.to_string()));
    };

    Ok(VectorStorage {
        client: Arc::new(Mutex::new(client)),
    })
}
