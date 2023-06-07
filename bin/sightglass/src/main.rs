use dotenv_codegen::dotenv;

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
const LIB_LOG_LEVEL: &str = "sightglass=DEBUG";
#[cfg(not(debug_assertions))]
const LIB_LOG_LEVEL: &str = "sightglass=INFO";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
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
        #[arg(short, long, default_values_t = vec![Roles::Api])]
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
                .add_directive("warp=INFO".parse().expect("invalid log filter")),
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
        let host = match dotenv!("HOST").parse::<Ipv4Addr>() {
            Ok(host) => host,
            Err(err) => {
                log::error!("Invalid HOST string {err}");
                return ExitCode::FAILURE;
            }
        };

        let port = match dotenv!("PORT").parse::<u16>() {
            Ok(port) => port,
            Err(err) => {
                log::error!("Invalid PORT string {err}");
                return ExitCode::FAILURE;
            }
        };

        let mut handles = Vec::new();
        if roles.contains(&Roles::Api) {
            let api_rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("api-runtime")
                .build()
                .expect("Unable to create runtime");

            let handle = api_rt.spawn(api::start(host, port));
            handles.push(handle);
        }

        if roles.contains(&Roles::Worker) {
            let worker_rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("api-runtime")
                .build()
                .expect("Unable to create runtime");

            let handle = worker_rt.spawn(api::start(host, port));
            handles.push(handle);
        }

        let _ = join_all(handles).await;
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}