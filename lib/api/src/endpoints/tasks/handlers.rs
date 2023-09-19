use crate::{schema::TaskResult, ServerError};
use libmemex::db::queue;
use sea_orm::{DatabaseConnection, EntityTrait};

pub async fn handle_check_task(
    task_id: i64,
    db: DatabaseConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    let result = match queue::Entity::find_by_id(task_id).one(&db).await {
        Ok(res) => res,
        Err(err) => return Err(warp::reject::custom(ServerError::DatabaseError(err))),
    };

    match result {
        Some(result) => Ok(warp::reply::json(&TaskResult::from(result))),
        None => Err(warp::reject::not_found()),
    }
}
