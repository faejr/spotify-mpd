#[macro_use(lazy_static)]
extern crate lazy_static;
extern crate regex;

use crate::spotify::new_spotify_client;
use serde::Deserialize;
use std::fs;

mod mpd;
mod spotify;

#[derive(Debug, Deserialize)]
pub struct Config {
    spotify_client_id: Option<String>,
    spotify_client_secret: Option<String>,
    spotify_client_redirect_uri: Option<String>
}

fn main() {
    let config_contents = fs::read_to_string("config.toml")
        .expect("Something went wrong reading the config file");

    let decoded: Config = toml::from_str(&config_contents).unwrap();
    let (spotify, _) = new_spotify_client(decoded).unwrap();

    let mut mpd_server = mpd::MpdServer::new("127.0.0.1:6600", spotify);

    mpd_server.run();
}
