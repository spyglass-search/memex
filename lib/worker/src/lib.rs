use std::sync::{Arc, Mutex};

use embedder::{ModelConfig, SentenceEmbedder};
use sea_orm::prelude::*;
use shared::db::create_connection_by_uri;
use shared::db::queue::{self, check_for_jobs, Job};
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

pub async fn start_worker(db_uri: &str) -> anyhow::Result<()> {
    let db = create_connection_by_uri(db_uri).await?;
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

    let _ = tokio::join!(scheduler, workers);
    Ok(())
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
                log::info!("🛑 Shutting down crawl manager");
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
    let model_config = ModelConfig::default();
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
                                tokio::spawn(async move {
                                    let (_handle, embedder) = SentenceEmbedder::spawn(&model_config);
                                    match embedder.encode(task.payload.content).await {
                                        Ok(embeddings) => {
                                            log::info!("created {} embeddings", embeddings.len());
                                        }
                                        Err(err) => {
                                            log::error!("Unable to encode job.id={} - {err}", task.id);
                                        }
                                    }

                                    let _ = queue::mark_done(&db, task.id).await;

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
