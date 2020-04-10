use serde::Deserialize;
use std::fs;
use anyhow::Result;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub spotify: Option<SpotifyConfig>,
    pub mpd: Option<MpdConfig>,
}

#[derive(Debug, Deserialize)]
pub struct SpotifyConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MpdConfig {
    pub ip: Option<String>,
    pub port: Option<u16>,
}

impl Config {
    pub fn new() -> Result<Self, anyhow::Error> {
        let config_contents = fs::read_to_string("config.toml")
            .expect("Something went wrong reading the config file");

        let config = toml::from_str(&config_contents)?;
        Ok(config)
    }

    pub fn get_redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.spotify.as_ref().unwrap().port.unwrap())
    }
}