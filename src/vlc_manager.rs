use anyhow::{Context, Result};
use std::{
    path::PathBuf,
    process::{Child, Command},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use crate::FLAGS;

pub enum VlcMessage {
    StopVideo,
    ChangeVideo {
        file_path: PathBuf,
        gain: f32,
        visualizer: Option<String>,
        shuffle: bool,
    },
    PlayFromString {
        media: String,
        gain: f32,
        visualizer: Option<String>,
    },
}

pub fn create_vlc_channel() -> Sender<VlcMessage> {
    let (send, rec) = mpsc::channel::<VlcMessage>();
    let _ = thread::spawn(move || thread_worker(&rec));

    send
}

fn thread_worker(rec: &Receiver<VlcMessage>) {
    let mut current_vlc_instance = None::<Child>;

    while let Ok(msg) = rec.recv() {
        // only ok since 2 msgs, might have to chage if more
        if let Some(mut vlc) = current_vlc_instance.take() {
            let _ = vlc.kill();
        }

        match msg {
            VlcMessage::ChangeVideo {
                file_path: path,
                gain,
                visualizer,
                shuffle,
            } => {
                let vlc_instance = play_video(
                    path.to_str().unwrap_or_default(),
                    gain,
                    &visualizer,
                    shuffle,
                )
                .expect("failed to play video");

                current_vlc_instance = Some(vlc_instance);
            }
            VlcMessage::PlayFromString {
                media,
                gain,
                visualizer,
            } => {
                let vlc_instance = play_video(&media, gain, &visualizer, false)
                    .expect("failed to play video from string");

                current_vlc_instance = Some(vlc_instance);
            }
            VlcMessage::StopVideo => {
                // already handled
            }
        }
    }
}

fn play_video(path: &str, gain: f32, visualizer: &Option<String>, shuffle: bool) -> Result<Child> {
    let mut vlc_builder = Command::new("vlc");

    for flag in unsafe { FLAGS.get_unchecked() } {
        vlc_builder.arg(flag);
    }

    vlc_builder.arg(format!("--gain={gain}"));

    if let Some(vis) = visualizer {
        vlc_builder
            .arg("--audio-visual=visual")
            .arg(format!("--effect-list={vis}"));
    }

    if shuffle {
        vlc_builder.arg("--random");
    }

    vlc_builder
        .arg(path)
        .spawn()
        .with_context(|| format!("Failed to launch VLC with video: {path:?}"))
}
