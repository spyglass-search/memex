use llm_base::{TokenId, Tokenizer};

pub type TokenMask = Vec<(TokenId, f32)>;

pub fn create_mask(tokens: &Tokenizer, only_numbers: bool) -> anyhow::Result<TokenMask> {
    let num_tokens = tokens.len();

    let mut mask: Vec<(TokenId, f32)> = Vec::new();

    for idx in 0..num_tokens {
        let token = tokens.token(idx);
        let token_str = String::from_utf8_lossy(&token);

        let bias = if only_numbers {
            if token_str.is_ascii() && token_str.parse::<f32>().is_ok() {
                1.0
            } else {
                f32::NEG_INFINITY
            }
        } else if token_str.is_ascii()
            && !token_str.starts_with(' ')
            && token_str.parse::<f32>().is_err()
        {
            1.0
        } else {
            f32::NEG_INFINITY
        };

        mask.push((idx as u32, bias));
    }

    Ok(mask)
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use llm_base::KnownModel;

    #[tokio::test]
    async fn test_logit_biasing() {
        let model_path: PathBuf =
            "../../resources/models/LLaMa2/llama-2-7b-chat.ggmlv3.q4_1.bin".into();

        let model = llm::load::<llm::models::Llama>(
            &model_path,
            llm::TokenizerSource::Embedded,
            llm::ModelParameters::default(),
            move |_| {},
        )
        .expect("Unable to load model");

        super::create_mask(&model.tokenizer(), false).expect("Unable to create mask");
    }
}
