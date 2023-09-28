use crate::ServerError;
use jsonschema::JSONSchema;
use warp::reject::Rejection;

use super::filters::SingleQuestion;

pub async fn handle_extract(request: SingleQuestion) -> Result<warp::reply::Response, Rejection> {
    let _time = std::time::Instant::now();
    let _schema = if let Some(schema) = &request.json_schema {
        Some(
            JSONSchema::options()
                .compile(schema)
                .map_err(|err| ServerError::ClientRequestError(err.to_string()))?,
        )
    } else {
        log::warn!("No Validation");
        None
    };

    todo!()
}
