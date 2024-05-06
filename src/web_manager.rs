use std::{
    fmt::format,
    io,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

use ::futures::pin_mut;
use anyhow::{anyhow, Context};
use axum::{
    body::Bytes,
    extract::{Query, Request, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    BoxError, Json, Router,
};
use futures_util::{Stream, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    fs::{self, File},
    io::BufWriter,
};
use tokio_stream::{wrappers::ReadDirStream, StreamExt};
use tokio_util::io::StreamReader;

use crate::{
    playlist::{self, Playlist},
    thumbnails::{generate_thumbnail, thumbnail_path},
    video_path,
    vlc_manager::ThreadMessage,
    VIDEO_PATH,
};

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
        .route(
            "/playlists",
            get(playlists).post(save_playlist).put(play_playlist),
        )
}

async fn panel() -> Html<&'static str> {
    Html(include_str!("../assets/index.html"))
}

#[derive(Deserialize)]
struct VideoName {
    video_name: String,
}

async fn file_upload(
    Query(VideoName { video_name }): Query<VideoName>,
    request: Request,
) -> Result<String, AppError> {
    let path = stream_to_file(&video_name, request.into_body().into_data_stream()).await?;

    println!("uploaded file to {path:?}");

    let _ = generate_thumbnail(&path).await?;

    let path_string = path
        .to_str()
        .context("failed to convert path -> string")?
        .to_string();

    Ok(path_string)
}

#[derive(Deserialize)]
struct SwitchVideo {
    video_name: String,
    #[serde(default)]
    gain: f32,
    visualizer: Option<String>,
}

async fn switch_video(
    State(video_sender): State<Sender<ThreadMessage>>,
    Json(SwitchVideo {
        video_name,
        gain,
        visualizer,
    }): Json<SwitchVideo>,
) -> Result<(), AppError> {
    video_sender
        .send(ThreadMessage::ChangeVideo {
            file_path: video_path(&video_name),
            gain,
            visualizer,
            shuffle: false,
        })
        .context("failed to send message to vlc thread")?;

    Ok(())
}

async fn stop_video(State(video_sender): State<Sender<ThreadMessage>>) -> Result<(), AppError> {
    video_sender
        .send(ThreadMessage::StopVideo)
        .map_err(|e| anyhow!("failed to send message to vlc thread -> {e:?}").into())
}

async fn delete_video(Json(VideoName { video_name }): Json<VideoName>) -> Result<(), AppError> {
    let video_path = video_path(&video_name);
    let _ = fs::remove_file(thumbnail_path(&video_path)).await;

    fs::remove_file(video_path)
        .await
        .map_err(|e| anyhow!("failed to delete video -> {e:?}").into())
}

#[derive(Serialize)]
struct VideoInfo {
    size: u64,
    name: String,
    name_without_ext: String,
}

async fn videos() -> Result<Json<Vec<VideoInfo>>, AppError> {
    let path = Path::new(VIDEO_PATH);

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

            let name_without_ext = entry
                .path()
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let size = metadata.len();
            Ok(Some(VideoInfo {
                size,
                name,
                name_without_ext,
            }))
        })
        .collect::<Result<Vec<VideoInfo>, std::io::Error>>()
        .await?;

    Ok(Json(files))
}

#[derive(Serialize)]
struct PlaylistResponse {
    // name without ext or path
    name: String,
    // name as seen above for videos
    videos: Vec<String>,
}

async fn playlists() -> Result<Json<Vec<PlaylistResponse>>, AppError> {
    let files = playlist::playlists()
        .await?
        .into_iter()
        .map(|p| PlaylistResponse {
            // ugh
            name: p
                .path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            videos: p
                .videos
                .iter()
                .filter_map(|v| Some(v.file_name()?.to_string_lossy().to_string()))
                .collect(),
        })
        .collect();

    Ok(Json(files))
}

#[derive(Deserialize)]
struct SavePlaylist {
    // e.x. 'frank ocean'
    playlist_name: String,
    // e.x. ['frank_ocean.mp4']
    videos: Vec<String>,
}

fn playlist_name_to_file(name: &str) -> String {
    format!("{}.vlc", name.replace(' ', "_"))
}

async fn save_playlist(
    Json(SavePlaylist {
        playlist_name,
        videos,
    }): Json<SavePlaylist>,
) -> Result<(), AppError> {
    let processed_name = playlist_name_to_file(&playlist_name);
    let playlist = Playlist::new(processed_name, videos);

    playlist::write_playlist(&playlist)
        .await
        .map_err(Into::into)
}

#[derive(Deserialize)]
struct PlayPlaylist {
    playlist_name: Option<String>,
    #[serde(default)]
    gain: f32,
    visualizer: Option<String>,
}

async fn play_playlist(
    State(video_sender): State<Sender<ThreadMessage>>,
    Json(PlayPlaylist {
        playlist_name,
        gain,
        visualizer,
    }): Json<PlayPlaylist>,
) -> Result<(), AppError> {
    let playlist_name = match playlist_name {
        Some(name) => name,
        None => {
            let _ = video_sender.send(ThreadMessage::ChangeVideo {
                gain,
                visualizer,
                file_path: Path::new(VIDEO_PATH).to_owned(),
                shuffle: true,
            });

            return Ok(());
        }
    };

    let file_path = playlist::playlist_path(&playlist_name_to_file(&playlist_name));
    if !file_path.is_file() {
        return Err(anyhow!("playlist not found").into());
    }

    video_sender
        .send(ThreadMessage::ChangeVideo {
            gain,
            visualizer,
            file_path,
            shuffle: true,
        })
        .map_err(|e| anyhow!("failed to send message to vlc thread -> {e:?}").into())
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
