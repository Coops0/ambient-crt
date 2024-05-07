use std::path::PathBuf;

use futures::TryStreamExt;
use tokio::fs;
use tokio_stream::{wrappers::ReadDirStream, StreamExt};

use crate::{video_path, PLAYLIST_PATH};

// should end with .vlc
pub fn playlist_path(playlist_file: &str) -> PathBuf {
    let mut path = PathBuf::from(PLAYLIST_PATH);
    path.push(playlist_file);

    path
}

pub struct Playlist {
    pub videos: Vec<PathBuf>,
    pub path: PathBuf,
}

impl Playlist {
    pub fn new(name: &str, videos: Vec<String>) -> Self {
        let path = playlist_path(name);
        Self {
            videos: videos.into_iter().map(|f| video_path(&f)).collect(),
            path,
        }
    }
}

pub async fn playlists() -> anyhow::Result<Vec<Playlist>> {
    let files = fs::read_dir(PLAYLIST_PATH).await?;
    let playlists = ReadDirStream::new(files)
        .into_stream()
        .try_filter_map(|entry| async move {
            let file_type = entry.file_type().await?;
            if !file_type.is_file() {
                return Ok(None);
            }

            let path = entry.path();
            let playlist = read_playlist(&path).await?;
            if playlist.videos.is_empty() {
                return Ok(None);
            }

            Ok(Some(playlist))
        })
        .collect::<Result<Vec<Playlist>, std::io::Error>>()
        .await?;

    Ok(playlists)
}

pub async fn read_playlist(path: &PathBuf) -> std::io::Result<Playlist> {
    let playlist = fs::read_to_string(path).await?;
    let files: Vec<PathBuf> = playlist
        .lines()
        .map(|line| line.strip_prefix("../").unwrap_or(line))
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect();

    Ok(Playlist {
        videos: files,
        path: path.to_owned(),
    })
}

pub async fn write_playlist(playlist: &Playlist) -> std::io::Result<()> {
    let _ = fs::remove_file(&playlist.path).await;

    if playlist.videos.is_empty() {
        return Ok(());
    }

    let files = playlist
        .videos
        .iter()
        .map(|p| format!("../{}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(&playlist.path, files).await
}
