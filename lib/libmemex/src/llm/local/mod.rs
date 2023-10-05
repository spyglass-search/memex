use llm::samplers::llm_samplers::samplers::SampleFlatBias;
use llm::samplers::llm_samplers::types::SamplerChain;
use llm::InferenceParameters;
use llm::{self, samplers::ConfiguredSamplers, InferenceSessionConfig};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::llm::ChatRole;

use super::{ChatMessage, LLMError, LLM};
mod schema;
use schema::LlmEvent;

pub struct LocalLLM<T>
where
    T: llm::KnownModel,
{
    model: T,
    infer_params: InferenceParameters,
    /// At the moment does nothing but will eventually be used by our internal
    /// sampler to only output JSON/etc.
    bias_sampler: Arc<Mutex<SampleFlatBias>>,
}

impl<T> LocalLLM<T>
where
    T: llm::KnownModel,
{
    fn new(model: T) -> Self {
        let bias_sampler = SampleFlatBias::default();
        // Create sampler chain
        let mut samplers = SamplerChain::new();
        samplers += bias_sampler.clone();

        let mut default_samplers = ConfiguredSamplers::default();
        default_samplers.ensure_default_slots();
        samplers += default_samplers.builder.into_chain();

        let infer_params = llm::InferenceParameters {
            sampler: Arc::new(Mutex::new(samplers)), // sampler: llm::samplers::default_samplers(),
        };

        Self {
            model,
            infer_params,
            bias_sampler: Arc::new(Mutex::new(bias_sampler)),
        }
    }

    async fn run_model(&self, prompt: &str) -> anyhow::Result<String, LLMError> {
        log::info!("running model w/ prompt: {prompt}");

        let buffer = Arc::new(Mutex::new(String::new()));
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

        let config = InferenceSessionConfig::default();
        let infer_params = self.infer_params.clone();
        let sender = sender.clone();

        let prompt_request = llm::InferenceRequest {
            prompt: llm::Prompt::Text(prompt),
            maximum_token_count: None,
            parameters: &infer_params,
            play_back_previous_tokens: false,
        };

        let channel = sender.clone();
        let mut session = self.model.start_session(config);
        let _stats = session
            .infer::<std::convert::Infallible>(
                &self.model,
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
            .map_err(|err| LLMError::InferenceError(err.to_string()))?;
        let _ = sender.send(LlmEvent::InferenceDone);
        // Wait for buffer to finish writing
        let _ = writer_handle.await;
        // Retrieve buffer and clean up any trailing/leading spaces.
        let buffer = buffer
            .lock()
            .expect("Unable to grab buffer")
            .trim()
            .to_string();
        Ok(buffer)
    }
}

#[async_trait::async_trait]
impl<T> LLM<schema::LocalLLMSize> for LocalLLM<T>
where
    T: llm::KnownModel,
{
    async fn chat_completion(
        &self,
        _: schema::LocalLLMSize,
        msgs: &[ChatMessage],
    ) -> anyhow::Result<String, LLMError> {
        log::info!("LocalLLM running chat_completion");

        let system_msg = msgs
            .iter()
            .find(|x| x.role == ChatRole::System)
            .map(|x| x.content.clone())
            .unwrap_or(String::from("You're a helpful assistant"));

        // Currently the prompt assumes a llama based model, pull this out into the
        // config file.
        let mut prompt = format!("[INST] <<SYS>>\n{system_msg}\n<</SYS>>\n\n");
        for msg in msgs {
            if msg.role == ChatRole::System {
                continue;
            }

            prompt.push_str(&format!("{}\n", msg.content));
        }
        prompt.push_str("[/INST]");
        self.run_model(&prompt).await
    }
}

#[cfg(test)]
mod test {
    use crate::llm::{ChatMessage, LLM};

    use super::schema::LocalLLMConfig;
    use super::LocalLLM;
    use std::path::PathBuf;

    #[ignore]
    #[tokio::test]
    async fn test_prompting() {
        let base_dir: PathBuf = "../..".into();
        let model_config: PathBuf = base_dir.join("resources/config.llama2.toml");

        let config = std::fs::read_to_string(model_config).expect("Unable to read cfg");
        let config: LocalLLMConfig = toml::from_str(&config).expect("Unable to parse cfg");
        let model_path: PathBuf = base_dir.join(config.model.path);

        let model_params = llm::ModelParameters::default();
        let model = llm::load::<llm::models::Llama>(
            &model_path,
            llm::TokenizerSource::Embedded,
            model_params,
            move |_| {},
        )
        .expect("Unable to load model");

        let llm = LocalLLM::new(model);
        let msgs = vec![
            ChatMessage::system("You're a helpful assistant. Answer questions as accurately and concisely as possible."),
            ChatMessage::user("Who won the world series in 2020?"),
        ];

        let result = llm.chat_completion(Default::default(), &msgs).await;
        assert!(result.is_ok());
        dbg!(result.unwrap());
    }
}
