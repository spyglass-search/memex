use embedder::{ModelConfig, SentenceEmbedder};
use sea_orm::prelude::*;
use sea_orm::Set;
use sea_orm::TransactionTrait;
use shared::db::create_connection_by_uri;
use shared::db::document;
use shared::db::queue::{self, check_for_jobs, Job};
use shared::vector::{get_vector_storage, VectorData, VectorStorage};
use std::sync::{Arc, Mutex};
use tokio::{
    sync::{broadcast, mpsc},
    time::Duration,
};

pub const NAMESPACE: uuid::Uuid = uuid::uuid!("5fdfe40a-de2c-11ed-bfa7-00155deae876");

#[derive(Debug, Clone)]
pub enum AppShutdown {
    Now,
}

pub enum WorkerCommand {
    GenerateEmbedding(Job),
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
    let db = match create_connection_by_uri(&db_uri).await {
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

                        if let Err(err) = queue.send(WorkerCommand::GenerateEmbedding(job)).await {
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
    let vector_uri = std::env::var("VECTOR_CONNECTION").expect("VECTOR_CONNECTION env var not set");
    let client = match get_vector_storage(&vector_uri).await {
        Ok(client) => client,
        Err(err) => {
            log::error!("Unable to connect to vector db: {err}");
            return;
        }
    };

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
                                let client = client.clone();
                                let limits = limits.clone();
                                let db = db.clone();
                                log::info!("[job={}] spawning task", task.task_id);
                                tokio::spawn(async move {
                                    if let Err(err) = _process_embeddings(db, client, task.clone()).await {
                                        log::error!("[job={}] Unable to process embeddings: {err}", task.task_id);
                                    }

                                    if let Ok(mut limits) = limits.lock() {
                                        limits.num_active -= 1;
                                    }
                                });
                            }

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
    task: queue::Model,
) -> anyhow::Result<()> {
    let start = std::time::Instant::now();
    let model_config = ModelConfig::default();

    let (_handle, embedder) = SentenceEmbedder::spawn(&model_config);
    log::info!("[job={}] generating embeddings", task.task_id);
    let embeddings = embedder.encode(task.payload.content).await?;
    log::info!(
        "[job={}] created {} embeddings in {}ms",
        task.task_id,
        embeddings.len(),
        start.elapsed().as_millis()
    );

    let txn = db.begin().await?;
    // Persist vectors to db & vector store
    let mut vectors = Vec::new();
    for (idx, embedding) in embeddings.iter().enumerate() {
        let doc_id = uuid::Uuid::new_v5(&NAMESPACE, format!("{}-{idx}", task.task_id).as_bytes())
            .to_string();

        let mut new_doc = document::ActiveModel::new();
        new_doc.task_id = Set(task.task_id.to_string());
        new_doc.document_id = Set(doc_id.clone());
        new_doc.segment = Set(idx as i64);
        new_doc.content = Set(embedding.content.clone());
        new_doc.insert(&txn).await?;

        vectors.push(VectorData {
            doc_id,
            vector: embedding.vector.clone(),
        });
    }

    if let Err(err) = client.add_vectors(vectors).await {
        log::error!("[job={}] Unable to upsert points: {err}", task.task_id);
    } else {
        log::info!("[job={}] Persisted embeddings", task.task_id);
    }
    txn.commit().await?;
    let _ = queue::mark_done(&db, task.id).await;
    log::info!(
        "[job={}] job finished in {}ms",
        task.task_id,
        start.elapsed().as_millis()
    );

    Ok(())
}
