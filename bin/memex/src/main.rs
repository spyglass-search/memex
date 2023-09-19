use clap::{Parser, Subcommand};
use futures::future::join_all;
use std::{net::Ipv4Addr, process::ExitCode};
use strum_macros::{Display, EnumString};
use tracing_log::LogTracer;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    EnvFilter,
};

const LOG_LEVEL: tracing::Level = tracing::Level::INFO;

#[cfg(debug_assertions)]
const LIB_LOG_LEVEL: &str = "memex=DEBUG";
#[cfg(not(debug_assertions))]
const LIB_LOG_LEVEL: &str = "memex=INFO";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
    #[clap(long, value_parser, value_name = "DATABASE_CONNECTION", env)]
    database_connection: Option<String>,
    #[clap(long, value_parser, value_name = "VECTOR_CONNECTION", env)]
    vector_connection: Option<String>,
}

#[derive(Debug, Display, Clone, PartialEq, EnumString)]
pub enum Roles {
    Api,
    Worker,
}

#[derive(Subcommand, PartialEq)]
enum Command {
    Debug,
    Serve {
        #[arg(short, long, default_values_t = vec![Roles::Api, Roles::Worker])]
        roles: Vec<Roles>,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    dotenv::dotenv().ok();

    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::from_default_env()
                .add_directive(LOG_LEVEL.into())
                .add_directive(LIB_LOG_LEVEL.parse().expect("invalid log filter"))
                .add_directive("api=DEBUG".parse().expect("invalid log filter"))
                .add_directive("worker=DEBUG".parse().expect("invalid log filter"))
                .add_directive("embedder=DEBUG".parse().expect("invalid log filter"))
                .add_directive("cached_path=WARN".parse().expect("invalid log filter"))
                .add_directive("hnsw_rs=WARN".parse().expect("invalid log filter")),
        )
        .with(
            fmt::Layer::new()
                .with_writer(std::io::stdout)
                .with_span_events(FmtSpan::CLOSE),
        );
    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");
    let _ = LogTracer::init();

    let args = Args::parse();

    if let Command::Serve { roles } = args.command {
        if roles.is_empty() {
            log::error!("No roles specified");
            return ExitCode::FAILURE;
        }

        log::info!("starting server with roles: {roles:?}");
        let host = match std::env::var("HOST")
            .expect("HOST not set")
            .parse::<Ipv4Addr>()
        {
            Ok(host) => host,
            Err(err) => {
                log::error!("Invalid HOST string {err}");
                return ExitCode::FAILURE;
            }
        };

        let port = match std::env::var("PORT").expect("PORT not set").parse::<u16>() {
            Ok(port) => port,
            Err(err) => {
                log::error!("Invalid PORT string {err}");
                return ExitCode::FAILURE;
            }
        };

        let db_uri = args
            .database_connection
            .expect("DATABASE_CONNECTION not set");
        let mut handles = Vec::new();

        let _vector_store_uri = args.vector_connection.expect("VECTOR_CONNECTION not set");

        if roles.contains(&Roles::Api) {
            let db_uri = db_uri.clone();
            handles.push(tokio::spawn(api::start(host, port, db_uri)));
        }

        if roles.contains(&Roles::Worker) {
            let db_uri = db_uri.clone();
            handles.push(tokio::spawn(worker::start(db_uri)));
        }

        let _ = join_all(handles).await;
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
