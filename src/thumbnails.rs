use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use tokio::{fs, process::Command};

use crate::{THUMB_PATH, VIDEO_PATH};

// get path of uploads/video.mp4
// return thumbs/video.jpg
#[must_use]
pub fn thumbnail_path<P: AsRef<Path>>(video_path: P) -> PathBuf {
    let video_path: &Path = video_path.as_ref();

    let mut thumb_path = PathBuf::from(THUMB_PATH);
    thumb_path.push(video_path.file_name().unwrap_or_default());
    thumb_path.set_extension("jpg");

    thumb_path
}

pub async fn generate_thumbnail(video_path: &PathBuf) -> anyhow::Result<PathBuf> {
    let thumbnail_path = thumbnail_path(video_path);

    // ffmpeg -ss 0 -i 3005.mp4 -vf "select='eq(pict_type,I)',scale=640:360,thumbnail" -frames:v 1 -qscale:v 2 output.jpg

    let output = Command::new("ffmpeg")
        .arg("-i")
        .arg(video_path)
        .arg("-ss")
        .arg("0")
        .arg("-vf")
        .arg("select='eq(pict_type,I)',thumbnail")
        .arg("-frames:v")
        .arg("1")
        .arg("-qscale:v")
        .arg("2")
        .arg(&thumbnail_path)
        .output()
        .await
        .context("failed to generate thumbnail")?;

    if !output.status.success() {
        bail!("ffmpeg failed: {:?}", output);
    }

    Ok(thumbnail_path)
}

pub async fn generate_new_thumbs() -> anyhow::Result<()> {
    let mut files = fs::read_dir(VIDEO_PATH).await?;
    while let Some(entry) = files.next_entry().await? {
        let file_type = entry.file_type().await?;
        if !file_type.is_file() {
            continue;
        }

        let thumbnail_path = thumbnail_path(entry.path());
        if thumbnail_path.exists() {
            continue;
        }

        match generate_thumbnail(&entry.path()).await {
            Ok(_) => println!("generated thumbnail: {thumbnail_path:?}"),
            Err(e) => eprintln!("failed to generate thumbnail: {e:?}"),
        }
    }

    Ok(())
}
