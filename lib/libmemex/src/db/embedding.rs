use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, Set};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "embeddings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Document this belongs to
    #[sea_orm(indexed)]
    pub document_id: String,
    /// Unique ID for this embedded segment.
    #[sea_orm(indexed)]
    pub uuid: String,
    /// The segment number (0, 1, 2, etc) of the content.
    pub segment: i64,
    /// The segmented content.
    pub content: String,
    /// The embeddeding for this segment.
    pub vector: Vec<f32>,
    /// Any metadata associated with this segment
    pub metadata: Option<Json>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::document::Entity",
        from = "Column::DocumentId",
        to = "super::document::Column::Uuid"
    )]
    Document,
}

impl Related<super::document::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Document.def()
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
