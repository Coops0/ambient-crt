use std::{io, path::PathBuf};

use ::futures::pin_mut;
use anyhow::{anyhow, Context};
use axum::{
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
    BoxError,
};
use futures_util::{Stream, TryStreamExt};
use rust_embed::RustEmbed;
use simplelog::warn;
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;

use crate::video_path;

pub async fn stream_to_file<S, E>(path: &str, stream: S) -> anyhow::Result<PathBuf>
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

    let body_with_io_error = stream.map_err(io::Error::other);
    let body_reader = StreamReader::new(body_with_io_error);
    pin_mut!(body_reader);

    tokio::io::copy(&mut body_reader, &mut file)
        .await
        .context("failed to copy body to file")?;

    Ok(path)
}

pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    #[inline]
    fn into_response(self) -> Response {
        // this is dumb
        warn!("web error captured -> {:?}", self.0);

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
    #[inline]
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[derive(RustEmbed)]
#[folder = "assets/"]
pub struct Asset;
