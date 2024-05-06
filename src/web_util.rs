use std::{io, path::PathBuf};

use ::futures::pin_mut;
use anyhow::{anyhow, Context};
use axum::{
    body::Bytes,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    BoxError,
};
use futures_util::{Stream, TryStreamExt};
use rust_embed::RustEmbed;
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

    let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
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
struct Asset;

pub struct StaticFile<T>(pub T);

impl<T> IntoResponse for StaticFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> Response {
        let path = self.0.into();

        match Asset::get(path.as_str()) {
            Some(content) => {
                let mime = mime_guess::from_path(path).first_or_octet_stream();
                ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        }
    }
}