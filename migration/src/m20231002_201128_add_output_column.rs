use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("queue", "task_output").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Queue::Table)
                        .add_column(ColumnDef::new(Queue::TaskOutput).json().null())
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
    TaskOutput,
}
