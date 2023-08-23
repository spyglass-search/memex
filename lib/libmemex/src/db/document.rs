use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, Set};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Eq)]
#[sea_orm(table_name = "document")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Associated task id
    pub task_id: String,
    /// NOTE: Each segment of a full piece of text are split into many "documents"
    /// And will each have a unique identifier.
    pub document_id: String,
    /// The segment number (0, 1, 2, etc) of the content
    pub segment: i64,
    /// The segmented content
    pub content: String,
    /// Any metadata associated with this segment
    pub metadata: Option<Json>,
    pub created_at: DateTimeUtc,
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
