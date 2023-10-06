use dotenv_codegen::dotenv;
use libmemex::{
    db::create_connection_by_uri,
    llm::{local::load_from_cfg, openai::OpenAIClient, LLM},
};
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::{convert::Infallible, net::Ipv4Addr, path::PathBuf, sync::Arc};
use thiserror::Error;
use warp::{hyper::StatusCode, reject::Reject, Filter, Rejection, Reply};

pub mod endpoints;
pub mod schema;
use schema::{ApiResponse, ErrorMessage};

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

pub struct ApiConfig {
    pub host: Ipv4Addr,
    pub port: u16,
    pub db_uri: String,
    pub open_ai_key: Option<String>,
    pub local_llm_config: Option<String>,
}

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

    let json = warp::reply::json(&ApiResponse::error(ErrorMessage {
        code: code.as_u16(),
        message,
    }));

    Ok(warp::reply::with_status(json, code))
}

// GET /health
pub fn health_check() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    let version = dotenv!("GIT_HASH");
    warp::path!("api" / "health")
        .and(warp::get())
        .map(move || warp::reply::json(&json!({ "version": version })))
}

pub async fn start(config: ApiConfig) {
    log::info!("starting api server @ {}:{}", config.host, config.port);

    log::info!("checking for upload directory...");
    let data_dir_path: PathBuf = endpoints::UPLOAD_DATA_DIR.into();
    if !data_dir_path.exists() {
        log::info!("creating upload directory @ {data_dir_path:?}");
        let _ = std::fs::create_dir_all(data_dir_path);
    }

    // Attempt to connect to db
    let db_connection = create_connection_by_uri(&config.db_uri, true)
        .await
        .unwrap_or_else(|err| panic!("Unable to connect to database: {} - {err}", config.db_uri));

    let llm_client: Arc<Box<dyn LLM>> = if let Some(openai_key) = config.open_ai_key {
        Arc::new(Box::new(OpenAIClient::new(&openai_key)))
    } else if let Some(llm_config_path) = config.local_llm_config {
        let llm = load_from_cfg(llm_config_path.into(), true)
            .await
            .expect("Unable to load local LLM");
        Arc::new(llm)
    } else {
        panic!("Please setup OPENAI_API_KEY or LOCAL_LLM_CONFIG");
    };

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE"])
        .allow_headers(["Authorization", "Content-Type"]);

    let api = warp::path("api")
        .and(endpoints::build(&db_connection, &llm_client))
        .with(warp::trace::request());

    let filters = health_check().or(api).with(cors).recover(handle_rejection);

    let (_addr, handle) =
        warp::serve(filters).bind_with_graceful_shutdown((config.host, config.port), async move {
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
    llm: Arc<Box<dyn LLM>>,
) -> impl Filter<Extract = (Arc<Box<dyn LLM>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || llm.clone())
}
