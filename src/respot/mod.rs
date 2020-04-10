pub mod player_worker;

use tokio_core::reactor::Core;
use librespot::playback::config::PlayerConfig;
use librespot::playback::audio_backend;
use futures_01::{Future, Async, Stream};
use core::task::Poll;
use librespot::core::session::Session;
use std::thread;
use tokio_signal::IoStream;
use librespot::playback::player::Player;
use crate::respot::player_worker::PlayerWorker;
use core::fmt;

#[derive(Debug)]
pub enum PlayerCommand {
    Load(String),
    Seek(u32),
    NextTrack,
    PreviousTrack,
    Stop,
    Play
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
            let player_config = PlayerConfig::default();
            let backend = audio_backend::find(None).unwrap();
            let (player, _) = Player::new(player_config, session.clone(), None, move || {
                (backend)(None)
            });

            let mut core = Core::new().unwrap();
            let player_worker = PlayerWorker::new(player, command_receiver, event_sender);

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