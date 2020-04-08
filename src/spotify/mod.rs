use rspotify::spotify::client::Spotify;
use std::time::{Instant, Duration};
use rspotify::spotify::oauth2::{SpotifyOAuth, SpotifyClientCredentials};
use rspotify::spotify::util::get_token;
use std::fmt::{Error};
use crate::Config;

const SCOPES: [&str; 9] = [
    "playlist-read-private",
    "user-follow-read",
    "user-library-modify",
    "user-library-read",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "user-read-playback-state",
    "user-read-private",
    "user-read-recently-played",
];

pub fn new_spotify_client(config: Config) -> Result<(Spotify, Instant), Error> {
    let client_id = config.spotify_client_id.unwrap();
    let client_secret = config.spotify_client_secret.unwrap();
    let redirect_uri = config.spotify_client_redirect_uri.unwrap();

    println!("using client_id: {}", client_id);

    let mut oauth = SpotifyOAuth::default()
        .client_id(&client_id)
        .client_secret(&client_secret)
        .redirect_uri(&redirect_uri)
        .scope(&SCOPES.join(" "))
        .build();

    let token_info = get_token(&mut oauth).ok_or("No token").unwrap();
    let token_expiry = Instant::now() + Duration::from_secs(token_info.expires_in.into())
        - Duration::from_secs(120);
    println!(
        "token will expire in {:?}",
        Duration::from_secs(token_info.expires_in.into())
    );

    let client_credential = SpotifyClientCredentials::default()
        .token_info(token_info)
        .build();

    let spotify = Spotify::default()
        .client_credentials_manager(client_credential)
        .build();

    Ok((spotify, token_expiry))
}