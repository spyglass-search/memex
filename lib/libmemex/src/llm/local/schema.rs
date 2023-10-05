use llm::{LoadProgress, ModelArchitecture};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug)]
pub enum LlmEvent {
    ModelLoadProgress(LoadProgress),
    TokenReceived(String),
    InferenceDone,
}

#[derive(Deserialize)]
pub struct LocalLLMConfig {
    pub prompt_template: PathBuf,
    pub model: ModelConfig,
}

#[derive(Clone, Deserialize)]
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

#[derive(Clone, Deserialize)]
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
