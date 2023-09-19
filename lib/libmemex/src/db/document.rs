use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, Set};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "documents")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// NOTE: Each segment of a full piece of text are split into many "documents"
    /// And will each have a unique identifier.
    #[sea_orm(indexed, unique)]
    pub uuid: String,
    /// Associated task id from the queue.
    #[sea_orm(indexed)]
    pub task_id: i64,
    /// The full text context of this document
    pub content: String,
    /// Any additional metadata associated with this document.
    pub metadata: Option<Json>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::embedding::Entity")]
    Embedding,
    #[sea_orm(
        belongs_to = "super::queue::Entity",
        from = "Column::TaskId",
        to = "super::queue::Column::Id"
    )]
    Task,
}

impl Related<super::embedding::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Embedding.def()
    }
}

impl Related<super::queue::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Task.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
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

impl ActiveModel {
    pub fn from_task(task: &super::queue::Model) -> Self {
        let uuid = uuid::Uuid::new_v5(&crate::NAMESPACE, task.id.to_string().as_bytes());

        Self {
            uuid: Set(uuid.to_string()),
            content: Set(task.payload.content.clone()),
            task_id: Set(task.id),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }
}
