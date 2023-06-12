use std::{fs::File, io::Read, path::PathBuf};

use config::ClippyConfig;
use indicatif::ProgressBar;
use llm::{
    InferenceRequest, InferenceResponse, InferenceStats, KnownModel, LoadProgress, VocabularySource,
};
use tera::Tera;
use tokio::sync::mpsc;

pub mod config;

#[derive(Debug)]
pub enum LlmEvent {
    ModelLoadProgress(LoadProgress),
    TokenReceived(String),
    InferenceDone,
}

fn build_prompt(template_path: PathBuf, question: &str) -> anyhow::Result<String> {
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

    match tera.render("prompt", &ctx) {
        Ok(s) => Ok(s),
        Err(err) => Err(anyhow::anyhow!(format!(
            "Unable to create prompt from template: {err}"
        ))),
    }
}

pub async fn ask_clippy(
    cfg: &ClippyConfig,
    pbar: &ProgressBar,
    question: &str,
    llm_events: mpsc::UnboundedSender<LlmEvent>,
) -> anyhow::Result<InferenceStats> {
    // Build prompt
    pbar.set_message("Building prompt...");
    let prompt = match build_prompt(cfg.prompt_template.clone(), question) {
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
        VocabularySource::Model,
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
