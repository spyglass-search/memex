use llm::samplers::llm_samplers::samplers::SampleFlatBias;
use llm::samplers::llm_samplers::types::SamplerChain;
use llm::{self, samplers::ConfiguredSamplers, InferenceSessionConfig};
use llm::{InferenceParameters, LoadProgress};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tiktoken_rs::cl100k_base;
use tokio::sync::mpsc;

use crate::llm::{split_text, ChatRole};

use self::schema::LocalLLMConfig;

use super::{ChatMessage, LLMError, LLM};
mod schema;
use schema::LlmEvent;

pub const MAX_TOKENS: usize = 2_048 - 512 - 100;

#[derive(Clone)]
pub struct LocalLLM<T>
where
    T: llm::KnownModel,
{
    model: T,
    infer_params: InferenceParameters,
    /// At the moment does nothing but will eventually be used by our internal
    /// sampler to only output JSON/etc.
    _bias_sampler: Arc<Mutex<SampleFlatBias>>,
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
            _bias_sampler: Arc::new(Mutex::new(bias_sampler)),
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
impl<T> LLM for LocalLLM<T>
where
    T: llm::KnownModel,
{
    async fn chat_completion(
        &self,
        _: &str,
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

    fn segment_text(&self, text: &str) -> (Vec<String>, String) {
        let cl = cl100k_base().unwrap();
        let size = cl.encode_with_special_tokens(text).len();
        log::debug!("context size: {size}");

        if size <= MAX_TOKENS {
            (vec![text.to_string()], Default::default())
        } else {
            let splits = split_text(text, MAX_TOKENS);
            (splits, Default::default())
        }
    }

    fn truncate_text(&self, text: &str) -> (String, String) {
        let cl = cl100k_base().unwrap();
        let total_tokens: usize = cl.encode_with_special_tokens(text).len();

        if total_tokens <= MAX_TOKENS {
            (text.to_string(), Default::default())
        } else {
            let mut buffer = String::new();
            for txt in text.split(' ') {
                let with_txt = buffer.clone() + txt;
                let current_size = cl.encode_with_special_tokens(&with_txt).len();
                if current_size > MAX_TOKENS {
                    break;
                } else {
                    buffer.push_str(txt);
                }
            }

            (buffer, Default::default())
        }
    }
}

pub async fn load_from_cfg(
    llm_config: PathBuf,
    report_progress: bool,
) -> anyhow::Result<Box<dyn LLM>> {
    let config = std::fs::read_to_string(llm_config.clone())?;
    let config: LocalLLMConfig = toml::from_str(&config)?;

    let parent_dir = llm_config.parent().unwrap();
    let model_path: PathBuf = parent_dir.join(config.model.path.clone());

    let model_params = config.to_model_params();
    let model = llm::load::<llm::models::Llama>(
        &model_path,
        llm::TokenizerSource::Embedded,
        model_params,
        move |event| {
            if report_progress {
                match &event {
                    LoadProgress::TensorLoaded {
                        current_tensor,
                        tensor_count,
                    } => {
                        log::info!("Loaded {}/{} tensors", current_tensor, tensor_count);
                    }
                    LoadProgress::Loaded { .. } => {
                        log::info!("Model finished loading");
                    }
                    _ => {}
                }
            }
        },
    )?;

    let llm = LocalLLM::new(model);
    Ok(Box::new(llm))
}

#[cfg(test)]
mod test {
    use crate::llm::ChatMessage;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_prompting() {
        let base_dir: PathBuf = "../..".into();
        let model_config: PathBuf = base_dir.join("resources/config.llama2.toml");

        let llm = super::load_from_cfg(model_config, true).await
            .expect("Unable to load model");

        let msgs = vec![
            ChatMessage::system("You're a helpful assistant. Answer questions as accurately and concisely as possible."),
            ChatMessage::user("Who won the world series in 2020?"),
        ];

        let result = llm.chat_completion(Default::default(), &msgs).await;
        assert!(result.is_ok());
        dbg!(result.unwrap());
    }
}
