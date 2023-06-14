use sea_orm::{prelude::*, ConnectOptions, Database, DatabaseBackend, ExecResult, Schema};

pub mod document;
pub mod queue;

async fn make_table(
    db: &DatabaseConnection,
    backend: &DatabaseBackend,
    schema: &Schema,
    entity: impl EntityTrait,
) -> Result<ExecResult, sea_orm::DbErr> {
    db.execute(backend.build(schema.create_table_from_entity(entity).if_not_exists()))
        .await
}

#[allow(dead_code)]
pub async fn setup_schema(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    if cfg!(not(debug_assertions)) {
        panic!("only use for dev/tests");
    }

    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    make_table(db, &backend, &schema, document::Entity).await?;
    make_table(db, &backend, &schema, queue::Entity).await?;

    Ok(())
}

/// Creates a connection based on the database uri
pub async fn create_connection_by_uri(db_uri: &str) -> Result<DatabaseConnection, DbErr> {
    // See https://www.sea-ql.org/SeaORM/docs/install-and-config/connection
    // for more connection options
    let mut opt = ConnectOptions::new(db_uri.to_owned());
    opt.max_connections(10)
        .min_connections(2)
        .sqlx_logging(false);

    let db = Database::connect(opt).await?;

    #[cfg(debug_assertions)]
    setup_schema(&db).await.expect("Unable to setup schema");

    Ok(db)
}
