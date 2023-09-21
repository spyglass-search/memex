pub async fn create_mask() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_logit_biasing() {
        super::create_mask().await.expect("Unable to create mask");
    }
}
