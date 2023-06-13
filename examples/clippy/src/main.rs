use chrono::Utc;
use clap::{Parser, Subcommand};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::PathBuf, process::ExitCode};
use tokio::sync::mpsc;

use libclippy::{ask_clippy, clippy_say, config::ClippyConfig, LlmEvent};

#[derive(Subcommand, PartialEq)]
enum Command {
    /// Ask clippy a question using it's memory of the documents you've added.
    Ask {
        /// Question you want to ask clippy
        question: String,
    },
    /// Ask clippy with only knowledge contained in the model.
    Qq { question: String },
    /// Erase clippy's memory
    #[command(visible_alias = "neuralyze")]
    Forget,
    /// Load a document into Clippy's all-knowing brain.
    LoadFile {
        /// File to load
        file: PathBuf,
    },
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = "http://127.0.0.1:8181")]
    memex_uri: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskResult {
    task_id: i64,
    collection: String,
    status: String,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SearchResults {
    pub results: Vec<libclippy::Document>,
}

fn elog(msg: String) {
    eprintln!("{}: {}", "ERROR".bright_red(), msg);
}

#[tokio::main]
async fn main() -> ExitCode {
    let client = reqwest::Client::new();
    let args = Args::parse();

    // do a quick check to see if memex is running
    let result = match client
        .get(format!("{}/health", args.memex_uri))
        .send()
        .await
    {
        Ok(res) => res,
        Err(err) => {
            elog(format!(
                "Unable to connect to memex (is it running?): {err}"
            ));
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = result.error_for_status() {
        elog(format!("Received error from memex: {err}"));
        return ExitCode::FAILURE;
    }

    // load clippy config
    let clippy_cfg: ClippyConfig = match File::open("resources/config.toml") {
        Ok(mut reader) => {
            let mut config = String::new();
            let _ = reader.read_to_string(&mut config);
            toml::from_str(&config).expect("Unable to parse config.toml")
        }
        Err(err) => {
            let cwd = std::env::current_dir();
            elog(format!(
                "Unable to read config.toml: {err}, current working dir: {cwd:?}"
            ));
            return ExitCode::FAILURE;
        }
    };

    match args.command {
        Command::Ask { question } => {
            handle_ask_cmd(&client, &args.memex_uri, &clippy_cfg, true, &question).await;
        }
        Command::Qq { question } => {
            handle_ask_cmd(&client, &args.memex_uri, &clippy_cfg, false, &question).await;
        }
        Command::LoadFile { file } => {
            if !file.exists() || !file.is_file() {
                elog(format!("{:?} is not a valid file.", file));
                return ExitCode::FAILURE;
            }

            // Read file to string
            let mut file = File::open(file).expect("Unable to open file");
            let mut file_data = String::new();
            file.read_to_string(&mut file_data)
                .expect("Unable to read file");

            // Post to memex
            let result = match client
                .post(format!("{}/collections/clippy", args.memex_uri))
                .json(&serde_json::json!({ "content": file_data }))
                .send()
                .await
            {
                Ok(res) => res,
                Err(err) => {
                    elog(format!("Unable to add file: {err}"));
                    return ExitCode::FAILURE;
                }
            };

            let resp = result
                .json::<TaskResult>()
                .await
                .expect("Unable to parse response");
            println!("✅ added document (task_id: {})", resp.task_id);
        }
        Command::Forget => {
            println!("Erasing clippy's memory.");
        }
    }

    ExitCode::SUCCESS
}

async fn handle_ask_cmd(
    client: &reqwest::Client,
    memex_uri: &str,
    clippy_cfg: &ClippyConfig,
    use_memory: bool,
    question: &str,
) {
    let question = question.trim().to_string();
    if question.is_empty() {
        clippy_say("Please ask a question");
        return;
    }

    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(std::time::Duration::from_millis(120));
    pb.set_style(ProgressStyle::with_template("{spinner:.green} {msg}").unwrap());
    pb.set_message("Rummaging through clippy's memex...");

    let results = if use_memory {
        client
            .get(format!("{}/collections/clippy/search", memex_uri))
            // only use two pieces of content, otherwise loading will take a while.
            .json(&serde_json::json!({ "query": question, "limit": 2 }))
            .send()
            .await
            .expect("Unable to connect to memex")
            .json::<SearchResults>()
            .await
            .expect("Unable to parse response")
            .results
    } else {
        Vec::new()
    };

    clippy_say(&format!("found {} relevant segments", results.len()));

    // Create a channel to to receive events
    let (sender, receiver) = mpsc::unbounded_channel::<LlmEvent>();
    let _guard = sender.clone();

    let writer_handle = tokio::spawn(libclippy::handle_llm_events(pb.clone(), receiver));

    {
        let pb = pb.clone();
        let handle = tokio::runtime::Handle::current();
        let clippy_cfg = clippy_cfg.clone();
        std::thread::spawn(move || {
            handle.spawn_blocking(move || {
                match ask_clippy(&clippy_cfg, &pb, &question, &results, sender) {
                    Ok(stats) => {
                        println!("");
                        println!("⏱️  predict time: {}ms", stats.predict_duration.as_millis());
                    }
                    Err(err) => {
                        eprintln!("Unable to run inference: {err}");
                    }
                }
            });
        });
    };

    let _ = writer_handle.await;
    pb.finish_and_clear();
}
