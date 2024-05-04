use std::path::Path;
use std::{path::PathBuf, sync::mpsc::Sender};

use ::futures::pin_mut;
use anyhow::anyhow;
use anyhow::Context;
use axum::Json;
use axum::{
    body::Bytes,
    extract::{Query, Request, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    BoxError, Router,
};
use futures_util::{Stream, TryStreamExt};
use serde::Deserialize;
use serde::Serialize;
use tokio::fs;
use tokio::{fs::File, io::BufWriter};
use tokio_stream::wrappers::ReadDirStream;
use tokio_stream::StreamExt;

use std::io;
use tokio_util::io::StreamReader;

use crate::{video_path, vlc_manager::ThreadMessage};

pub fn manager_router() -> Router<Sender<ThreadMessage>> {
    Router::new()
        .route("/", get(panel))
        .route("/stop", get(stop_video))
        .route(
            "/videos",
            get(videos)
                .post(file_upload)
                .delete(delete_video)
                .put(switch_video),
        )
}

async fn panel() -> Html<&'static str> {
    Html(include_str!("../templates/panel.html"))
}

#[derive(Deserialize)]
struct FileName {
    file_name: String,
}

async fn file_upload(
    Query(FileName { file_name }): Query<FileName>,
    request: Request,
) -> Result<String, AppError> {
    let path = stream_to_file(&file_name, request.into_body().into_data_stream()).await?;

    println!("Uploaded file to {path:?}");

    let path_string = path
        .to_str()
        .context("failed to convert path -> string")?
        .to_string();

    Ok(path_string)
}

#[derive(Deserialize)]
struct SwitchVideoParams {
    file_name: String,
    #[serde(default)]
    gain: f32,
    visualizer: Option<String>,
}

async fn switch_video(
    State(video_sender): State<Sender<ThreadMessage>>,
    Json(SwitchVideoParams {
        file_name,
        gain,
        visualizer,
    }): Json<SwitchVideoParams>,
) -> Result<(), AppError> {
    let path = video_path(&file_name);
    if !path.is_file() {
        return Err(anyhow!("file not found").into());
    }

    video_sender
        .send(ThreadMessage::ChangeVideo {
            path,
            gain,
            visualizer,
        })
        .map_err(|_| anyhow!("failed to send message to vlc thread"))?;

    Ok(())
}

async fn stop_video(State(video_sender): State<Sender<ThreadMessage>>) -> Result<(), AppError> {
    video_sender
        .send(ThreadMessage::StopVideo)
        .map_err(|_| anyhow!("failed to send message to vlc thread").into())
}

async fn delete_video(Json(FileName { file_name }): Json<FileName>) -> Result<(), AppError> {
    let path = video_path(&file_name);
    fs::remove_file(path).await?;

    Ok(())
}

#[derive(Serialize)]
struct VideoInfo {
    size: u64,
    name: String,
}

async fn videos() -> Result<Json<Vec<VideoInfo>>, AppError> {
    let path = Path::new("./uploads/");

    let files = ReadDirStream::new(fs::read_dir(path).await?)
        .into_stream()
        .try_filter_map(|entry| async move {
            let file_type = entry.file_type().await?;
            if !file_type.is_file() {
                return Ok(None);
            }

            let metadata = entry.metadata().await?;
            let Ok(name) = entry.file_name().into_string() else {
                return Ok(None);
            };

            let size = metadata.len();
            Ok(Some(VideoInfo { size, name }))
        })
        .collect::<Result<Vec<VideoInfo>, std::io::Error>>()
        .await?;

    Ok(Json(files))
}

async fn stream_to_file<S, E>(path: &str, stream: S) -> anyhow::Result<PathBuf>
where
    S: Stream<Item = Result<Bytes, E>> + Send,
    E: Into<BoxError>,
{
    let path = video_path(path);

    let mut file = BufWriter::new(
        File::create(&path)
            .await
            .map_err(|_| anyhow!("failed to create file {path:?}"))?,
    );

    let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
    let body_reader = StreamReader::new(body_with_io_error);
    pin_mut!(body_reader);

    tokio::io::copy(&mut body_reader, &mut file)
        .await
        .context("failed to copy body to file")?;

    Ok(path)
}

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
