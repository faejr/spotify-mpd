use serde::Deserialize;
use std::fs;
use anyhow::Result;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub spotify: Option<SpotifyConfig>,
    pub mpd: Option<MpdConfig>
}

#[derive(Debug, Deserialize)]
pub struct SpotifyConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Deserialize)]
pub struct MpdConfig{
    pub port: Option<u16>
}

impl Config {
    pub fn new()  -> Result<Self, anyhow::Error> {
        let config_contents = fs::read_to_string("config.toml")
            .expect("Something went wrong reading the config file");

        let config = toml::from_str(&config_contents)?;
        Ok(config)
    }

    pub fn get_redirect_uri(&self) -> String {
        format!("https://login.spotilocal.com:{}/callback", self.spotify.as_ref().unwrap().port.unwrap())
    }
}