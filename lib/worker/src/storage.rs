use qdrant_client::{
    prelude::*,
    qdrant::{vectors_config::Config, VectorParams, VectorsConfig},
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct VectorStorage {
    pub collection: String,
    pub client: Arc<Mutex<QdrantClient>>,
}

impl VectorStorage {
    pub async fn add_vectors(&self, points: Vec<PointStruct>) -> anyhow::Result<()> {
        let client = self.client.lock().await;
        client
            .upsert_points_blocking(self.collection.clone(), points, None)
            .await?;

        Ok(())
    }
}

pub async fn get_or_create_vector_storage(collection: &str) -> VectorStorage {
    let qdrant_host = std::env::var("QDRANT_ENDPOINT").expect("QDRANT_ENDPOINT env var not set");

    let config = QdrantClientConfig::from_url(&qdrant_host);
    let client = match qdrant_client::client::QdrantClient::new(Some(config)) {
        Ok(client) => client,
        Err(err) => panic!("Unable to connect to vectordb: {err}"),
    };

    if client.collection_info(collection.to_string()).await.is_err() {
        if let Err(err) = client
            .create_collection(&CreateCollection {
                collection_name: collection.to_string(),
                on_disk_payload: Some(true),
                vectors_config: Some(VectorsConfig {
                    config: Some(Config::Params(VectorParams {
                        size: 384,
                        distance: Distance::Cosine.into(),
                        hnsw_config: None,
                        quantization_config: None,
                        on_disk: Some(true),
                    })),
                }),
                ..Default::default()
            })
            .await
        {
            log::error!("Unable to create collection: {err}");
        } else {
            log::info!("Creating not existent collection: {collection}");
        }
    }

    VectorStorage {
        collection: collection.to_string(),
        client: Arc::new(Mutex::new(client)),
    }
}
