use libmemex::db::{document, embedding, queue};
use libmemex::embedding::{ModelConfig, SentenceEmbedder};
use libmemex::llm::openai::{segment, OpenAIClient};
use libmemex::llm::prompter;
use libmemex::storage::{VectorData, VectorStorage};
use libmemex::NAMESPACE;
use sea_orm::{prelude::*, Set, TransactionTrait};

pub async fn process_embeddings(
    db: DatabaseConnection,
    client: VectorStorage,
    task: &queue::Model,
) -> anyhow::Result<()> {
    let start = std::time::Instant::now();
    let model_config = ModelConfig::default();

    let (_handle, embedder) = SentenceEmbedder::spawn(&model_config);
    log::info!("[job={}] generating embeddings", task.id);
    let embeddings = embedder.encode(task.payload.content.clone()).await?;
    log::info!(
        "[job={}] created {} embeddings in {}ms",
        task.id,
        embeddings.len(),
        start.elapsed().as_millis()
    );

    // Create a wrapper document w/ all the data from the task
    let document = document::ActiveModel::from_task(task);
    let document = document.insert(&db).await?;

    let txn = db.begin().await?;
    // Persist vectors to db & vector store
    let mut vectors = Vec::new();
    for (idx, embedding) in embeddings.iter().enumerate() {
        // Create a unique identifier for this segment w/ the task_id & segment
        let uuid = uuid::Uuid::new_v5(
            &NAMESPACE,
            format!("{}-{idx}", document.uuid.clone()).as_bytes(),
        )
        .to_string();

        let mut new_seg = embedding::ActiveModel::new();
        new_seg.uuid = Set(uuid.clone());
        new_seg.document_id = Set(document.uuid.clone());
        new_seg.segment = Set(idx as i64);
        new_seg.content = Set(embedding.content.clone());
        new_seg.vector = Set(embedding.vector.clone().into());
        new_seg.insert(&txn).await?;

        vectors.push(VectorData {
            _id: uuid.clone(),
            document_id: document.uuid.clone(),
            text: embedding.content.clone(),
            segment_id: idx,
            vector: embedding.vector.clone(),
        });
    }

    if let Err(err) = client.add_vectors(vectors).await {
        log::error!("[job={}] Unable to upsert points: {err}", task.id);
    } else {
        log::info!("[job={}] Persisted embeddings", task.id);
    }
    txn.commit().await?;
    Ok(())
}

pub async fn generate_summary(client: &OpenAIClient, payload: &str) -> anyhow::Result<String> {
    // Break task content into segments
    let (splits, model) = segment(payload);
    let mut buffer = String::new();
    for (idx, segment) in splits.iter().enumerate() {
        let time = std::time::Instant::now();
        let request = prompter::summarize(segment);

        if let Ok(content) = client.chat_completion(&model, &request).await {
            buffer.push_str(&content);
        }

        log::info!(
            "segment {} finished in {}ms",
            idx + 1,
            time.elapsed().as_millis()
        );
    }

    Ok(buffer)
}
