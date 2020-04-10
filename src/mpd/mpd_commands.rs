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
#[async_trait]
impl MpdCommand for StatusCommand {
    async fn execute(&self, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        output.push("repeat: 0");
        output.push("random: 0");
        output.push("single: 0");
        output.push("consume: 0");
        output.push("playlist: 1");
        output.push("mixrampdb: 0.00000");

        let mut output_strings: Vec<String> = output.iter().map(|x| x.to_string()).collect::<Vec<String>>().into();
        output_strings.push(format!("volume: {}", self.queue.get_volume()));
        let status = self.queue.get_status();
        let playlist_length = self.queue.len();
        output_strings.push(format!("playlistlength: {}", playlist_length));
        output_strings.push(format!("state: {}", status.to_string()));
        if status == PlayerEvent::Playing || status == PlayerEvent::Paused {
            if let Some(songid) = self.queue.get_current_index() {
                output_strings.push(format!("song: {}", songid));
                output_strings.push(format!("songid: {}", songid));
            }
            let elapsed = self.queue.get_current_progress();
            let duration = self.queue.get_duration();
            output_strings.push(format!("time: {}:{}", elapsed.as_secs(), duration));
            output_strings.push(format!("elapsed: {}", elapsed.as_secs_f32()));
            output_strings.push(format!("duration: {}", duration));
            output_strings.push("audio: 44100:24:2".to_owned());
            output_strings.push("bitrate: 320".to_owned());
        }

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
    spotify: Arc<Spotify>
}
impl ListPlaylistInfoCommand {
    pub fn new(spotify: Arc<Spotify>) -> Self {
        Self {
            spotify
        }
    }
}
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
        let mut output = vec![];
        let track_id = &args.unwrap()[1];
        let track_result = self.spotify.track(track_id).await;
        match track_result {
            Ok(full_track) => {
                let track = Track::from(&full_track);
                let song_id = self.queue.append(&track);
                output.push(format!("Id: {}", song_id))
            },
            Err(e) => return Err(Error::from(e.compat()))
        }

        Ok(output)
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

pub struct PauseCommand {
    queue: Arc<Queue>
}
impl PauseCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self {
            queue
        }
    }
}
#[async_trait]
impl MpdCommand for PauseCommand {
    async fn execute(&self, _: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        self.queue.toggle_playback();

        Ok(vec![])
    }
}

pub struct NextCommand {
    queue: Arc<Queue>
}
impl NextCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self {
            queue
        }
    }
}
#[async_trait]
impl MpdCommand for NextCommand {
    async fn execute(&self, _: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        self.queue.next();

        Ok(vec![])
    }
}

pub struct PrevCommand {
    queue: Arc<Queue>
}
impl PrevCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self {
            queue
        }
    }
}
#[async_trait]
impl MpdCommand for PrevCommand {
    async fn execute(&self, _: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        self.queue.previous();

        Ok(vec![])
    }
}

pub struct ClearCommand {
    queue: Arc<Queue>
}
impl ClearCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self {
            queue
        }
    }
}
#[async_trait]
impl MpdCommand for ClearCommand {
    async fn execute(&self, _: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        self.queue.clear();

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
    async fn execute(&self, _: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        let queue = self.queue.queue.read().unwrap();
        let mut pos = 0;
        for track in (*queue).clone() {
            output.extend(track.to_mpd_format(pos));
            pos = pos + 1;
        }

        Ok(output)
    }
}

pub struct CurrentSongCommand {
    queue: Arc<Queue>
}
impl CurrentSongCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self { queue }
    }
}
#[async_trait]
impl MpdCommand for CurrentSongCommand {
    async fn execute(&self, _: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        if let Some(current_track) = self.queue.get_current() {
            output = current_track.to_mpd_format(0);
        }

        Ok(output)
    }
}

pub struct SetVolCommand {
    queue: Arc<Queue>
}
impl SetVolCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self { queue }
    }
}
#[async_trait]
impl MpdCommand for SetVolCommand {
    async fn execute(&self, args: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        let volume_level = &args.unwrap()[1];

        self.queue.set_volume(volume_level.parse::<u16>().unwrap());

        Ok(vec![])
    }
}

pub struct VolumeCommand {
    queue: Arc<Queue>
}
impl VolumeCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self { queue }
    }
}
#[async_trait]
impl MpdCommand for VolumeCommand {
    async fn execute(&self, args: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        let volume_level_str = &args.unwrap()[1];
        println!("{:?}", volume_level_str);
        let volume_level = volume_level_str.parse::<i16>().unwrap();

        self.queue.set_volume(self.queue.get_volume().wrapping_add(volume_level as u16));

        Ok(vec![])
    }
}

pub struct DeleteIdCommand {
    queue: Arc<Queue>
}
impl DeleteIdCommand {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self { queue }
    }
}
#[async_trait]
impl MpdCommand for DeleteIdCommand {
    async fn execute(&self, args: Option<Captures<'_>>) -> Result<Vec<String>, Error> {
        let song_id_arg = &args.unwrap()[1];

        if let Ok(song_id) = usize::from_str(song_id_arg) {
            self.queue.remove(song_id);
        }

        Ok(vec![])
    }
}
