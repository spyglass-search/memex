use libmemex::db::create_connection_by_uri;
use libmemex::db::queue::{self, check_for_jobs, Job, TaskType};
use libmemex::llm::openai::OpenAIClient;
use libmemex::storage::get_vector_storage;
use sea_orm::{prelude::*, Set};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::{
    sync::{broadcast, mpsc},
    time::Duration,
};

mod tasks;

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

                        // Map task type to the relevant WorkerCommand
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

                            let db = db.clone();

                            tokio::spawn(run_task(task.id, db.clone(), limits.clone(), async move {
                                let vector_uri = std::env::var("VECTOR_CONNECTION").expect("VECTOR_CONNECTION env var not set");
                                let client = match get_vector_storage(&vector_uri, &task.collection).await {
                                    Ok(client) => client,
                                    Err(err) => {
                                        log::error!("Unable to connect to vector db: {err}");
                                        return;
                                    }
                                };

                                if let Err(err) = tasks::process_embeddings(db, client, &task).await {
                                    log::error!("[job={}] Unable to process embeddings: {err}", task.id);
                                }
                            }));
                        }
                        WorkerCommand::LLMExtract(job) => {
                            let _task = match queue::Entity::find_by_id(job.id).one(&db).await {
                                Ok(Some(model)) => model,
                                _ => continue
                            };
                        }
                        WorkerCommand::LLMSummarize(job) => {
                            // Get payload
                            let task = match queue::Entity::find_by_id(job.id).one(&db).await {
                                Ok(Some(model)) => model,
                                _ => continue
                            };

                            {
                                let db = db.clone();
                                let content = task.payload.content.clone();
                                tokio::spawn(run_task(task.id, db.clone(), limits.clone(), async move {
                                    let client = OpenAIClient::new(
                                        &std::env::var("OPENAI_API_KEY").expect("OpenAI API key not set")
                                    );

                                    match tasks::generate_summary(&client, &content).await {
                                        Ok(summary) => {
                                            let value = serde_json::json!({ "bullets": summary });
                                            let mut update: queue::ActiveModel = task.into();
                                            update.task_output = Set(Some(value));
                                            let _ = update.save(&db).await;
                                        },
                                        Err(err) => {
                                            log::error!("[job={}] Unable to generate summary: {err}", task.id);
                                        }
                                    }
                                }));
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

pub async fn run_task<T>(
    task_id: i64,
    db: DatabaseConnection,
    limits: WorkerLimitMutex,
    future: T,
) -> <T as Future>::Output
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
    let start = Instant::now();
    log::info!("[job={}] spawning task", task_id);
    let res = future.await;
    log::info!(
        "[job={}] job finished in {}ms",
        task_id,
        start.elapsed().as_millis()
    );
    let _ = queue::mark_done(&db, task_id).await;
    if let Ok(mut limits) = limits.lock() {
        limits.num_active -= 1;
    }

    res
}
