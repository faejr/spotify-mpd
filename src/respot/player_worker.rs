use crate::respot::{PlayerCommand, PlayerEvent};
use librespot::playback::player::Player;
use librespot::core::spotify_id::SpotifyId;
use futures::task::{Context, Poll};
use std::pin::Pin;
use librespot::playback::mixer::Mixer;
use futures::channel::mpsc;
use futures::{Stream, Future};
use futures::compat::Future01CompatExt;
use futures_01::sync::oneshot::Canceled;

pub struct PlayerWorker {
    player: Player,
    command_receiver: Pin<Box<mpsc::UnboundedReceiver<PlayerCommand>>>,
    event_sender: std::sync::mpsc::Sender<PlayerEvent>,
    play_task: Pin<Box<dyn Future<Output=Result<(), Canceled>>>>,
    active: bool,
    mixer: Box<dyn Mixer>,
}

impl PlayerWorker {
    pub fn new(player: Player, mixer: Box<dyn Mixer>, command_receiver: mpsc::UnboundedReceiver<PlayerCommand>, event_sender: std::sync::mpsc::Sender<PlayerEvent>) -> Self {
        Self {
            player,
            command_receiver: Box::pin(command_receiver),
            event_sender,
            play_task: Box::pin(futures::future::pending()),
            active: false,
            mixer,
        }
    }
    fn handle_event(&mut self, event: PlayerCommand) {
        match event {
            PlayerCommand::Load(id) => {
                let uri = SpotifyId::from_base62(&id).unwrap();

                self.play_task = Box::pin(self.player.load(uri, false, 0).compat());
                info!("Loaded track {:?}", id);
            }
            PlayerCommand::Play => {
                self.player.play();
                self.event_sender.send(PlayerEvent::Playing).unwrap();
                self.active = true;
                info!("Starting playback");
            }
            PlayerCommand::Pause => {
                self.player.pause();
                self.event_sender.send(PlayerEvent::Paused).unwrap();
                self.active = false;
                info!("pausing playback");
            }
            PlayerCommand::Stop => {
                self.player.stop();
                self.active = false;
                info!("Stopping playback");
            }
            PlayerCommand::SetVolume(vol) => {
                self.mixer.set_volume(Self::calc_logarithmic_volume(vol));
            }
            _ => {}
        }
    }

    fn calc_logarithmic_volume(volume: u16) -> u16 {
        let mixer_volume = ((std::cmp::min(volume, 100) as f32) / 100.0 * 65535_f32).ceil() as u16;
        // Volume conversion taken from https://github.com/plietar/librespot/blob/master/src/spirc.rs
        const IDEAL_FACTOR: f64 = 6.908;
        let normalized_volume = mixer_volume as f64 / std::u16::MAX as f64;

        let val = if normalized_volume < 0.999 {
            let new_volume = (normalized_volume * IDEAL_FACTOR).exp() / 1000.0;
            (new_volume * std::u16::MAX as f64) as u16
        } else {
            std::u16::MAX
        };

        debug!("input volume:{} to mixer: {}", volume, val);

        val
    }
}

impl futures::Future for PlayerWorker {
    type Output = Result<(), ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> futures::task::Poll<Self::Output> {
        loop {
            let mut progress = false;

            if let Poll::Ready(Some(command)) = self.command_receiver.as_mut().poll_next(cx) {
                self.handle_event(command);

                progress = true;
            }

            match self.play_task.as_mut().poll(cx) {
                Poll::Ready(Ok(())) => {
                    debug!("player: PlayerState::EndOfTrack");
                    progress = true;
                    self.event_sender.send(PlayerEvent::EndOfTrack).unwrap();
                }
                Poll::Ready(Err(Canceled)) => {
                    debug!("player task cancelled");
                    self.play_task = Box::pin(futures::future::pending());
                }
                Poll::Pending => ()
            }

            if !progress {
                return Poll::Pending;
            }
        }
    }
}