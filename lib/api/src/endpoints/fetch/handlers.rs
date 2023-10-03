use crate::{schema::ApiResponse, ServerError};
use warp::reject::Rejection;

use super::filters;

pub async fn handle_fetch(query: filters::FetchRequest) -> Result<impl warp::Reply, Rejection> {
    let time = std::time::Instant::now();

    if let Some(url) = query.url {
        let content = reqwest::get(url)
            .await
            .map_err(|err| ServerError::Other(err.to_string()))?
            .text()
            .await
            .map_err(|err| ServerError::Other(err.to_string()))?;

        Ok(warp::reply::json(&ApiResponse::success(
            &time.elapsed(),
            Some(serde_json::json!({ "content": content })),
        )))
    } else {
        Err(warp::reject())
    }
}
