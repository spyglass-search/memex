use llm::{
    samplers::{
        llm_samplers::{
            configure::{SamplerChainBuilder, SamplerSlot},
            samplers::{SampleRepetition, SampleTemperature, SampleTopK, SampleTopP},
        },
        ConfiguredSamplers,
    },
    ModelArchitecture,
};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug)]
pub enum LlmEvent {
    TokenReceived(String),
    InferenceDone,
}

#[derive(Deserialize)]
pub struct LocalLLMConfig {
    pub prompt_template: PathBuf,
    pub model: ModelConfig,
}

impl LocalLLMConfig {
    pub fn to_model_params(&self) -> llm::ModelParameters {
        llm::ModelParameters {
            prefer_mmap: false,
            context_size: 2048,
            lora_adapters: None,
            ..Default::default()
        }
    }

    pub fn base_samplers(&self) -> ConfiguredSamplers {
        let model = self.model.clone();
        let sampler_builder: SamplerChainBuilder = SamplerChainBuilder::from([
            (
                "repetition",
                SamplerSlot::new_chain(
                    move || {
                        Box::new(
                            SampleRepetition::default()
                                .penalty(model.repeat_penalty)
                                .last_n(model.repetition_penalty_last_n),
                        )
                    },
                    [],
                ),
            ),
            (
                "topk",
                SamplerSlot::new_single(
                    move || Box::new(SampleTopK::default().k(model.top_k)),
                    Option::<SampleTopK>::None,
                ),
            ),
            (
                "topp",
                SamplerSlot::new_single(
                    move || Box::new(SampleTopP::default().p(model.top_p)),
                    Option::<SampleTopK>::None,
                ),
            ),
            (
                "temperature",
                SamplerSlot::new_single(
                    move || Box::new(SampleTemperature::default().temperature(model.temperature)),
                    Option::<SampleTopK>::None,
                ),
            ),
        ]);

        let mut sampler = ConfiguredSamplers {
            builder: sampler_builder,
            ..Default::default()
        };
        sampler.ensure_default_slots();
        sampler
    }
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

/// TODO: Test on more architectures
#[derive(Clone, Deserialize)]
pub enum ModelArch {
    // Bloom,
    // Gpt2,
    GptJ,
    // GptNeoX,
    Llama,
    // Mpt,
}

impl From<ModelArchitecture> for ModelArch {
    fn from(value: ModelArchitecture) -> Self {
        match value {
            // ModelArchitecture::Bloom => Self::Bloom,
            // ModelArchitecture::Gpt2 => Self::Gpt2,
            ModelArchitecture::GptJ => Self::GptJ,
            // ModelArchitecture::GptNeoX => Self::GptNeoX,
            ModelArchitecture::Llama => Self::Llama,
            // ModelArchitecture::Mpt => Self::Mpt,
            _ => {
                panic!("Model architecture not yet supported");
            }
        }
    }
}
