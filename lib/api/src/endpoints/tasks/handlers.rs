use crate::{
    schema::{ApiResponse, TaskResult},
    ServerError,
};
use libmemex::db::queue;
use sea_orm::{DatabaseConnection, EntityTrait};

pub async fn handle_check_task(
    task_id: i64,
    db: DatabaseConnection,
) -> Result<impl warp::Reply, warp::Rejection> {
    let time = std::time::Instant::now();
    let result = match queue::Entity::find_by_id(task_id).one(&db).await {
        Ok(res) => res,
        Err(err) => return Err(warp::reject::custom(ServerError::DatabaseError(err))),
    };

    match result {
        Some(result) => {
            let result = TaskResult::from(result);
            Ok(warp::reply::json(&ApiResponse::success(
                time.elapsed(),
                Some(result),
            )))
        }
        None => Err(warp::reject::not_found()),
    }
}
