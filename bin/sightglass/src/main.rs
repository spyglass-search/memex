use dotenv_codegen::dotenv;

use clap::{Parser, Subcommand};
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
    #[strum(serialize = "api")]
    Api,
    #[strum(serialize = "worker")]
    Worker,
}

#[derive(Subcommand, PartialEq)]
enum Command {
    Debug,
    Serve {
        #[arg(short, long)]
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
        log::info!("starting server with roles: {roles:?}");
        let host =  match dotenv!("HOST").parse::<Ipv4Addr>() {
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

        api::start(host, port).await;
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
