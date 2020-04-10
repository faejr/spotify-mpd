pub mod player_worker;

use tokio_core::reactor::Core;
use librespot::playback::config::PlayerConfig;
use librespot::playback::audio_backend;
use futures_01::{Future, Async, Stream};
use librespot::core::session::Session;
use std::thread;
use tokio_signal::IoStream;
use librespot::playback::player::Player;
use crate::respot::player_worker::PlayerWorker;
use core::fmt;
use librespot::playback::config::Bitrate::Bitrate320;

#[derive(Debug)]
pub enum PlayerCommand {
    Load(String),
    Seek(u32),
    SetVolume(u16),
    NextTrack,
    PreviousTrack,
    Stop,
    Play,
    Pause
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerEvent {
    FinishedTrack,
    Playing,
    Stopped,
    Paused
}
impl fmt::Display for PlayerEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PlayerEvent::Playing => write!(f, "play"),
            PlayerEvent::Paused => write!(f, "pause"),
            _ => write!(f, "stop"),
        }
    }
}

// Todo: How can we get a futures 0.3 compatible IoStream?
pub struct Respot {
    cancel_signal: IoStream<()>
}
impl Respot {
    pub fn new(session: Session, command_receiver: std::sync::mpsc::Receiver<PlayerCommand>, event_sender: std::sync::mpsc::Sender<PlayerEvent>) -> Self {
        let respot = Self {
            cancel_signal: Box::new(tokio_signal::ctrl_c().flatten_stream())
        };
        respot.start_player(session, command_receiver, event_sender);

        respot
    }
    fn start_player(&self, session: Session, command_receiver: std::sync::mpsc::Receiver<PlayerCommand>, event_sender: std::sync::mpsc::Sender<PlayerEvent>)  {
        thread::spawn(move || {
            let create_mixer = librespot::playback::mixer::find(Some("softvol".to_owned()))
                .expect("Unable to create softvol mixer");
            let mixer = create_mixer(None);

            let mut player_config = PlayerConfig::default();
            player_config.bitrate = Bitrate320;
            let backend = audio_backend::find(None).unwrap();
            let (player, _) = Player::new(player_config, session.clone(), mixer.get_audio_filter(), move || {
                (backend)(None)
            });

            let mut core = Core::new().unwrap();
            let player_worker = PlayerWorker::new(player, mixer, command_receiver, event_sender);

            debug!("Connected");
            core.run(futures::compat::Compat::new(player_worker)).unwrap();
        });
    }
}

impl Future for Respot {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> futures_01::Poll<(), ()> {
        loop {
            if let Async::Ready(Some(())) = self.cancel_signal.poll().unwrap() {
                debug!("Ctrl-C received");
                std::process::exit(0);
            };

            return Ok(Async::NotReady)
        }
    }
}