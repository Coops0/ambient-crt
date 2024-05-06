mod playlist;
mod thumbnails;
mod vlc_manager;
mod web_manager;
mod web_util;

use std::path::{Path, PathBuf};

use once_cell::sync::OnceCell;
use tokio::{fs, net::TcpListener};
use tower_http::services::ServeDir;
use vlc_manager::launch_vlc_thread;
use web_manager::manager_router;

use crate::thumbnails::generate_new_thumbs;

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
    "--intf=dummy",
];

pub static FLAGS: OnceCell<Vec<String>> = OnceCell::new();

#[tokio::main]
async fn main() {
    for path in &[VIDEO_PATH, THUMB_PATH, PLAYLIST_PATH] {
        let _ = fs::create_dir(path).await;
    }

    let _ = generate_new_thumbs().await;

    let flags: Vec<String> = match fs::read_to_string("flags.txt").await {
        Ok(flag_file) => flag_file
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect(),
        _ => {
            let _ = fs::write("flags.txt", DEFAULT_FLAGS.join("\n")).await;
            DEFAULT_FLAGS.iter().map(ToString::to_string).collect()
        }
    };

    println!("got flags = {flags:?}");
    let _ = FLAGS.set(flags);

    let app = manager_router()
        .nest_service("/thumbs", ServeDir::new(THUMB_PATH))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(launch_vlc_thread());

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[must_use]
pub fn video_path(path: &str) -> PathBuf {
    Path::new(VIDEO_PATH).join(path)
}
