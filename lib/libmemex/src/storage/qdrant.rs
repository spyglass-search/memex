use qdrant_client::{
    prelude::*,
    qdrant::{vectors_config::Config, VectorParams, VectorsConfig},
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn connect_to_qdrant(host: &str, collection: &str) -> Arc<Mutex<QdrantClient>> {
    let config = QdrantClientConfig::from_url(host);
    let client = match qdrant_client::client::QdrantClient::new(Some(config)) {
        Ok(client) => client,
        Err(err) => panic!("Unable to connect to vectordb: {err}"),
    };

    if client
        .collection_info(collection.to_string())
        .await
        .is_err()
    {
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

    Arc::new(Mutex::new(client))
}
