use std::path::PathBuf;

use indicatif::ProgressBar;
use llm::{
    InferenceParameters, InferenceRequest, InferenceResponse, KnownModel, LoadProgress,
    ModelParameters, VocabularySource,
};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum LlmEvent {
    ModelLoadProgress(LoadProgress),
    TokenReceived(String),
    InferenceDone,
}

pub async fn prompt_llm(
    pbar: &ProgressBar,
    prompt: &str,
    llm_events: mpsc::UnboundedSender<LlmEvent>,
) {
    pbar.set_message("Loading model...");

    let model_params = ModelParameters {
        prefer_mmap: false,
        context_size: 2048,
        lora_adapters: None,
    };

    let infer_params = InferenceParameters::default();
    let full_prompt = format!(r#"
        A chat between a human ("user") and an AI assistant ("clippy"). The assistant
        gives helpful, detailed, and polite answers to the human's questions.

        clippy: How may I help you?
        user: what is today's date?
        clippy: It is June 12th, 2023
        user: {prompt}
    "#);

    let prompt_request = InferenceRequest {
        prompt: llm::Prompt::Text(&full_prompt),
        maximum_token_count: None,
        parameters: &infer_params,
        play_back_previous_tokens: false,
    };

    let channel = llm_events.clone();
    let model_path: PathBuf = "resources/Wizard-Vicuna-7B-Uncensored.ggmlv3.q4_0.bin".into();
    let model = llm::load::<llm::models::Llama>(
        &model_path,
        VocabularySource::Model,
        model_params,
        move |progress| {
            if !channel.is_closed() {
                let _ = channel.send(LlmEvent::ModelLoadProgress(progress));
            }
        },
    )
    .expect("Unable to load model");

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
                    if let Err(_) = channel.send(LlmEvent::TokenReceived(token)) {
                        return Ok(llm::InferenceFeedback::Halt);
                    }
                }
                InferenceResponse::EotToken => {
                    if let Err(_) = channel.send(LlmEvent::InferenceDone) {
                        return Ok(llm::InferenceFeedback::Halt);
                    }
                }
                _ => {}
            }

            Ok(llm::InferenceFeedback::Continue)
        },
    );

    let _ = llm_events.send(LlmEvent::InferenceDone);
    if let Err(err) = res {
        eprintln!("Unable to run inference: {err}");
    }
}
