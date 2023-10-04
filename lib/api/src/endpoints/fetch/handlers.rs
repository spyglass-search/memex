use crate::{schema::ApiResponse, ServerError};
use futures_util::TryStreamExt;
use warp::filters::multipart::FormData;
use warp::Buf;

use super::filters;

// When memex is running inside the docker image.
#[cfg(not(debug_assertions))]
const EXE_NAME: &str = "/usr/local/bin/pdftotext";
#[cfg(not(debug_assertions))]
const DATA_DIR: &str = "/tmp";

// In debug mode or running locally
#[cfg(all(target_os = "windows", debug_assertions))]
const EXE_NAME: &str = "./resources/utils/win/pdftotext.exe";
#[cfg(all(target_os = "macos", debug_assertions))]
const EXE_NAME: &str = "./resources/utils/mac/pdftotext";
#[cfg(all(target_os = "linux", debug_assertions))]
const EXE_NAME: &str = "./resources/utils/linux/pdftotext";
#[cfg(debug_assertions)]
const DATA_DIR: &str = "./uploads";

pub async fn handle_fetch(
    query: filters::FetchRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let time = std::time::Instant::now();

    if let Some(url) = query.url {
        let content = reqwest::get(url)
            .await
            .map_err(|err| ServerError::Other(err.to_string()))?
            .text()
            .await
            .map_err(|err| ServerError::Other(err.to_string()))?;

        Ok(warp::reply::json(&ApiResponse::success(
            time.elapsed(),
            Some(serde_json::json!({ "content": content })),
        )))
    } else {
        Err(warp::reject())
    }
}

pub async fn handle_parse(form: FormData) -> Result<impl warp::Reply, warp::Rejection> {
    let time = std::time::Instant::now();

    let field_names: Vec<_> = form
        .and_then(|mut field| async move {
            let mut bytes: Vec<u8> = Vec::new();
            let content_type = field.content_type().map(|f| f.to_string());

            // field.data() only returns a piece of the content, you should call over it until it replies None
            // TODO we should stream directly to s3 instead of memory
            while let Some(content) = field.data().await {
                let content = content.unwrap();
                let chunk: &[u8] = content.chunk();
                bytes.extend_from_slice(chunk);
            }

            Ok((field.name().to_string(), content_type, bytes))
        })
        .try_collect()
        .await
        .map_err(|e| ServerError::Other(e.to_string()))?;

    let (content_type, data) = field_names
        .iter()
        .find_map(|(field, content_type, data)| {
            if field == "file" {
                Some((content_type, data))
            } else {
                None
            }
        })
        .ok_or(ServerError::Other("Invalid request".to_string()))?;

    let file_ending = match content_type {
        Some(content) => {
            if content == "application/pdf" || content == "application" {
                "pdf"
            } else {
                return Err(ServerError::Other("File type not supported".to_string()).into());
            }
        }
        _ => {
            return Err(ServerError::Other("File type not supported".to_string()).into());
        }
    };

    let file_id = uuid::Uuid::new_v4();
    let filename = format!("{DATA_DIR}/{}.{}", file_id, file_ending);
    let parsed_output = format!("{DATA_DIR}/{}.txt", file_id);

    log::debug!("saving file to {filename}");
    tokio::fs::write(&filename, data)
        .await
        .map_err(|e| ServerError::Other(e.to_string()))?;

    // Run pdftotext on the sucker
    let mut cmd = tokio::process::Command::new(EXE_NAME);
    cmd.arg("-q")
        .arg("-nopgbrk")
        .arg("-enc")
        .arg("UTF-8")
        .arg(filename.clone())
        .arg(parsed_output.clone());

    log::debug!("running command: {:?}", cmd);
    let parsed = match cmd.spawn() {
        Ok(mut child) => {
            if let Err(err) = child.wait().await {
                return Err(ServerError::Other(err.to_string()).into());
            } else {
                // Read results
                let bytes = tokio::fs::read(parsed_output.clone())
                    .await
                    .map_err(|e| ServerError::Other(e.to_string()))?;
                String::from_utf8_lossy(&bytes).to_string()
            }
        }
        Err(err) => {
            return Err(ServerError::Other(err.to_string()).into());
        }
    };

    // Remove tmp files after success parse
    let _ = std::fs::remove_file(filename);
    let _ = std::fs::remove_file(parsed_output);

    Ok(warp::reply::json(&ApiResponse::success(
        time.elapsed(),
        Some(serde_json::json!({ "parsed": parsed })),
    )))
}
