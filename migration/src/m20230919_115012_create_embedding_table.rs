use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();

        if backend == DatabaseBackend::Sqlite {
            manager
                .create_table(
                    Table::create()
                        .table(Embeddings::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(Embeddings::Id)
                                .integer()
                                .not_null()
                                .auto_increment()
                                .primary_key(),
                        )
                        .col(ColumnDef::new(Embeddings::Uuid).string().not_null())
                        .col(ColumnDef::new(Embeddings::DocumentId).string().not_null())
                        .col(ColumnDef::new(Embeddings::Segment).integer().not_null())
                        .col(ColumnDef::new(Embeddings::Content).string().not_null())
                        .col(ColumnDef::new(Embeddings::Vector).json().not_null())
                        .col(ColumnDef::new(Embeddings::Metadata).json().null())
                        .col(ColumnDef::new(Embeddings::CreatedAt).date_time().not_null())
                        .col(ColumnDef::new(Embeddings::UpdatedAt).date_time().not_null())
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .create_table(
                    Table::create()
                        .table(Embeddings::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(Embeddings::Id)
                                .integer()
                                .not_null()
                                .auto_increment()
                                .primary_key(),
                        )
                        .col(ColumnDef::new(Embeddings::Uuid).string().not_null())
                        .col(ColumnDef::new(Embeddings::DocumentId).string().not_null())
                        .col(ColumnDef::new(Embeddings::Segment).integer().not_null())
                        .col(ColumnDef::new(Embeddings::Content).string().not_null())
                        .col(
                            ColumnDef::new(Embeddings::Vector)
                                .array(ColumnType::Float)
                                .not_null(),
                        )
                        .col(ColumnDef::new(Embeddings::Metadata).json().null())
                        .col(ColumnDef::new(Embeddings::CreatedAt).date_time().not_null())
                        .col(ColumnDef::new(Embeddings::UpdatedAt).date_time().not_null())
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Embeddings {
    Table,
    Id,
    Uuid,
    DocumentId,
    Segment,
    Content,
    Vector,
    Metadata,
    CreatedAt,
    UpdatedAt,
}
