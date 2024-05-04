mod vlc_manager;
mod web_manager;

use std::path::{Path, PathBuf};

use once_cell::sync::OnceCell;
use tokio::{fs, net::TcpListener};
use vlc_manager::launch_vlc_thread;
use web_manager::manager_router;

// this is for me so im not sanitizing anything
// don't use this with any public facing server or ur gonna get OWNED!!!!

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
    let _ = fs::create_dir("uploads/").await;

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

    println!("got flags = {flags:?}");
    FLAGS.set(flags).unwrap();

    let video_sender = launch_vlc_thread();
    let app = manager_router().with_state(video_sender);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[must_use]
pub fn video_path(path: &str) -> PathBuf {
    Path::new("uploads/").join(path)
}
