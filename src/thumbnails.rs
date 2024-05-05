use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use futures::TryStreamExt;
use tokio::{fs, process::Command};
use tokio_stream::{wrappers::ReadDirStream, StreamExt};

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
    let video_path = Path::new(VIDEO_PATH);

    let files = fs::read_dir(video_path).await?;
    let generated_thumbs = ReadDirStream::new(files)
        .into_stream()
        .try_filter_map(|entry| async move {
            let file_type = entry.file_type().await?;
            if !file_type.is_file() {
                return Ok(None);
            }

            let thumbnail_path = thumbnail_path(entry.path());
            if thumbnail_path.exists() {
                return Ok(None);
            }

            match generate_thumbnail(&entry.path()).await {
                Ok(_) => {
                    println!("generated thumbnail: {thumbnail_path:?}");
                    Ok(Some(thumbnail_path))
                }
                Err(e) => {
                    eprintln!("failed to generate thumbnail: {e:?}");
                    Ok(None)
                }
            }
        })
        .collect::<Result<Vec<PathBuf>, std::io::Error>>()
        .await?;

    if !generated_thumbs.is_empty() {
        println!("generated thumbnails: {}", generated_thumbs.len());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thumbnail_path() {
        let video_path = PathBuf::from("uploads/video.mp4");
        let thumb_path = thumbnail_path(&video_path);

        assert_eq!(thumb_path, PathBuf::from("thumbs/video.jpg"));
    }
}
