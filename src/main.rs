mod vlc_manager;
mod web_manager;

use std::path::{Path, PathBuf};

use tokio::net::TcpListener;
use vlc_manager::launch_vlc_thread;
use web_manager::manager_router;

// this is for me so im not sanitizing anything
// don't use this with any public facing server or ur gonna get OWNED!!!!

#[tokio::main]
async fn main() {
    let _ = tokio::fs::create_dir("uploads/").await;

    let video_sender = launch_vlc_thread();
    let app = manager_router().with_state(video_sender);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[must_use]
pub fn video_path(path: &str) -> PathBuf {
    Path::new("uploads/").join(path)
}
