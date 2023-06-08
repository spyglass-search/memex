use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Set, Statement};
use serde::{Deserialize, Serialize};

const MAX_RETRIES: i32 = 5;

#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Eq)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum JobStatus {
    #[sea_orm(string_value = "Queued")]
    Queued,
    #[sea_orm(string_value = "Processing")]
    Processing,
    #[sea_orm(string_value = "Completed")]
    Completed,
    #[sea_orm(string_value = "Failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct TaskPayload {
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct TaskError {
    pub error_type: String,
    pub msg: String,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub task_id: String,
    pub payload: TaskPayload,
    /// Task status.
    pub status: JobStatus,
    /// If this failed, the reason for the failure
    pub error: Option<TaskError>,
    /// Number of retries for this task.
    #[sea_orm(default_value = 0)]
    pub num_retries: i32,
    /// When this was first added to the crawl queue.
    pub created_at: DateTimeUtc,
    /// When this task was last updated.
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        unimplemented!("No relations")
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            status: Set(JobStatus::Queued),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }

    // Triggered before insert / update
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert {
            self.updated_at = Set(chrono::Utc::now());
        }

        Ok(self)
    }
}

pub async fn mark_done(db: &DatabaseConnection, id: i64) -> Option<Model> {
    if let Ok(Some(crawl)) = Entity::find_by_id(id).one(db).await {
        let mut updated: ActiveModel = crawl.into();
        updated.status = Set(JobStatus::Completed);
        updated.updated_at = Set(chrono::Utc::now());
        updated.update(db).await.ok()
    } else {
        None
    }
}

pub async fn mark_failed(db: &DatabaseConnection, id: i64, retry: bool, error: Option<TaskError>) {
    if let Ok(Some(crawl)) = Entity::find_by_id(id).one(db).await {
        let mut updated: ActiveModel = crawl.clone().into();

        // Bump up number of retries if this failed
        if retry && crawl.num_retries <= MAX_RETRIES {
            updated.num_retries = Set(crawl.num_retries + 1);
            // Queue again
            updated.status = Set(JobStatus::Queued);
        } else {
            updated.status = Set(JobStatus::Failed);
        }

        updated.error = Set(error);
        let _ = updated.update(db).await;
    }
}

pub async fn enqueue<C>(db: &C, job_id: &str, content: &str) -> Result<Model, DbErr>
where
    C: ConnectionTrait,
{
    let mut new = ActiveModel::new();
    new.task_id = Set(job_id.to_string());
    new.payload = Set(TaskPayload {
        content: content.to_string(),
    });

    Entity::insert(new).exec_with_returning(db).await
}

pub async fn enqueue_many<C>(db: &C, models: &[ActiveModel]) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    Entity::insert_many(models.to_vec())
        .exec_without_returning(db)
        .await?;
    Ok(())
}

#[derive(Clone, Debug, FromQueryResult)]
pub struct Job {
    pub id: i64,
}

pub async fn check_for_jobs(db: &DatabaseConnection) -> Result<Option<Job>, DbErr> {
    let backend = db.get_database_backend();
    let sql: String = match backend {
        DatabaseBackend::Sqlite => r#"
            UPDATE queue
            SET
                status = 'Processing',
                updated_at = $1
            WHERE queue.id IN (
                SELECT
                    id
                FROM queue
                WHERE status = 'Queued'
                ORDER BY queue.created_at ASC
                LIMIT 1
            )
            RETURNING queue.id"#
            .into(),
        _ => r#"
            UPDATE queue
            SET
                status = 'Processing',
                updated_at = $1
            WHERE queue.id IN (
                SELECT
                    id
                FROM queue
                WHERE status = 'Queued'
                ORDER BY queue.created_at ASC
                LIMIT 1
                FOR UPDATE
            )
            RETURNING queue.id"#
            .into(),
    };

    let query = Statement::from_sql_and_values(backend, &sql, [chrono::Utc::now().into()]);

    Job::find_by_statement(query).one(db).await
}

#[cfg(test)]
mod test {
    use sea_orm::EntityTrait;

    use super::{enqueue, Entity};
    use crate::db::{
        create_connection_by_uri,
        queue::{check_for_jobs, JobStatus},
    };

    #[tokio::test]
    async fn test_enqueue_and_dequeue() {
        let db = create_connection_by_uri("sqlite::memory:")
            .await
            .expect("Unable to connect");

        let res = enqueue(&db, "job-id", "this is the content").await;
        assert!(res.is_ok());

        // Dequeue
        let job = check_for_jobs(&db).await;
        assert!(job.is_ok());

        // Make sure job has been updated
        let job = job.unwrap().unwrap();
        let model = Entity::find_by_id(job.id).one(&db).await.unwrap();
        assert!(model.is_some());
        let model = model.unwrap();
        assert_eq!(model.status, JobStatus::Processing);
    }
}
