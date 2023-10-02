use libmemex::db::create_connection_by_uri;
use libmemex::db::queue::{self, check_for_jobs, Job, TaskType};
use libmemex::db::{document, embedding};
use libmemex::embedding::{ModelConfig, SentenceEmbedder};
use libmemex::storage::{get_vector_storage, VectorData, VectorStorage};
use libmemex::NAMESPACE;
use sea_orm::prelude::*;
use sea_orm::Set;
use sea_orm::TransactionTrait;
use std::sync::{Arc, Mutex};
use tokio::{
    sync::{broadcast, mpsc},
    time::Duration,
};

#[derive(Debug, Clone)]
pub enum AppShutdown {
    Now,
}

pub enum WorkerCommand {
    GenerateEmbedding(Job),
    LLMExtract(Job),
    LLMSummarize(Job),
}

pub struct WorkerInstanceLimits {
    pub num_active: usize,
    pub max_active: usize,
}

impl Default for WorkerInstanceLimits {
    fn default() -> Self {
        Self {
            num_active: 0,
            max_active: 5,
        }
    }
}

impl WorkerInstanceLimits {
    pub fn can_work(&self) -> bool {
        self.num_active < self.max_active
    }
}

pub type WorkerLimitMutex = Arc<Mutex<WorkerInstanceLimits>>;

pub async fn start(db_uri: String) {
    let db = match create_connection_by_uri(&db_uri, false).await {
        Ok(db) => db,
        Err(err) => {
            log::error!("Unable to connect to db: {err}");
            return;
        }
    };

    let limits = Arc::new(Mutex::new(WorkerInstanceLimits::default()));

    // Create channels for scheduler / crawlers
    let (worker_cmd_tx, worker_cmd_rx) = mpsc::channel::<WorkerCommand>(5);

    // Handle shutdowns
    let (shutdown_tx, _) = broadcast::channel::<AppShutdown>(5);

    // Work scheduler
    let scheduler = tokio::spawn(run_scheduler(
        db.clone(),
        limits.clone(),
        worker_cmd_tx,
        shutdown_tx.subscribe(),
    ));

    // Work handlers
    let workers = tokio::spawn(run_workers(
        db,
        limits,
        worker_cmd_rx,
        shutdown_tx.subscribe(),
    ));

    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            log::warn!("Shutdown request received");
            shutdown_tx
                .send(AppShutdown::Now)
                .expect("Unable to send AppShutdown cmd");
        }
        Err(err) => {
            log::error!("Unable to listen for shutdown signal: {}", err);
            shutdown_tx
                .send(AppShutdown::Now)
                .expect("Unable to send AppShutdown cmd");
        }
    }

    let _ = tokio::join!(scheduler, workers);
}

// Simple wrapper to return early if we're already at our processing limit.
async fn check_for_jobs_with_limit(
    db: &DatabaseConnection,
    limits: WorkerLimitMutex,
) -> Result<Option<Job>, DbErr> {
    let can_work = if let Ok(limits) = limits.lock() {
        limits.can_work()
    } else {
        false
    };

    if can_work {
        return check_for_jobs(db).await;
    }

    Ok(None)
}

pub async fn run_scheduler(
    db: DatabaseConnection,
    limits: WorkerLimitMutex,
    queue: mpsc::Sender<WorkerCommand>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    let mut queue_check_interval = tokio::time::interval(Duration::from_millis(100));
    // first tick always completes immediately.
    queue_check_interval.tick().await;
    loop {
        tokio::select! {
            job = check_for_jobs_with_limit(&db, limits.clone()) => {
                match job {
                    Ok(Some(job)) => {
                        log::debug!("found task: {:?}", job);
                        // Update limits
                        if let Ok(mut limits) = limits.lock() {
                            limits.num_active += 1;
                        }

                        let cmd = match job.task_type {
                            TaskType::Ingest => WorkerCommand::GenerateEmbedding,
                            TaskType::Extract => WorkerCommand::LLMExtract,
                            TaskType::Summarize => WorkerCommand::LLMSummarize,
                        };

                        if let Err(err) = queue.send(cmd(job)).await {
                            log::error!("Worker channel closed: {err}");
                            return;
                        }
                    },
                    // No tasks detected
                    Ok(None) => {},
                    Err(err) => {
                        log::error!("Unable to check job queue: {err}");
                    }
                }

                // wait a little before grabbing the next job
                queue_check_interval.tick().await;
            }
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down scheduler");
                return;
            }
        }
    }
}

pub async fn run_workers(
    db: DatabaseConnection,
    limits: WorkerLimitMutex,
    mut task_queue: mpsc::Receiver<WorkerCommand>,
    mut shutdown_rx: broadcast::Receiver<AppShutdown>,
) {
    loop {
        tokio::select! {
            cmd = task_queue.recv() => {
                if let Some(cmd) = cmd {
                    match cmd {
                        WorkerCommand::GenerateEmbedding(job) => {
                            // Get payload
                            let task = match queue::Entity::find_by_id(job.id).one(&db).await {
                                Ok(Some(model)) => model,
                                _ => continue
                            };

                            {
                                let limits = limits.clone();
                                let db = db.clone();
                                log::info!("[job={}] spawning task", task.id);
                                tokio::spawn(async move {
                                    let vector_uri = std::env::var("VECTOR_CONNECTION").expect("VECTOR_CONNECTION env var not set");
                                    let client = match get_vector_storage(&vector_uri, &task.collection).await {
                                        Ok(client) => client,
                                        Err(err) => {
                                            log::error!("Unable to connect to vector db: {err}");
                                            return;
                                        }
                                    };

                                    if let Err(err) = _process_embeddings(db, client, &task).await {
                                        log::error!("[job={}] Unable to process embeddings: {err}", task.id);
                                    }

                                    if let Ok(mut limits) = limits.lock() {
                                        limits.num_active -= 1;
                                    }
                                });
                            }

                        }
                        WorkerCommand::LLMExtract(_) | WorkerCommand::LLMSummarize(_) => {
                            todo!()
                        }
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down worker");
                return;
            }
        }
    }
}

async fn _process_embeddings(
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
    let _ = queue::mark_done(&db, task.id).await;
    log::info!(
        "[job={}] job finished in {}ms",
        task.id,
        start.elapsed().as_millis()
    );

    Ok(())
}
