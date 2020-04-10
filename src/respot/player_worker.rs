use crate::respot::{PlayerCommand, PlayerEvent};
use librespot::playback::player::Player;
use futures::channel::oneshot::Canceled;
use librespot::core::spotify_id::SpotifyId;
use futures::task::{Context, Poll};
use std::pin::Pin;
use librespot::playback::mixer::Mixer;

pub struct PlayerWorker {
    player: Player,
    command_receiver: std::sync::mpsc::Receiver<PlayerCommand>,
    event_sender: std::sync::mpsc::Sender<PlayerEvent>,
    play_task: Pin<Box<dyn futures::Future<Output = Result<(), Canceled>>>>,
    active: bool,
    mixer: Box<dyn Mixer>
}

impl PlayerWorker {
    pub fn new(player: Player, mixer: Box<dyn Mixer>, command_receiver: std::sync::mpsc::Receiver<PlayerCommand>, event_sender: std::sync::mpsc::Sender<PlayerEvent>) -> Self {
        Self {
            player,
            command_receiver,
            event_sender,
            play_task: Box::pin(futures::future::pending()),
            active: false,
            mixer
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
            PlayerCommand::Pause => {
                self.player.pause();
                self.event_sender.send(PlayerEvent::Paused).unwrap();
                self.active = false;
                info!("pausing playback");
            },
            PlayerCommand::Stop => {
                self.player.stop();
                self.active = false;
                info!("Stopping playback");
            },
            PlayerCommand::SetVolume(vol) => {
                self.mixer.set_volume(Self::calc_logarithmic_volume(vol));
                info!("Setting volume to {}", vol);
            }
            _ => {}
        }
    }

    fn calc_logarithmic_volume(mut volume: u16) -> u16 {
        volume = ((std::cmp::min(volume, 100) as f32) / 100.0 * 65535_f32).ceil() as u16;
        // Volume conversion taken from https://github.com/plietar/librespot/blob/master/src/spirc.rs
        const IDEAL_FACTOR: f64 = 6.908;
        let normalized_volume = volume as f64 / std::u16::MAX as f64;

        let mut val = std::u16::MAX;
        if normalized_volume < 0.999 {
            let new_volume = (normalized_volume * IDEAL_FACTOR).exp() / 1000.0;
            val = (new_volume * std::u16::MAX as f64) as u16;
        }

        debug!("input volume:{} to mixer: {}", volume, val);

        val
    }
}

impl futures::Future for PlayerWorker {
    type Output = Result<(), ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> futures::task::Poll<Self::Output> {
        loop {
            let mut progress = false;

            if let Ok(command) = self.command_receiver.recv() {
                self.handle_event(command);

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