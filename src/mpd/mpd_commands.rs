use rspotify::client::Spotify;
use rspotify::model::playlist::{SimplifiedPlaylist};
use rspotify::model::artist::SimplifiedArtist;
use async_trait::async_trait;
use anyhow::{Error, Result};
use std::sync::Arc;
use crate::queue::Queue;
use regex::Captures;
use crate::track::Track;
use std::str::FromStr;
use crate::respot::PlayerEvent;

#[async_trait]
pub trait MpdCommand {
    async fn execute(&self, args: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error>;
}

pub struct StatusCommand {
    queue: Arc<Queue>
}
impl StatusCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self {
            queue
        }
    }
}
/*
volume: 100
repeat: 0
random: 0
single: 0
consume: 0
playlist: 4
playlistlength: 1
mixrampdb: 0.000000
state: play
song: 0
songid: 3
time: 6:226
elapsed: 6.078
bitrate: 224
duration: 225.515
audio: 44100:24:2
nextsong: 1
nextsongid: 2
OK
*/
#[async_trait]
impl MpdCommand for StatusCommand {
    async fn execute(&self, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        output.push("volume: 100");
        output.push("repeat: 0");
        output.push("random: 0");
        output.push("single: 0");
        output.push("consume: 0");
        output.push("playlist: 1");
        output.push("playlistlength: 1");
        output.push("mixrampdb: 0.00000");

        let mut output_strings: Vec<String> = output.iter().map(|x| x.to_string()).collect::<Vec<String>>().into();
        let status = self.queue.get_status();
        output_strings.push(format!("state: {}", status.to_string()));
        if status == PlayerEvent::Playing {
            output_strings.push(format!("song: {}", 0));
            output_strings.push(format!("duration: {}", self.queue.get_duration()));
            output_strings.push(format!("elapsed: {}", 10));
            /*
            elapsed: 6.078
            bitrate: 224
            duration: 225.515
            audio: 44100:24:2
            */
        }
        println!("{:?}", output_strings.as_slice());

        Ok(output_strings)
    }
}

pub struct StatsCommand;

#[async_trait]
impl MpdCommand for StatsCommand {
    async fn execute(&self, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        output.push("uptime: 0");
        output.push("playtime: 0");

        Ok(output.iter().map(|x| x.to_string()).collect::<Vec<String>>().into())
    }
}

pub struct ListPlaylistsCommand {
    pub(crate) spotify: Arc<Spotify>
}

#[async_trait]
impl MpdCommand for ListPlaylistsCommand {
    async fn execute(&self, _: Option<regex::Captures<'_>>)-> Result<Vec<String>, Error> {
        let playlists_result = self.spotify.current_user_playlists(None, None).await;
        let mut string_builder = vec![];

        match playlists_result {
            Ok(playlists) => {
                for playlist in playlists.items {
                    string_builder.push(format!("playlist: {}", playlist.name));
                    // We don't know the time :(
                    string_builder.push("Last-Modified: 1970-01-01T00:00:00Z".to_owned());
                }
            },
            Err(e) => {
                println!("error fetching playlists: {:?}", e)
            },
        }

        Ok(string_builder)
    }
}

pub struct ListPlaylistInfoCommand {
    pub(crate) spotify: Arc<Spotify>
}
// TODO: Walk through each track page to find all songs
#[async_trait]
impl MpdCommand for ListPlaylistInfoCommand {
    async fn execute(&self, args: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let playlist_name = &args.unwrap()[1];

        let simplified_playlist = self.get_playlist_by_name(playlist_name).await;
        let mut string_builder = vec![];
        match simplified_playlist {
            Some(playlist) => {
                let playlist_tracks_result = self.spotify.user_playlist_tracks(
                    &self.get_current_user_id().await?,
                    &playlist.id,
                    None,
                    None,
                    None,
                    None
                ).await;
                match playlist_tracks_result {
                    Ok(playlist_tracks) => {
                        for playlist_track in playlist_tracks.items
                        {
                            match playlist_track.track {
                                Some(track) => {
                                    // We don't support local tracks
                                    if track.is_local {
                                        break;
                                    }
                                    string_builder.push(format!("file: {}", track.id.unwrap()));
                                    string_builder.push(format!("Last-Modified: {}", track.album.release_date.unwrap()));
                                    string_builder.push(format!("Artist: {}", self.get_artists(track.artists)));
                                    string_builder.push(format!("AlbumArtist: {}", self.get_artists(track.album.artists)));
                                    string_builder.push(format!("Title: {}", track.name));
                                    string_builder.push(format!("Album: {}", track.album.name));
                                    let seconds = track.duration_ms / 1000;
                                    string_builder.push(format!("Time: {}", seconds));
                                    string_builder.push(format!("duration: {}", seconds));
                                },
                                _ => {}
                            }

                        }
                    },
                    Err(_) => ()
                }
            },
            _ => {
                string_builder.push("ACK".to_owned());
            }
        }

        Ok(string_builder)
    }
}
impl ListPlaylistInfoCommand {
    async fn get_playlist_by_name(&self, name: &str) -> Option<SimplifiedPlaylist> {
        let playlists_result = self.spotify.current_user_playlists(None, None).await;
        match playlists_result {
            Ok(playlists) => {
                for playlist in playlists.items {
                    if playlist.name == name {
                        return Some(playlist)
                    }
                }
            },
            Err(_) => {
                println!("Unable to get playlist by name")
            }
        }


        None
    }

    async fn get_current_user_id(&self) -> Result<String, Error> {
        let current_user_result = self.spotify.current_user().await;
        match current_user_result {
            Ok(current_user) => Ok(current_user.id),
            Err(e) => Err(Error::from(e.compat()))
        }
    }

    fn get_artists(&self, artists: Vec<SimplifiedArtist>) -> String {
        let artists_vec: Vec<String> = artists.iter().map(|x| x.name.to_owned()).collect();

        artists_vec.join(";")
    }
}

pub struct AddCommand {
    queue: Arc<Queue>,
    spotify: Arc<Spotify>
}
#[async_trait]
impl MpdCommand for AddCommand {
    async fn execute(&self, args: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        let track_id = &args.unwrap()[1];
        let track_result = self.spotify.track(track_id).await;
        match track_result {
            Ok(full_track) => {
                let track = Track::from(&full_track);
                self.queue.append(&track);
            },
            Err(e) => return Err(Error::from(e.compat()))
        }
        debug!("{}", track_id);

        Ok(vec![])
    }
}
impl AddCommand {
    pub fn new(queue: Arc<Queue>, spotify: Arc<Spotify>) -> Self {
        Self {
            queue,
            spotify
        }
    }
}

pub struct PlayCommand {
    queue: Arc<Queue>
}
impl PlayCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self {
            queue
        }
    }
}
#[async_trait]
impl MpdCommand for PlayCommand {
    async fn execute(&self, args: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        let index = usize::from_str(&args.unwrap()[1]).unwrap();
        self.queue.play(index);

        Ok(vec![])
    }
}

pub struct PlaylistInfoCommand {
    queue: Arc<Queue>
}
impl PlaylistInfoCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self { queue }
    }
}
#[async_trait]
impl MpdCommand for PlaylistInfoCommand {
    async fn execute(&self, args: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        let queue = self.queue.queue.read().unwrap();
        let mut pos = 0;
        for track in (*queue).clone() {
            output.push(format!("file: {}", track.id.unwrap()));
            output.push(format!("Artist: {}", track.artists.join(";")));
            output.push(format!("AlbumArtist: {}", track.album_artists.join(";")));
            output.push(format!("Title: {}", track.title));
            output.push(format!("Album: {}", track.album));
            output.push(format!("Track: {}", track.track_number));
            output.push(format!("Date: {}", track.date));
            output.push(format!("Time: {}", track.duration / 1000));
            output.push(format!("duration: {}", track.duration / 1000));
            output.push(format!("Pos: {}", pos));
            output.push(format!("Id: {}", pos));
            pos = pos + 1;
        }

        Ok(output)
    }
}

// SetVolCommand ?