use async_trait::async_trait;
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use tokio::sync::Mutex;
use url::Url;

use self::{
    local::HnswStore,
    opensearch::{OpenSearchConnectionConfig, OpenSearchStore},
};

pub mod local;
pub mod opensearch;
pub mod qdrant;

#[derive(Debug, Clone)]
pub struct VectorData {
    /// Internal ID used to identify this vector/segment
    pub _id: String,
    /// Document this segment is originally from
    pub document_id: String,
    /// Context represented by this vector
    pub text: String,
    /// Sentence embedding for this text content
    pub vector: Vec<f32>,
    /// Segment number of the content
    pub segment_id: usize,
}

#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("Unable to connect: {0}")]
    ConnectionError(String),
    #[error("DeleteError: {0}")]
    DeleteError(String),
    #[error("File IO error: {0}")]
    FileIOError(#[from] std::io::Error),
    #[error("Unable to insert vector: {0}")]
    InsertionError(String),
    #[error("Unable to search: {0}")]
    SearchError(String),
    #[error("Unable to deserialize: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Unable to save db file: {0}")]
    SaveError(String),
    #[error("Unsupported vector db: {0}")]
    Unsupported(String),
}

// doc_id & score
pub type VectorSearchResult = (String, f32);
pub type StoreResult<T> = Result<T, VectorStoreError>;

#[async_trait]
pub trait VectorStore {
    /// Delete a single document from the vector store.
    /// If a segment is given, that particular segment is deleted.
    async fn delete(&mut self, id: &str) -> StoreResult<()>;
    /// Delete ALL documents from the vector store.
    async fn delete_all(&mut self) -> StoreResult<()>;
    /// Bulk insert many documents at a time
    async fn bulk_insert(&mut self, data: &[VectorData]) -> StoreResult<()>;
    /// Insert a single document
    async fn insert(&mut self, data: &VectorData) -> StoreResult<()>;
    async fn search(&self, vec: &[f32], limit: usize) -> StoreResult<Vec<VectorSearchResult>>;
}

#[derive(Clone)]
pub struct VectorStorage {
    pub client: Arc<Mutex<dyn VectorStore + Send + Sync>>,
}

impl VectorStorage {
    pub async fn add_vectors(&self, points: Vec<VectorData>) -> Result<(), VectorStoreError> {
        let mut client = self.client.lock().await;
        client.bulk_insert(&points).await?;
        Ok(())
    }

    pub async fn delete_collection(&self) -> Result<(), VectorStoreError> {
        let mut client = self.client.lock().await;
        client.delete_all().await
    }

    pub async fn search(
        &self,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, VectorStoreError> {
        let client = self.client.lock().await;
        client.search(query, limit).await
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
    let client: Arc<Mutex<dyn VectorStore + Send + Sync>> = if scheme == "hnsw" {
        let storage: PathBuf = uri.strip_prefix("hnsw://").unwrap_or_default().into();
        // Collections are stored as folders
        let storage = storage.join(collection);
        if !storage.exists() {
            std::fs::create_dir_all(storage.clone())?;
        }

        let store = if HnswStore::has_store(&storage) {
            HnswStore::load(&storage)?
        } else {
            HnswStore::new(&storage)
        };

        Arc::new(Mutex::new(store))
    } else if scheme == "opensearch+https" {
        let connect_url = uri.strip_prefix("opensearch+").unwrap_or_default();
        let config = OpenSearchConnectionConfig {
            index: collection.to_string(),
            embedding_dimension: 384,
            ..Default::default()
        };

        let store = OpenSearchStore::new(connect_url, config)
            .await
            .map_err(|x| VectorStoreError::ConnectionError(x.to_string()))?;
        Arc::new(Mutex::new(store))
    } else {
        return Err(VectorStoreError::Unsupported(uri.to_string()));
    };

    Ok(VectorStorage { client })
}
