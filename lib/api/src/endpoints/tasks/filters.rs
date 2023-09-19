use sea_orm::DatabaseConnection;
use warp::Filter;

use super::handlers;
use crate::with_db;

pub fn build(
    db: &DatabaseConnection,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("tasks" / i64)
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(handlers::handle_check_task)
}
