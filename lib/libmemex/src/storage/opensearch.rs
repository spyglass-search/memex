use super::{StoreResult, VectorData, VectorSearchResult, VectorStore, VectorStoreError};
use async_trait::async_trait;
use opensearch::{
    auth::Credentials,
    cert::CertificateValidation,
    http::transport::{SingleNodeConnectionPool, TransportBuilder},
    BulkOperation, BulkOperations, OpenSearch,
};
use serde_json::{json, Value};
use url::Url;

pub struct OpenSearchStore {
    pub client: OpenSearch,
    pub index_name: String,
}

#[derive(Default)]
pub struct OpenSearchConnectionConfig {
    pub credentials: Option<Credentials>,
    pub index: String,
    pub embedding_dimension: usize,
}

impl OpenSearchStore {
    pub async fn new(
        connect_url: &str,
        config: OpenSearchConnectionConfig,
    ) -> anyhow::Result<Self> {
        let client = connect(connect_url, config.credentials)?;
        // Make sure index is created
        create_index(&client, &config.index, config.embedding_dimension).await?;

        Ok(Self {
            client,
            index_name: config.index.clone(),
        })
    }

    pub async fn delete_index(&self) -> anyhow::Result<()> {
        self.client
            .indices()
            .delete(opensearch::indices::IndicesDeleteParts::Index(&[
                &self.index_name
            ]))
            .send()
            .await?;
        Ok(())
    }
}

#[async_trait]
impl VectorStore for OpenSearchStore {
    async fn delete(&mut self, _doc_id: &str) -> StoreResult<()> {
        Ok(())
    }

    async fn delete_all(&mut self) -> StoreResult<()> {
        self.delete_index()
            .await
            .map_err(|err| VectorStoreError::DeleteError(err.to_string()))?;
        Ok(())
    }

    async fn bulk_insert(&mut self, data: &[VectorData]) -> StoreResult<()> {
        let mut ops = BulkOperations::new();
        for item in data {
            ops.push(BulkOperation::index(json!({
                "doc_id": item.doc_id,
                "text": item.text.to_string(),
                "embedding": item.vector
            })))
            .map_err(|err| VectorStoreError::InsertionError(err.to_string()))?;
        }

        self.client
            .bulk(opensearch::BulkParts::Index(&self.index_name))
            .body(vec![ops])
            .send()
            .await
            .map_err(|err| VectorStoreError::InsertionError(err.to_string()))?;

        Ok(())
    }

    async fn insert(&mut self, data: &VectorData) -> StoreResult<()> {
        self.bulk_insert(&[data.to_owned()]).await
    }

    async fn search(&self, vec: &[f32], limit: usize) -> StoreResult<Vec<VectorSearchResult>> {
        let response = self
            .client
            .search(opensearch::SearchParts::Index(&[&self.index_name]))
            .body(serde_json::json!({
                    "size": limit,
                    "query": {
                        "knn": {
                            "embedding": {
                                "vector": vec,
                                "k": limit
                            }
                        },
                    }
                }
            ))
            .send()
            .await
            .map_err(|err| VectorStoreError::SearchError(err.to_string()))?;

        let response_body = response
            .json::<Value>()
            .await.map_err(|err| VectorStoreError::SearchError(err.to_string()))?;

        let mut results = Vec::new();
        for hit in response_body["hits"]["hits"].as_array().unwrap() {
            println!("{:?} - {:?}", hit["_score"], hit["_source"]);
            results.push((hit["_source"]["doc_id"].to_string(), hit["_score"].as_f64().unwrap_or_default() as f32))
        }

        Ok(results)
    }
}

pub async fn create_index(
    client: &OpenSearch,
    name: &str,
    embedding_dim: usize,
) -> anyhow::Result<()> {
    client
        .indices()
        .create(opensearch::indices::IndicesCreateParts::Index(name))
        .body(serde_json::json!({
            "settings": {
                "index.knn": true
            },
            "mappings": {
                "properties": {
                    "embedding": {
                        "type": "knn_vector",
                        "dimension": embedding_dim
                    }
                }
            }
        }))
        .send()
        .await?;

    Ok(())
}

/// Utility method to connect to
pub fn connect(url: &str, credentials: Option<Credentials>) -> anyhow::Result<OpenSearch> {
    let url = Url::parse(url)?;

    let mut transport = TransportBuilder::new(SingleNodeConnectionPool::new(url.clone()));
    transport = transport.cert_validation(CertificateValidation::None);

    if let Some(creds) = credentials {
        transport = transport.auth(creds);
    } else {
        transport = transport.auth(opensearch::auth::Credentials::Basic(
            url.username().to_string(),
            url.password().map_or("".to_string(), |x| x.to_string()),
        ));
    }

    Ok(OpenSearch::new(transport.build()?))
}

#[cfg(test)]
mod test {
    use super::OpenSearchConnectionConfig;
    use crate::storage::{VectorStore, VectorData};
    use serde_json::Value;

    const OPENSEARCH_URL: &str = "https://admin:admin@localhost:9200";

    #[tokio::test]
    async fn test_initialize() {
        let config = OpenSearchConnectionConfig {
            index: "test".to_string(),
            embedding_dimension: 3,
            ..Default::default()
        };
        let mut store = super::OpenSearchStore::new(OPENSEARCH_URL, config)
            .await
            .expect("Unable to create client");

        let info: Value = store
            .client
            .info()
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        println!(
            "INFO - {}: {}",
            info["version"]["distribution"].as_str().unwrap(),
            info["version"]["number"].as_str().unwrap()
        );

        store.delete_all().await.expect("Unable to delete index");
    }

    #[tokio::test]
    async fn test_search() {
        let index_name = "movies";
        let config = OpenSearchConnectionConfig {
            index: index_name.to_string(),
            embedding_dimension: 3,
            ..Default::default()
        };
        let mut store = super::OpenSearchStore::new(OPENSEARCH_URL, config)
            .await
            .expect("Unable to create client");

        store.bulk_insert(&vec![
            VectorData {
                doc_id: "test-one".into(),
                text: "".into(),
                segment_id: 0,
                vector: vec![1.5, 2.5, 3.5]
            },
            VectorData {
                doc_id: "test-two".into(),
                text: "".into(),
                segment_id: 0,
                vector: vec![2.5, 3.5, 4.5]
            },
            VectorData {
                doc_id: "test-three".into(),
                text: "".into(),
                segment_id: 0,
                vector: vec![2.5, 3.5, 5.5]
            },
            VectorData {
                doc_id: "test-four".into(),
                text: "".into(),
                segment_id: 0,
                vector: vec![2.5, 0.5, 5.5]
            }
        ]).await.unwrap();

        // wait a little bit for indexing to finish.
        let _ = tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        let results = store.search(&vec![2.0, 3.0, 4.0], 2).await.unwrap();
        assert_eq!(results.len(), 2);

        for result in results {
            println!("{:?} - {:?}", result.0, result.1);
        }

        store.delete_all().await.expect("Unable to delete index");
    }
}
