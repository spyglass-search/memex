use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("queue", "task_type").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Queue::Table)
                        .add_column(
                            ColumnDef::new(Queue::TaskType)
                                .string()
                                .not_null()
                                .default("Ingest"),
                        )
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
enum Queue {
    Table,
    TaskType,
}
