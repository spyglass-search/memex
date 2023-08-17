use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

use config::ClippyConfig;
use indicatif::ProgressBar;
use llm::{InferenceRequest, InferenceResponse, InferenceStats, KnownModel, LoadProgress};
use serde::Deserialize;
use tera::Tera;
use tokio::sync::mpsc;

pub mod config;

#[derive(Debug, Deserialize)]
pub struct Document {
    pub id: String,
    pub segment: i64,
    pub content: String,
    pub score: f32,
}

#[derive(Debug)]
pub enum LlmEvent {
    ModelLoadProgress(LoadProgress),
    TokenReceived(String),
    InferenceDone,
}

pub fn clippy_say(msg: &str) {
    println!("📎 💬: {msg}");
}

fn build_prompt(
    template_path: PathBuf,
    question: &str,
    ctxt: &[Document],
) -> anyhow::Result<String> {
    let mut tera = Tera::default();
    let mut file = File::open(template_path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    tera.add_raw_template("prompt", &buf)?;

    let mut ctx = tera::Context::new();
    // Add question
    ctx.insert("question", question);
    // Format today's date like so "Sunday, January 1st 1960"
    let today = chrono::Local::now();
    let today_string = today.format("%A, %B %d %Y at %I:%M %p").to_string();
    ctx.insert("today", &today_string);
    // Bot & user name
    ctx.insert("bot", "clippy");
    ctx.insert("user", "user");

    let context = if ctxt.is_empty() {
        "Answer the following question concisely.".to_string()
    } else {
        let extract = ctxt
            .iter()
            .map(|doc| format!("doc_id: {}\ncontent: {}", doc.id, doc.content))
            .collect::<Vec<String>>()
            .join("\n---\n");
        format!("Answer the question given the following extracted parts of a document:\n```\n{extract}\n```")
    };
    ctx.insert("context", &context);

    match tera.render("prompt", &ctx) {
        Ok(s) => Ok(s),
        Err(err) => Err(anyhow::anyhow!(format!(
            "Unable to create prompt from template: {err}"
        ))),
    }
}

pub async fn handle_llm_events(pbar: ProgressBar, mut receiver: mpsc::UnboundedReceiver<LlmEvent>) {
    let mut first_token = true;
    loop {
        if let Some(event) = receiver.recv().await {
            match &event {
                LlmEvent::TokenReceived(token) => {
                    if first_token {
                        print!("📎 💬:");
                        pbar.finish_and_clear();
                        first_token = false;
                    }
                    print!("{}", token);
                    std::io::stdout().flush().unwrap();
                }
                LlmEvent::InferenceDone => {
                    std::io::stdout().flush().unwrap();
                    return;
                }
                LlmEvent::ModelLoadProgress(prog) => {
                    pbar.set_message(format!("{prog:?}"));
                }
            }
        }
    }
}

pub fn ask_clippy(
    cfg: &ClippyConfig,
    pbar: &ProgressBar,
    question: &str,
    ctxt: &[Document],
    llm_events: mpsc::UnboundedSender<LlmEvent>,
) -> anyhow::Result<InferenceStats> {
    // Build prompt
    pbar.set_message("Building prompt...");
    let prompt = match build_prompt(cfg.prompt_template.clone(), question, ctxt) {
        Ok(s) => s,
        Err(err) => {
            return Err(anyhow::anyhow!(format!(
                "Unable to create prompt from template: {err}"
            )));
        }
    };

    // Load configuration
    pbar.set_message("Loading model...");
    let model_params = cfg.to_model_params();
    let infer_params = cfg.to_inference_params();
    let prompt_request = InferenceRequest {
        prompt: llm::Prompt::Text(&prompt),
        maximum_token_count: None,
        parameters: &infer_params,
        play_back_previous_tokens: false,
    };

    let channel = llm_events.clone();
    let model = match llm::load::<llm::models::Llama>(
        &cfg.model.path,
        llm::TokenizerSource::Embedded,
        model_params,
        move |progress| {
            if !channel.is_closed() {
                let _ = channel.send(LlmEvent::ModelLoadProgress(progress));
            }
        },
    ) {
        Ok(model) => model,
        Err(err) => return Err(anyhow::anyhow!("Unable to load model: {err}")),
    };

    // Run inference
    let channel = llm_events.clone();
    let mut session = model.start_session(Default::default());
    let res = session.infer::<std::convert::Infallible>(
        &model,
        &mut rand::thread_rng(),
        &prompt_request,
        &mut Default::default(),
        move |t| {
            match t {
                InferenceResponse::InferredToken(token) => {
                    if channel.send(LlmEvent::TokenReceived(token)).is_err() {
                        return Ok(llm::InferenceFeedback::Halt);
                    }
                }
                InferenceResponse::EotToken => {
                    if channel.send(LlmEvent::InferenceDone).is_err() {
                        return Ok(llm::InferenceFeedback::Halt);
                    }
                }
                _ => {}
            }

            Ok(llm::InferenceFeedback::Continue)
        },
    );

    let _ = llm_events.send(LlmEvent::InferenceDone);
    match res {
        Ok(stats) => Ok(stats),
        Err(err) => Err(anyhow::anyhow!("Unable to run inference: {err}")),
    }
}
