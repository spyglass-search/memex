use llm::samplers::llm_samplers::samplers::SampleFlatBias;
use llm::samplers::llm_samplers::types::SamplerChain;
use llm::{self, samplers::ConfiguredSamplers, InferenceSessionConfig, LoadProgress};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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

// Run model and apply rules to generation.
async fn run_guidance<T>(
    model: &T,
    default_samplers: SamplerChain,
    prompt: &str,
) -> anyhow::Result<()>
where
    T: llm::KnownModel,
{
    // Create a mask based on the current tokens generated and
    let token_bias = sherpa::create_mask(model.tokenizer(), false).expect("Unable to create mask");
    let mut bias_sampler = SampleFlatBias::new(token_bias);

    // Create sampler chain
    let mut samplers = SamplerChain::new();
    samplers += bias_sampler.clone();
    samplers += default_samplers;

    let infer_params = llm::InferenceParameters {
        sampler: Arc::new(Mutex::new(samplers)), // sampler: llm::samplers::default_samplers(),
    };

    // Add our biasing mask to the sampler chain.
    let mut num_tokens = 0;
    let buffer = Arc::new(Mutex::new(prompt.to_string()));
    let tokens = Arc::new(Mutex::new(Vec::new()));

    let (sender, mut receiver) = mpsc::unbounded_channel::<LlmEvent>();

    let writer_handle = {
        let buffer = buffer.clone();
        let tokens = tokens.clone();
        tokio::spawn(async move {
            loop {
                if let Some(event) = receiver.recv().await {
                    match &event {
                        LlmEvent::TokenReceived(token) => {
                            if let Ok(mut buff) = buffer.lock() {
                                *buff += token;
                            }

                            if let Ok(mut tokens) = tokens.lock() {
                                tokens.push(token.to_string());
                            }
                        }
                        LlmEvent::InferenceDone => {
                            std::io::stdout().flush().unwrap();
                            return;
                        }
                        _ => {}
                    }
                }
            }
        })
    };

    {
        let config = InferenceSessionConfig::default();
        let buffer = buffer.clone();
        loop {
            let prompt = if let Ok(buff) = buffer.lock() {
                buff.to_string()
            } else {
                continue;
            };
            println!("processing prompt: {}", prompt);
            let infer_params = infer_params.clone();
            let sender = sender.clone();

            let prompt_request = llm::InferenceRequest {
                prompt: llm::Prompt::Text(&prompt),
                maximum_token_count: Some(1),
                parameters: &infer_params,
                play_back_previous_tokens: false,
            };

            let mut session = model.start_session(config);
            let channel = sender.clone();
            let _res = session
                .infer::<std::convert::Infallible>(
                    model,
                    &mut rand::thread_rng(),
                    &prompt_request,
                    &mut Default::default(),
                    move |t| {
                        match t {
                            llm::InferenceResponse::InferredToken(token) => {
                                if channel.send(LlmEvent::TokenReceived(token)).is_err() {
                                    return Ok(llm::InferenceFeedback::Halt);
                                }
                            }
                            llm::InferenceResponse::EotToken => {
                                if channel.send(LlmEvent::InferenceDone).is_err() {
                                    return Ok(llm::InferenceFeedback::Halt);
                                }
                            }
                            _ => {}
                        }

                        Ok(llm::InferenceFeedback::Continue)
                    },
                )
                .expect("Unable");

            let _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            num_tokens += 1;
            *bias_sampler =
                sherpa::create_mask(model.tokenizer(), false).expect("Unable to create mask");
            if num_tokens >= 3 {
                let _ = sender.send(LlmEvent::InferenceDone);
                break;
            }
        }
    }

    let _ = writer_handle.await;
    println!("result: {}", buffer.lock().unwrap());
    println!("tokens generated: {:?}", tokens.lock().unwrap());
    println!("🛑");
    Ok(())
}

pub async fn run_prompt(prompt: &str) -> anyhow::Result<()> {
    println!("Prompting...");

    // Load model
    let model_path: PathBuf =
        "../../resources/models/LLaMa2/llama-2-7b-chat.ggmlv3.q4_1.bin".into();
    let model_params = llm::ModelParameters::default();
    let model = match llm::load::<llm::models::Llama>(
        &model_path,
        llm::TokenizerSource::Embedded,
        model_params,
        move |_| {},
    ) {
        Ok(model) => model,
        Err(err) => return Err(anyhow::anyhow!("Unable to load model: {err}")),
    };

    // Configure samplers
    let mut samplers = ConfiguredSamplers::default();
    samplers.ensure_default_slots();

    run_guidance(&model, samplers.builder.into_chain(), prompt).await
}

#[cfg(test)]
mod test {
    use super::run_prompt;

    #[tokio::test]
    async fn test_structured_prompt() {
        run_prompt("i see london, i see france, i see ")
            .await
            .expect("Unable to prompt");
        println!("");
    }
}
