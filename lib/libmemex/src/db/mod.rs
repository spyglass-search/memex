use migration::{Migrator, MigratorTrait};
use sea_orm::{prelude::*, ConnectOptions, Database};

pub mod document;
pub mod queue;

/// Creates a connection based on the database uri
pub async fn create_connection_by_uri(db_uri: &str) -> Result<DatabaseConnection, DbErr> {
    // See https://www.sea-ql.org/SeaORM/docs/install-and-config/connection
    // for more connection options
    let mut opt = ConnectOptions::new(db_uri.to_owned());
    opt.max_connections(10)
        .min_connections(2)
        .sqlx_logging(false);

    let db = Database::connect(opt).await?;

    Migrator::up(&db, None)
        .await
        .expect("Unable to run migrations");

    Ok(db)
}
