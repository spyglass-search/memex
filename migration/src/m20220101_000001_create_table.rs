use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Documents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Documents::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Documents::TaskId).string().not_null())
                    .col(ColumnDef::new(Documents::DocumentId).string().not_null())
                    .col(ColumnDef::new(Documents::Segment).big_integer().not_null())
                    .col(ColumnDef::new(Documents::Content).string().not_null())
                    .col(ColumnDef::new(Documents::Metadata).json().null())
                    .col(ColumnDef::new(Documents::CreatedAt).date_time().not_null())
                    .col(ColumnDef::new(Documents::UpdatedAt).date_time().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Queue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Queue::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Queue::Collection).string().not_null())
                    .col(ColumnDef::new(Queue::Payload).json().not_null())
                    .col(ColumnDef::new(Queue::Status).string().not_null())
                    .col(ColumnDef::new(Queue::Error).json().null())
                    .col(
                        ColumnDef::new(Queue::NumRetries)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Queue::CreatedAt).date_time().not_null())
                    .col(ColumnDef::new(Queue::UpdatedAt).date_time().not_null())
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
enum Documents {
    Table,
    Id,
    TaskId,
    DocumentId,
    Segment,
    Content,
    Metadata,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Queue {
    Table,
    Id,
    Collection,
    Payload,
    Status,
    Error,
    NumRetries,
    CreatedAt,
    UpdatedAt,
}
