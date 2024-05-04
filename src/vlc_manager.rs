use anyhow::Context;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::mpsc::{self, Receiver, Sender};

pub enum ThreadMessage {
    StopVideo,
    ChangeVideo {
        path: PathBuf,
        muted: bool,
        visualizer: Option<String>,
    },
}

pub fn launch_vlc_thread() -> Sender<ThreadMessage> {
    let (send, rec) = mpsc::channel::<ThreadMessage>();
    let _ = std::thread::spawn(move || thread_worker(&rec));
    send
}

fn thread_worker(rec: &Receiver<ThreadMessage>) {
    let mut current_vlc_instance = None::<Child>;

    while let Ok(msg) = rec.recv() {
        match msg {
            ThreadMessage::ChangeVideo {
                path,
                muted,
                visualizer,
            } => {
                if let Some(mut vlc) = current_vlc_instance.take() {
                    let _ = vlc.kill();
                }

                let vlc_instance =
                    play_video(&path, muted, &visualizer).expect("failed to play video");

                current_vlc_instance = Some(vlc_instance);
            }
            ThreadMessage::StopVideo => {
                if let Some(mut vlc) = current_vlc_instance.take() {
                    let _ = vlc.kill();
                }
            }
        }
    }
}

fn play_video(path: &PathBuf, muted: bool, visualizer: &Option<String>) -> anyhow::Result<Child> {
    let mut vlc_builder = Command::new("vlc");

    vlc_builder
        .arg("--fullscreen")
        .arg("--loop")
        .arg("--no-video-title-show")
        .arg("--play-and-exit")
        .arg("--no-osd")
        .arg("--no-volume-save")
        // .arg("--intf=dummy")
        .arg("--video-on-top")
        .arg("--macosx-continue-playback=2") // ?? not working
        .arg("--no-snapshot-preview");

    if muted {
        vlc_builder.arg("--gain=0");
    }

    if let Some(vis) = visualizer {
        vlc_builder
            .arg("--audio-visual=visual")
            .arg(format!("--effect-list={vis}"));
    }

    vlc_builder.arg(path);

    let instance = vlc_builder
        .spawn()
        .with_context(|| format!("Failed to launch VLC with video: {path:?}"))?;

    Ok(instance)
}
