use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(
                        ColumnDef::new(Embeddings::Uuid)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Embeddings::DocumentId).string().not_null())
                    .col(ColumnDef::new(Embeddings::Segment).integer().not_null())
                    .col(ColumnDef::new(Embeddings::Content).string().not_null())
                    .col(ColumnDef::new(Embeddings::Vector).json_binary().not_null())
                    .col(ColumnDef::new(Embeddings::Metadata).json().null())
                    .col(ColumnDef::new(Embeddings::CreatedAt).date_time().not_null())
                    .col(ColumnDef::new(Embeddings::UpdatedAt).date_time().not_null())
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk-embeddings-document_id-documents-uuid")
                            .from_tbl(Embeddings::Table)
                            .from_col(Embeddings::DocumentId)
                            .to_tbl(Documents::Table)
                            .to_col(Documents::Uuid),
                    )
                    .to_owned(),
            )
            .await?;

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

#[derive(Iden)]
enum Documents {
    Table,
    Uuid,
}
