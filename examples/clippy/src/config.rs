use std::{path::PathBuf, sync::Arc};

use llm::{samplers::TopPTopK, ModelArchitecture};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ClippyConfig {
    pub prompt_template: PathBuf,
    pub model: ModelConfig,
}

impl ClippyConfig {
    pub fn to_model_params(&self) -> llm::ModelParameters {
        llm::ModelParameters {
            prefer_mmap: false,
            context_size: 2048,
            lora_adapters: None,
        }
    }

    pub fn to_inference_params(&self) -> llm::InferenceParameters {
        llm::InferenceParameters {
            sampler: Arc::new(TopPTopK {
                top_k: self.model.top_k,
                top_p: self.model.top_p,
                temperature: self.model.temperature,
                repeat_penalty: self.model.repeat_penalty,
                repetition_penalty_last_n: self.model.repetition_penalty_last_n,
                bias_tokens: llm::TokenBias::empty(),
            }),
            ..Default::default()
        }
    }
}

#[derive(Deserialize)]
pub enum ModelArch {
    Bloom,
    Gpt2,
    GptJ,
    GptNeoX,
    Llama,
    Mpt,
}

impl From<ModelArchitecture> for ModelArch {
    fn from(value: ModelArchitecture) -> Self {
        match value {
            ModelArchitecture::Bloom => Self::Bloom,
            ModelArchitecture::Gpt2 => Self::Gpt2,
            ModelArchitecture::GptJ => Self::GptJ,
            ModelArchitecture::GptNeoX => Self::GptNeoX,
            ModelArchitecture::Llama => Self::Llama,
            ModelArchitecture::Mpt => Self::Mpt,
        }
    }
}

#[derive(Deserialize)]
pub struct ModelConfig {
    pub path: PathBuf,
    pub model_type: ModelArch,
    pub prefer_mmap: bool,
    pub top_k: usize,
    pub top_p: f32,
    pub repeat_penalty: f32,
    pub temperature: f32,
    pub repetition_penalty_last_n: usize,
}
