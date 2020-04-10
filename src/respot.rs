use tokio_core::reactor::Core;
use librespot::playback::config::PlayerConfig;
use librespot::playback::audio_backend;
use futures_01::{Future, Async, Stream};
use core::task::Poll;
use librespot::core::session::Session;
use std::thread;
use tokio_signal::IoStream;
use librespot::playback::player::Player;
use librespot::core::spotify_id::SpotifyId;
use std::pin::Pin;
use futures_01::sync::oneshot::Canceled;
use futures::task::Context;

#[derive(Debug)]
pub enum PlayerCommand {
    Load(String),
    Seek(u32),
    NextTrack,
    PreviousTrack,
    Stop,
    Play
}

#[derive(Debug)]
pub enum PlayerEvent {
    FinishedTrack,
    Playing
}

struct PlayerWorker {
    player: Player,
    command_receiver: std::sync::mpsc::Receiver<PlayerCommand>,
    event_sender: std::sync::mpsc::Sender<PlayerEvent>,
    play_task: Pin<Box<dyn futures::Future<Output = Result<(), Canceled>>>>,
    active: bool,
    cancel_signal: IoStream<()>
}

impl PlayerWorker {
    fn new(player: Player, command_receiver: std::sync::mpsc::Receiver<PlayerCommand>, event_sender: std::sync::mpsc::Sender<PlayerEvent>) -> Self {
        Self {
            player,
            command_receiver,
            event_sender,
            play_task: Box::pin(futures::future::pending()),
            active: false,
            cancel_signal: Box::new(tokio_signal::ctrl_c().flatten_stream())
        }
    }
    fn handle_event(&mut self, event: PlayerCommand) {
        match event {
            PlayerCommand::Load(id) => {
                let uri = SpotifyId::from_base62(&id).unwrap();

                Box::pin(self.player.load(uri, false, 0));
                info!("Loaded track {:?}", id);
            },
            PlayerCommand::Play => {
                self.player.play();
                self.event_sender.send(PlayerEvent::Playing).unwrap();
                self.active = true;
                info!("Starting playback");
            },
            PlayerCommand::Stop => {
                self.player.stop();
                self.active = false;
                info!("Stopping playback");
            }
            _ => {}
        }
    }
}

impl futures::Future for PlayerWorker {
    type Output = Result<(), ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> futures::task::Poll<Self::Output> {
        loop {
            let mut progress = false;

            if let Ok(io_event) = self.command_receiver.recv() {
                self.handle_event(io_event);

                progress = true;
            }

            match self.play_task.as_mut().poll(cx) {
                Poll::Ready(Ok(())) => {
                    debug!("end of track!");
                    progress = true;
                    self.event_sender.send(PlayerEvent::FinishedTrack).unwrap();
                }
                Poll::Ready(Err(Canceled)) => {
                    debug!("player task is over!");
                    self.play_task = Box::pin(futures::future::pending());
                }
                Poll::Pending => (),
            }

            if !progress {
                return Poll::Pending;
            }
        }
    }
}

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