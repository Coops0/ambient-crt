use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use simplelog::{error, info};
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

    let output = Command::new("ffmpeg")
        .arg("-i")
        .arg(video_path)
        .arg("-vf")
        .arg("scale=640:360,select='eq(pict_type,I)',thumbnail")
        .arg("-vframes")
        .arg("1")
        .arg("-qscale:v")
        .arg("9")
        .arg("-pattern_type")
        .arg("none")
        .arg("-update")
        .arg("1")
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

        info!("generating thumbnail for '{}'", entry.path().display());
        match generate_thumbnail(&entry.path()).await {
            Ok(_) => info!("generated thumbnail '{}'", thumbnail_path.display()),
            Err(e) => error!("failed to generate thumbnail: {e:?}"),
        }
    }

    Ok(())
}
