use dotenv_codegen::dotenv;
use libmemex::{db::create_connection_by_uri, llm::openai::OpenAIClient};
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::{convert::Infallible, net::Ipv4Addr};
use thiserror::Error;
use warp::{hyper::StatusCode, reject::Reject, Filter, Rejection, Reply};

pub mod endpoints;
pub mod schema;
use schema::ErrorMessage;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Client request error: {0}")]
    ClientRequestError(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sea_orm::DbErr),
    #[error("Server error: {0}")]
    Other(String),
}

impl Reject for ServerError {}

// Handle custom errors/rejections
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message: String;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "NOT_FOUND".into();
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        // We can handle a specific error, here METHOD_NOT_ALLOWED,
        // and render it however we want
        code = StatusCode::METHOD_NOT_ALLOWED;
        message = "METHOD_NOT_ALLOWED".into();
    } else {
        // We should have expected this... Just log and say its a 500
        eprintln!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "UNHANDLED_REJECTION".into();
    }

    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message,
    });

    Ok(warp::reply::with_status(json, code))
}

// GET /health
pub fn health_check() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    let version = dotenv!("GIT_HASH");
    warp::path("health")
        .and(warp::get())
        .map(move || warp::reply::json(&json!({ "version": version })))
}

pub async fn start(host: Ipv4Addr, port: u16, db_uri: String) {
    log::info!("starting api server @ {}:{}", host, port);

    // Attempt to connect to db
    let db_connection = create_connection_by_uri(&db_uri, true)
        .await
        .unwrap_or_else(|err| panic!("Unable to connect to database: {} - {err}", db_uri));

    let llm_client =
        OpenAIClient::new(&std::env::var("OPENAI_API_KEY").expect("OpenAI API key not set"));
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE"])
        .allow_headers(["Authorization", "Content-Type"]);

    let api = warp::path("api")
        .and(endpoints::build(&db_connection, &llm_client))
        .with(warp::trace::request());

    let filters = health_check().or(api).with(cors).recover(handle_rejection);

    let (_addr, handle) =
        warp::serve(filters).bind_with_graceful_shutdown((host, port), async move {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen to shutdown signal");
        });

    handle.await;
}

/// Filter that will clone the db for use in handlers
pub fn with_db(
    db: DatabaseConnection,
) -> impl Filter<Extract = (DatabaseConnection,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}

pub fn with_llm(
    llm: OpenAIClient,
) -> impl Filter<Extract = (OpenAIClient,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || llm.clone())
}
