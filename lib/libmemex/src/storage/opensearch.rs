#[cfg(test)]
mod test {
    use dotenv::dotenv;
    use opensearch::{
        OpenSearch,
        cert::CertificateValidation,
        http::transport::{SingleNodeConnectionPool, TransportBuilder
    }};
    use serde_json::Value;
    use std::env;
    use url::Url;

    #[tokio::test]
    async fn test_opensearch() {
        dotenv().ok();
        let url = Url::parse(&env::var("OPENSEARCH_ENDPOINT").unwrap()).unwrap();
        let transport = TransportBuilder::new(SingleNodeConnectionPool::new(url))
            .cert_validation(CertificateValidation::None)
            .auth(opensearch::auth::Credentials::Basic("admin".to_string(), "admin".to_string()))
            .build()
            .unwrap();

        let client = OpenSearch::new(transport);
        let info: Value = client.info().send()
            .await.unwrap().json().await.unwrap();

        println!(
            "INFO - {}: {}",
            info["version"]["distribution"].as_str().unwrap(),
            info["version"]["number"].as_str().unwrap()
        );
    }
}