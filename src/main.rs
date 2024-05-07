mod media_keys;
mod playlist;
mod thumbnails;
mod vlc_manager;
mod web_manager;
mod web_util;

use std::{
    borrow::Cow,
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

use axum::{
    http::header,
    response::{Html, IntoResponse},
    routing::get,
};
use media_keys::MediaKeyMessage;
use once_cell::sync::OnceCell;
use rust_embed::EmbeddedFile;
use simplelog::{
    info, ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode, WriteLogger,
};
use tokio::{fs, net::TcpListener};
use tower_http::services::ServeDir;
use vlc_manager::{create_vlc_channel, VlcMessage};
use web_manager::manager_router;
use web_util::Asset;

use crate::{media_keys::create_enigo_channel, thumbnails::generate_new_thumbs};

// this is for me so im not sanitizing anything
// don't use this with any public facing server or ur gonna get OWNED!!!!

pub const VIDEO_PATH: &str = "uploads/";
pub const THUMB_PATH: &str = "thumbs/";
pub const PLAYLIST_PATH: &str = "playlists/";

const DEFAULT_FLAGS: &[&str] = &[
    "--fullscreen",
    "--loop",
    "--no-video-title-show",
    "--play-and-exit",
    "--no-osd",
    "--no-volume-save",
    "--video-on-top",
    "--no-snapshot-preview",
    "--intf=dummy", // breaks stuff on macos, remove from flags.txt for testing locally
];

pub static FLAGS: OnceCell<Vec<String>> = OnceCell::new();

#[tokio::main]
async fn main() {
    tokio::task::spawn_blocking(|| {
        // have to do this since file::create is blocking
        // & this can't take tokio's asyncread
        //
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("ambient_crt.log")
            .unwrap();

        CombinedLogger::init(vec![
            TermLogger::new(
                LevelFilter::Info,
                Config::default(),
                TerminalMode::Mixed,
                ColorChoice::Auto,
            ),
            WriteLogger::new(LevelFilter::Info, Config::default(), log_file),
        ])
        .unwrap();
    })
    .await
    .unwrap();

    for path in &[VIDEO_PATH, THUMB_PATH, PLAYLIST_PATH] {
        drop(fs::create_dir(path).await);
    }

    info!("checking if need to generate new thumbnails...");
    let _ = generate_new_thumbs().await;

    let flags: Vec<String> = if let Ok(flag_file) = fs::read_to_string("flags.txt").await {
        flag_file
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    } else {
        let _ = fs::write("flags.txt", DEFAULT_FLAGS.join("\n")).await;
        DEFAULT_FLAGS.iter().map(ToString::to_string).collect()
    };

    info!("loaded {} flags", flags.len());
    let _ = FLAGS.set(flags);

    let vlc_channel = create_vlc_channel();
    let enigo_channel = create_enigo_channel();

    let app = manager_router()
        .nest_service("/thumbs", ServeDir::new(THUMB_PATH))
        .route("/", get(index))
        .route("/styles", get(styles))
        .route("/script", get(script))
        .with_state(AppState {
            vlc: vlc_channel,
            media_keys: enigo_channel,
        });

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("binded to port 3000");

    axum::serve(listener, app).await.unwrap();
}

// make this clonable since we want the senders to be cloned
// if we used an arc, then it would just share a reference
#[derive(Clone)]
pub struct AppState {
    pub vlc: Sender<VlcMessage>,
    pub media_keys: Sender<MediaKeyMessage>,
}

#[must_use]
pub fn video_path(path: &str) -> PathBuf {
    Path::new(VIDEO_PATH).join(path)
}

type EmbeddedData = Cow<'static, [u8]>;

async fn index() -> Html<EmbeddedData> {
    let EmbeddedFile { data, .. } = unsafe { Asset::get("index.html").unwrap_unchecked() };
    Html(data)
}
async fn styles() -> impl IntoResponse {
    let EmbeddedFile { data, .. } = unsafe { Asset::get("styles.css").unwrap_unchecked() };

    ([(header::CONTENT_TYPE, "text/css")], data)
}
async fn script() -> EmbeddedData {
    // js can have type of octet stream
    unsafe { Asset::get("script.js").unwrap_unchecked() }.data
}
