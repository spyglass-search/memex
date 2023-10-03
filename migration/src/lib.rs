pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20230919_115012_create_embedding_table;
mod m20230920_114744_add_task_type_column;
mod m20231002_201128_add_output_column;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20230919_115012_create_embedding_table::Migration),
            Box::new(m20230920_114744_add_task_type_column::Migration),
            Box::new(m20231002_201128_add_output_column::Migration),
        ]
    }
}
