use chrono::Utc;
use clap::{Parser, Subcommand};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::PathBuf, process::ExitCode};
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Subcommand, PartialEq)]
enum Command {
    /// Ask clippy a question.
    Ask {
        /// Question you want to ask clippy
        question: String,
    },
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub segment: i64,
    pub content: String,
    pub score: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResults {
    pub results: Vec<Document>
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
        elog(format!("ERROR: Received error from memex: {err}"));
        return ExitCode::FAILURE;
    }

    match args.command {
        Command::Ask { question } => {
            println!("asking clippy: {question}");
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

            let resp = result.json::<TaskResult>().await.expect("Unable to parse response");
            println!("✅ added document (task_id: {})", resp.task_id);
        }
        Command::Forget => {
            println!("Erasing clippy's memory.");
        }
    }

    ExitCode::SUCCESS
}