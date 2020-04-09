#[macro_use(lazy_static)]
extern crate lazy_static;
extern crate regex;

use crate::spotify::{new_spotify_client, get_token_auto, SCOPES};
use crate::config::Config;
use rspotify::oauth2::SpotifyOAuth;
use anyhow::Result;

mod config;
mod mpd;
mod spotify;
mod redirect_uri;

#[tokio::main]
async fn main() -> Result<()> {
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

            let mut mpd_server = mpd::MpdServer::new("127.0.0.1:6600", spotify);
            mpd_server.run();
        },
        None => println!("\nSpotify auth failed"),
    }

    Ok(())
}
