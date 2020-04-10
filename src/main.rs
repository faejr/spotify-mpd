#[macro_use(lazy_static)]
extern crate lazy_static;
extern crate regex;
#[macro_use] extern crate log;

extern crate strum;
#[macro_use]
extern crate strum_macros;
use serde::{Serialize, Deserialize};
use strum_macros::{Display};

use crate::spotify::{new_spotify_client, get_token_auto, SCOPES};
use crate::config::{Config, SpotifyConfig};
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
    let handle = core.handle();

    let config = Config::new()?;
    let spotify_config = config.spotify.as_ref().unwrap();

    let mut oauth = SpotifyOAuth::default()
        .client_id(spotify_config.client_id.as_ref().unwrap())
        .redirect_uri(&config.get_redirect_uri())
        .scope(&SCOPES.join(" "))
        .build();

    match get_token_auto(&mut oauth, spotify_config.port.unwrap()).await {
        Some(token_info) => {
            let (spotify, token_expiry) = new_spotify_client(token_info);

            let session = get_session(&spotify_config);
            let (command_sender, command_receiver) = std::sync::mpsc::channel::<PlayerCommand>();
            let (event_sender, _event_receiver) = std::sync::mpsc::channel::<PlayerEvent>();
            let command_receiver_mutex = Arc::new(Mutex::new(command_sender));

            let queue = Arc::new(Queue::new(command_receiver_mutex));

            let mut mpd_server = mpd::MpdServer::new("127.0.0.1:6600", spotify, queue);
            mpd_server.run();

            core.run(Respot::new(session, command_receiver, event_sender)).unwrap();
        },
        None => error!("Spotify auth failed"),
    }

    Ok(())
}

fn get_session(config: &SpotifyConfig) -> Session {
    let mut core = Core::new().unwrap();
    let session_config = SessionConfig::default();
    let credentials = Credentials::with_password(config.username.as_ref().unwrap().to_owned(), config.password.as_ref().unwrap().to_owned());

    let session = core
        .run(Session::connect(session_config, credentials, None, core.handle()))
        .unwrap();

    session
}