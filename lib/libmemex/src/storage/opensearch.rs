use anyhow::Result;
use opensearch::{
    cert::CertificateValidation,
    http::transport::{SingleNodeConnectionPool, TransportBuilder},
    OpenSearch,
};
use url::Url;

// use super::VectorStore;

pub struct OpenSearchStore {
    pub client: OpenSearch,
    pub index_name: String,
}

impl OpenSearchStore {
    pub async fn new(index: &str, embedding_dim: usize) -> Result<Self> {
        let connect_url = std::env::var("OPENSEARCH_ENDPOINT").unwrap();
        let client = connect(&connect_url)?;
        // Make sure index is created
        create_index(&client, &index, embedding_dim).await?;

        Ok(Self {
            client,
            index_name: index.to_string(),
        })
    }

    pub async fn delete(&self) -> Result<()> {
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

pub async fn create_index(client: &OpenSearch, name: &str, embedding_dim: usize) -> Result<()> {
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

pub fn connect(url: &str) -> Result<OpenSearch> {
    let url = Url::parse(url)?;
    let transport = TransportBuilder::new(SingleNodeConnectionPool::new(url))
        .cert_validation(CertificateValidation::None)
        .auth(opensearch::auth::Credentials::Basic(
            "admin".to_string(),
            "admin".to_string(),
        ))
        .build()?;

    Ok(OpenSearch::new(transport))
}

#[cfg(test)]
mod test {
    use dotenv::dotenv;
    use opensearch::{BulkOperation, BulkOperations};
    use serde_json::{json, Value};

    #[tokio::test]
    async fn test_initialize() {
        dotenv().ok();

        let store = super::OpenSearchStore::new("test", 3)
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

        store.delete().await.expect("Unable to delete index");
    }

    #[tokio::test]
    async fn test_search() {
        dotenv().ok();

        let index_name = "movies";
        let store = super::OpenSearchStore::new(index_name, 3)
            .await
            .expect("Unable to create client");

        let mut ops = BulkOperations::new();
        ops.push(BulkOperation::index(
            json!({"price": 12.2, "embedding": [1.5, 2.5, 3.5] }),
        ))
        .unwrap();
        ops.push(BulkOperation::index(
            json!({"price": 7.1, "embedding": [2.5, 3.5, 4.5] }),
        ))
        .unwrap();
        ops.push(BulkOperation::index(
            json!({"price": 8.1, "embedding": [2.5, 3.5, 5.5] }),
        ))
        .unwrap();
        ops.push(BulkOperation::index(
            json!({"price": 9.1, "embedding": [2.5, 0.5, 5.5] }),
        )).unwrap();

        store
            .client
            .bulk(opensearch::BulkParts::Index(index_name))
            .body(vec![ops])
            .send()
            .await
            .expect("Unable to index");

        // wait a little bit for indexing to finish.
        let _ = tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        let response = store
            .client
            .search(opensearch::SearchParts::Index(&[index_name]))
            .body(serde_json::json!({
                    "size": 2,
                    "query": {
                        "knn": {
                            "embedding": {
                                "vector": [2.0, 3.0, 4.0],
                                "k": 10
                            }
                        },
                    }
                }
            ))
            .send()
            .await
            .expect("Unable to search");

        let response_body = response
            .json::<Value>()
            .await
            .expect("Unable to parse results");

        println!(
            "RESPONSE:\n{}\nRESULTS:",
            serde_json::to_string_pretty(&response_body).unwrap()
        );
        for hit in response_body["hits"]["hits"].as_array().unwrap() {
            println!("{:?} - {:?}", hit["_score"], hit["_source"]);
        }

        store.delete().await.expect("Unable to delete index");
    }
}
