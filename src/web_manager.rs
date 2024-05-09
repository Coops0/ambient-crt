use std::path::Path;

use anyhow::{anyhow, Context};
use axum::{
    extract::{Query, Request, State},
    routing::{get, patch},
    Json, Router,
};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use simplelog::info;
use tokio::fs;
use tokio_stream::{wrappers::ReadDirStream, StreamExt};

use crate::{
    media_keys::MediaKeyMessage,
    playlist::{self, Playlist},
    thumbnails::{generate_thumbnail, thumbnail_path},
    video_path,
    vlc_manager::VlcMessage,
    web_util::{stream_to_file, AppError},
    AppState, VIDEO_PATH,
};

pub fn manager_router() -> Router<AppState> {
    Router::new()
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
        .route("/media-control", patch(media_control))
}

#[derive(Deserialize)]
struct VideoName {
    video_name: String,
}

type WebResult<T = ()> = Result<T, AppError>;

async fn file_upload(
    Query(VideoName { video_name }): Query<VideoName>,
    request: Request,
) -> WebResult<String> {
    let path = stream_to_file(&video_name, request.into_body().into_data_stream()).await?;
    info!("uploaded file to '{}'", path.display());

    let t = generate_thumbnail(&path).await?;
    info!("generated thumbnail at '{}'", t.display());

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
    State(AppState { vlc, .. }): State<AppState>,
    Json(SwitchVideo {
        video_name,
        gain,
        visualizer,
    }): Json<SwitchVideo>,
) -> WebResult {
    let video = video_path(&video_name);
    if !video.is_file() {
        return Err(anyhow!("video not found").into());
    }

    info!("switching video to '{}'", video.display());
    vlc.send(VlcMessage::ChangeVideo {
        file_path: video,
        gain,
        visualizer,
        shuffle: false,
    })
    .context("failed to send message to vlc thread")?;

    Ok(())
}

async fn stop_video(State(AppState { vlc, .. }): State<AppState>) -> WebResult {
    vlc.send(VlcMessage::StopVideo)
        .map_err(|e| anyhow!("failed to send message to vlc thread -> {e:?}").into())
}

async fn delete_video(Json(VideoName { video_name }): Json<VideoName>) -> WebResult {
    let video_path = video_path(&video_name);
    let _ = fs::remove_file(thumbnail_path(&video_path)).await;

    info!("deleting video '{}'", video_path.display());

    for mut playlist in playlist::playlists().await? {
        if !playlist.videos.iter().any(|v| *v == video_path) {
            continue;
        }

        info!("removing video from playlist '{}'", playlist.path.display());

        playlist.videos.retain(|v| v != &video_path);
        let _ = playlist::write_playlist(&playlist).await;
    }

    info!("deleted video");

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

async fn videos() -> WebResult<Json<Vec<VideoInfo>>> {
    let files = ReadDirStream::new(fs::read_dir(VIDEO_PATH).await?)
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

async fn playlists() -> WebResult<Json<Vec<PlaylistResponse>>> {
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

#[inline]
fn playlist_name_to_file(name: &str) -> String {
    format!("{}.vlc", name.replace(' ', "_"))
}

#[derive(Deserialize)]
struct SavePlaylist {
    // e.x. 'frank ocean'
    playlist_name: String,
    // e.x. ['frank_ocean.mp4']
    videos: Vec<String>,
}

async fn save_playlist(
    Json(SavePlaylist {
        playlist_name,
        videos,
    }): Json<SavePlaylist>,
) -> WebResult {
    let processed_name = playlist_name_to_file(&playlist_name);
    let video_length = videos.len();

    let playlist = Playlist::new(&processed_name, videos);

    info!(
        "saved playlist '{}' with {video_length} videos",
        playlist_name,
    );

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
    State(AppState { vlc, .. }): State<AppState>,
    Json(PlayPlaylist {
        playlist_name,
        gain,
        visualizer,
    }): Json<PlayPlaylist>,
) -> WebResult {
    let Some(playlist_name) = playlist_name else {
        info!("shuffling all videos");
        let _ = vlc.send(VlcMessage::ChangeVideo {
            gain,
            visualizer,
            file_path: Path::new(VIDEO_PATH).to_owned(),
            shuffle: true,
        });

        return Ok(());
    };

    let file_path = playlist::playlist_path(&playlist_name_to_file(&playlist_name));
    if !file_path.is_file() {
        return Err(anyhow!("playlist not found").into());
    }

    info!("playing playlist '{}'", playlist_name);

    vlc.send(VlcMessage::ChangeVideo {
        gain,
        visualizer,
        file_path,
        shuffle: true,
    })
    .map_err(|e| anyhow!("failed to send message to vlc thread -> {e:?}").into())
}

#[derive(Deserialize)]
struct MediaControl {
    action: u8,
}

async fn media_control(
    State(AppState { media_keys, .. }): State<AppState>,
    Json(MediaControl { action }): Json<MediaControl>,
) -> WebResult {
    match action {
        0 => media_keys.send(MediaKeyMessage::PlayPause),
        1 => media_keys.send(MediaKeyMessage::Skip),
        2 => media_keys.send(MediaKeyMessage::Previous),
        _ => return Err(anyhow!("invalid action").into()),
    }
    .map_err(|e| anyhow!("failed to send message to media keys thread -> {e:?}").into())
}
