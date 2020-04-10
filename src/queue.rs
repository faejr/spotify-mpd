use std::sync::{Arc, RwLock, Mutex};
use crate::track::Track;
use crate::respot::{PlayerCommand, PlayerEvent};
use std::cmp::Ordering;
use futures::task::{Context, Poll};
use std::pin::Pin;
use std::time::SystemTime;
use tokio_core::reactor::Core;

pub struct Queue {
    pub queue: Arc<RwLock<Vec<Track>>>,
    current_track: RwLock<Option<usize>>,
    command_sender: Arc<Mutex<std::sync::mpsc::Sender<PlayerCommand>>>,
    status: RwLock<PlayerEvent>,
}

impl Queue {
    pub fn new(command_sender: Arc<Mutex<std::sync::mpsc::Sender<PlayerCommand>>>) -> Self {
        Self {
            queue: Arc::new(RwLock::new(Vec::new())),
            current_track: RwLock::new(None),
            command_sender,
            status: RwLock::new(PlayerEvent::Stopped),
        }
    }

    pub fn start_worker(queue: Arc<Queue>, event_receiver: std::sync::mpsc::Receiver<PlayerEvent>) {
        std::thread::spawn(move || {
            let mut core = Core::new().unwrap();
            let queue_worker = QueueWorker::new(queue, event_receiver);

            core.run(futures::compat::Compat::new(queue_worker)).unwrap();
        });
    }

    pub fn next_index(&self) -> Option<usize> {
        match *self.current_track.read().unwrap() {
            Some(mut index) => {
                let mut next_index = index + 1;
                if next_index < self.queue.read().unwrap().len() {
                    Some(next_index)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub fn previous_index(&self) -> Option<usize> {
        match *self.current_track.read().unwrap() {
            Some(mut index) => {
                if index > 0 {
                    let mut next_index = index - 1;
                    Some(next_index)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub fn get_current(&self) -> Option<Track> {
        match *self.current_track.read().unwrap() {
            Some(index) => Some(self.queue.read().unwrap()[index].clone()),
            None => None,
        }
    }

    pub fn append(&self, track: &Track) {
        let mut q = self.queue.write().unwrap();
        q.push(track.clone());
        debug!("appended new track to queue");
    }

    pub fn append_next(&self, tracks: Vec<&Track>) -> usize {
        let mut q = self.queue.write().unwrap();

        let first = match *self.current_track.read().unwrap() {
            Some(index) => index + 1,
            None => q.len(),
        };

        let mut i = first;
        for track in tracks {
            q.insert(i, track.clone());
            i += 1;
        }

        first
    }

    pub fn remove(&self, index: usize) {
        {
            let mut q = self.queue.write().unwrap();
            q.remove(index);
        }

        // if the queue is empty stop playback
        let len = self.queue.read().unwrap().len();
        if len == 0 {
            self.stop();
            return;
        }

        // if we are deleting the currently playing track, play the track with
        // the same index again, because the next track is now at the position
        // of the one we deleted
        let current = *self.current_track.read().unwrap();
        if let Some(current_track) = current {
            match current_track.cmp(&index) {
                Ordering::Equal => {
                    // stop playback if we have the deleted the last item and it
                    // was playing
                    if current_track == len {
                        self.stop();
                    } else {
                        self.play(index);
                    }
                }
                Ordering::Greater => {
                    let mut current = self.current_track.write().unwrap();
                    current.replace(current_track - 1);
                }
                _ => (),
            }
        }
    }

    pub fn clear(&self) {
        self.stop();

        let mut q = self.queue.write().unwrap();
        q.clear();
    }

    pub fn len(&self) -> usize {
        self.queue.read().unwrap().len()
    }

    pub fn shift(&self, from: usize, to: usize) {
        let mut queue = self.queue.write().unwrap();
        let item = queue.remove(from);
        queue.insert(to, item);

        // if the currently playing track is affected by the shift, update its
        // index
        let mut current = self.current_track.write().unwrap();
        if let Some(index) = *current {
            if index == from {
                current.replace(to);
            } else if index == to && from > index {
                current.replace(to + 1);
            } else if index == to && from < index {
                current.replace(to - 1);
            }
        }
    }

    pub fn play(&self, mut index: usize) {
        if let Some(track) = &self.queue.read().unwrap().get(index) {
            debug!("Dispatching load");
            self.dispatch(PlayerCommand::Load(track.id.as_ref().unwrap().to_owned()));
            let mut current = self.current_track.write().unwrap();
            current.replace(index);
            debug!("Dispatching play");
            self.dispatch(PlayerCommand::Play);
        }
    }

    pub fn toggleplayback(&self) {
        // self.respot.toggleplayback();
    }

    pub fn stop(&self) {
        let mut current = self.current_track.write().unwrap();
        *current = None;
        self.dispatch(PlayerCommand::Stop);
    }

    pub fn next(&self, manual: bool) {
        if let Some(index) = self.next_index() {
            self.play(index);
        } else {
            self.dispatch(PlayerCommand::Stop);
        }
    }

    pub fn previous(&self) {
        if let Some(index) = self.previous_index() {
            self.play(index);
        } else {
            self.dispatch(PlayerCommand::Stop);
        }
    }

    pub fn get_status(&self) -> PlayerEvent {
        let status = self.status.read().expect("unable to get read lock");

        return (*status).clone();
    }

    pub fn get_duration(&self) -> u32 {
        if let Some(ref track) = self.get_current() {
            info!("{}", track.duration / 1000);
            return track.duration / 1000
        }

        0
    }

    fn dispatch (&self, command: PlayerCommand) {
        self.command_sender.lock().unwrap().send(command).unwrap();
    }
}

struct QueueWorker {
    queue: Arc<Queue>,
    event_receiver: std::sync::mpsc::Receiver<PlayerEvent>,
}
impl QueueWorker {
    fn new(queue: Arc<Queue>, event_receiver: std::sync::mpsc::Receiver<PlayerEvent>) -> Self {
        Self {
            queue,
            event_receiver
        }
    }

    fn handle_event(&self, event: PlayerEvent) {
        match event {
            PlayerEvent::Paused => {
                //self.queue.set_elapsed(Some(self.get_current_progress()));
                //self.set_since(None);
            }
            PlayerEvent::Playing => {
                //self.set_since(Some(SystemTime::now()));
                info!("Received a playing event!");
            }
            PlayerEvent::Stopped | PlayerEvent::FinishedTrack => {
                //self.set_elapsed(None);
                //self.set_since(None);
            }
        }

        let mut status = self.queue.status.write().expect("unable to get write lock");
        *status = event;
    }
}

impl futures::Future for QueueWorker {
    type Output = Result<(), ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> futures::task::Poll<Self::Output> {
        loop {
            let mut progress = false;

            if let Ok(event) = self.event_receiver.recv() {
                self.handle_event(event);

                progress = true;
            }

            if !progress {
                return Poll::Pending;
            }
        }
    }
}