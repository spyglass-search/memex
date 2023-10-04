use serde::{Deserialize, Serialize};
use warp::Filter;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FetchRequest {
    /// Url to fetch
    pub url: Option<String>,
}

fn fetch_url() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("fetch")
        .and(warp::get())
        .and(warp::query::<FetchRequest>())
        .and_then(super::handlers::handle_fetch)
}

pub fn parse_file() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("fetch" / "parse")
        .and(warp::post())
        .and(warp::multipart::form().max_length(50_000_000))
        .and_then(super::handlers::handle_parse)
}

pub fn build() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    fetch_url().or(parse_file()).boxed()
}
