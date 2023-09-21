use std::path::PathBuf;

use llm::{self, InferenceSessionConfig, KnownModel, LoadProgress};
use tokio::sync::mpsc;

pub mod embedding;
pub mod openai;
pub mod prompter;

#[derive(Debug)]
pub enum LlmEvent {
    ModelLoadProgress(LoadProgress),
    TokenReceived(String),
    InferenceDone,
}

pub fn run_prompt(prompt: &str) -> anyhow::Result<()> {
    println!("Prompting...");
    let model_params = llm::ModelParameters::default();
    let infer_params = llm::InferenceParameters::default();

    let prompt_request = llm::InferenceRequest {
        prompt: llm::Prompt::Text(prompt),
        maximum_token_count: None,
        parameters: &infer_params,
        play_back_previous_tokens: false,
    };

    let model_path: PathBuf =
        "../../resources/models/LLaMa2/llama-2-7b-chat.ggmlv3.q4_1.bin".into();
    let model = match llm::load::<llm::models::Llama>(
        &model_path,
        llm::TokenizerSource::Embedded,
        model_params,
        move |_| {},
    ) {
        Ok(model) => model,
        Err(err) => return Err(anyhow::anyhow!("Unable to load model: {err}")),
    };

    let config = InferenceSessionConfig::default();
    let mut session = model.start_session(config);

    let (sender, _receiver) = mpsc::unbounded_channel::<LlmEvent>();

    print!("Prompt: {}", prompt);
    let _res = session.infer::<std::convert::Infallible>(
        &model,
        &mut rand::thread_rng(),
        &prompt_request,
        &mut Default::default(),
        move |t| {
            match t {
                llm::InferenceResponse::InferredToken(token) => {
                    print!("{}", token);
                    if sender.send(LlmEvent::TokenReceived(token)).is_err() {
                        return Ok(llm::InferenceFeedback::Halt);
                    }
                }
                llm::InferenceResponse::EotToken => {
                    if sender.send(LlmEvent::InferenceDone).is_err() {
                        return Ok(llm::InferenceFeedback::Halt);
                    }
                }
                _ => {}
            }

            Ok(llm::InferenceFeedback::Continue)
        },
    )?;

    Ok(())
}
