#[macro_use(lazy_static)]
extern crate lazy_static;
extern crate regex;
#[macro_use]
extern crate log;

extern crate strum;
#[macro_use]
extern crate strum_macros;

use crate::spotify::{new_spotify_client, get_token_auto, SCOPES};
use crate::config::Config;
use rspotify::oauth2::SpotifyOAuth;
use anyhow::Result;
use tokio_core::reactor::Core;
use librespot::core::authentication::Credentials;
use librespot::core::config::SessionConfig;
use librespot::core::session::Session;
use crate::respot::{PlayerCommand, PlayerEvent, Respot};
use std::sync::{Mutex, Arc};
use crate::queue::Queue;

mod config;
mod mpd;
mod spotify;
mod redirect_uri;

mod respot;
mod queue;
mod track;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let mut core = Core::new().unwrap();

    let config = Config::new()?;
    let spotify_config = config.spotify.as_ref().unwrap();

    let mut oauth = SpotifyOAuth::default()
        .client_id(spotify_config.client_id.as_ref().unwrap())
        .client_secret(spotify_config.client_secret.as_ref().unwrap())
        .redirect_uri(&config.get_redirect_uri())
        .scope(&SCOPES.join(" "))
        .build();

    match get_token_auto(&mut oauth, spotify_config.port.unwrap()).await {
        Some(token_info) => {
            let (spotify, token_expiry) = new_spotify_client(token_info);

            let (command_sender, command_receiver) = std::sync::mpsc::channel::<PlayerCommand>();
            let (event_sender, event_receiver) = std::sync::mpsc::channel::<PlayerEvent>();
            let command_sender_mutex = Arc::new(Mutex::new(command_sender));

            let queue = Arc::new(Queue::new(command_sender_mutex));
            Queue::start_worker(queue.clone(), event_receiver);

            let session_config = SessionConfig::default();
            let credentials = Credentials::with_password(spotify_config.username.as_ref().unwrap().to_owned(), spotify_config.password.as_ref().unwrap().to_owned());

            let session = core
                .run(Session::connect(session_config, credentials, None, core.handle()))
                .unwrap();

            std::thread::spawn(move || {
                let mut mpd_server = mpd::MpdServer::new("127.0.0.1:6600", spotify, queue);
                mpd_server.run();
            });

            core.run(Respot::new(session, command_receiver, event_sender)).unwrap();
        }
        None => error!("Spotify auth failed"),
    }

    Ok(())
}
