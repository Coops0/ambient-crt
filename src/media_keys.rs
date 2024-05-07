use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use enigo::{Key, Keyboard, Settings};
use simplelog::{error, info};

#[derive(Debug)]
pub enum MediaKeyMessage {
    PlayPause,
    Skip,
    Previous,
    CustomKey(enigo::Key),
}

pub fn create_enigo_channel() -> Sender<MediaKeyMessage> {
    let (send, rec) = mpsc::channel::<MediaKeyMessage>();
    let _ = thread::spawn(move || thread_worker(&rec));

    send
}

fn thread_worker(rec: &Receiver<MediaKeyMessage>) {
    let mut enigo = enigo::Enigo::new(&Settings::default()).expect("failed to initalize enigo");

    while let Ok(msg) = rec.recv() {
        info!("running media key message {msg:?}");

        let m = match msg {
            MediaKeyMessage::PlayPause => enigo.key(Key::MediaPlayPause, enigo::Direction::Click),
            MediaKeyMessage::Skip => enigo.key(Key::MediaNextTrack, enigo::Direction::Click),
            MediaKeyMessage::Previous => enigo.key(Key::MediaPrevTrack, enigo::Direction::Click),
            MediaKeyMessage::CustomKey(key) => enigo.key(key, enigo::Direction::Click),
        };

        if let Err(err) = m {
            error!("failed to send media key: {:?}", err);
        }
    }
}
