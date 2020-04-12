use rspotify::model::playlist::SimplifiedPlaylist;
use rspotify::model::artist::SimplifiedArtist;
use async_trait::async_trait;
use anyhow::{Error, Result};
use std::sync::Arc;
use crate::track::Track;
use std::str::FromStr;
use crate::respot::PlayerEvent;
use crate::mpd::Client;

#[async_trait]
pub trait MpdCommand {
    fn get_type(&self) -> Vec<&str>;
    async fn execute(&self, client: Arc<Client>, args: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error>;
}

pub struct StatusCommand;
#[async_trait]
impl MpdCommand for StatusCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["status"]
    }

    async fn execute(&self, client: Arc<Client>, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        output.push("repeat: 0");
        output.push("random: 0");
        output.push("single: 0");
        output.push("consume: 0");
        output.push("playlist: 1");
        output.push("mixrampdb: 0.00000");

        let mut output_strings: Vec<String> = output.iter().map(|x| x.to_string()).collect::<Vec<String>>().into();
        output_strings.push(format!("volume: {}", client.queue.get_volume()));
        let status = client.queue.get_status();
        let playlist_length = client.queue.len();
        output_strings.push(format!("playlistlength: {}", playlist_length));
        output_strings.push(format!("state: {}", status.to_string()));
        if status == PlayerEvent::Playing || status == PlayerEvent::Paused {
            if let Some(songid) = client.queue.get_current_index() {
                output_strings.push(format!("song: {}", songid));
                output_strings.push(format!("songid: {}", songid));
            }
            let elapsed = client.queue.get_current_elapsed_time();
            let duration = client.queue.get_duration();
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
    fn get_type(&self) -> Vec<&str> {
        vec!["stats"]
    }

    async fn execute(&self, _: Arc<Client>, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        output.push("uptime: 0");
        output.push("playtime: 0");

        Ok(output.iter().map(|x| x.to_string()).collect::<Vec<String>>().into())
    }
}

pub struct ListPlaylistsCommand;
#[async_trait]
impl MpdCommand for ListPlaylistsCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["listplaylists"]
    }

    async fn execute(&self, client: Arc<Client>, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let playlists_result = client.spotify.current_user_playlists(None, None).await;
        let mut string_builder = vec![];

        match playlists_result {
            Ok(playlists) => {
                for playlist in playlists.items {
                    string_builder.push(format!("playlist: {}", playlist.name));
                    // We don't know the time :(
                    string_builder.push("Last-Modified: 1970-01-01T00:00:00Z".to_owned());
                }
            }
            Err(e) => {
                println!("error fetching playlists: {:?}", e)
            }
        }

        Ok(string_builder)
    }
}

pub struct ListPlaylistInfoCommand;
#[async_trait]
impl MpdCommand for ListPlaylistInfoCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["listplaylistinfo"]
    }

    async fn execute(&self, client: Arc<Client>, args: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let playlist_name = &args.unwrap()[1];

        let simplified_playlist = self.get_playlist_by_name(Arc::clone(&client), playlist_name).await;
        let mut string_builder = vec![];
        match simplified_playlist {
            Some(playlist) => {
                let playlist_tracks_result = client.spotify.user_playlist_tracks(
                    &self.get_current_user_id(&client).await?,
                    &playlist.id,
                    None,
                    None,
                    None,
                    None,
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
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => ()
                }
            }
            _ => {
                string_builder.push("ACK".to_owned());
            }
        }

        Ok(string_builder)
    }
}

impl ListPlaylistInfoCommand {
    async fn get_playlist_by_name(&self, client: Arc<Client>, name: &str) -> Option<SimplifiedPlaylist> {
        let playlists_result = client.spotify.current_user_playlists(None, None).await;
        match playlists_result {
            Ok(playlists) => {
                for playlist in playlists.items {
                    if playlist.name == name {
                        return Some(playlist);
                    }
                }
            }
            Err(_) => {
                println!("Unable to get playlist by name")
            }
        }


        None
    }

    async fn get_current_user_id(&self, client: &Arc<Client>) -> Result<String, Error> {
        let current_user_result = client.spotify.current_user().await;
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

pub struct AddCommand;
#[async_trait]
impl MpdCommand for AddCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["add", "addid"]
    }

    async fn execute(&self, client: Arc<Client>, args: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        let track_id = &args.unwrap()[1];
        let track_result = client.spotify.track(track_id).await;
        match track_result {
            Ok(full_track) => {
                let track = Track::from(&full_track);
                let song_id = client.queue.append(&track);
                output.push(format!("Id: {}", song_id))
            }
            Err(e) => return Err(Error::from(e.compat()))
        }

        Ok(output)
    }
}

pub struct PlayCommand;
#[async_trait]
impl MpdCommand for PlayCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["play", "playid"]
    }

    async fn execute(&self, client: Arc<Client>, args: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let index = usize::from_str(&args.unwrap()[1]).unwrap();
        client.queue.play(index);

        Ok(vec![])
    }
}

pub struct PauseCommand;
#[async_trait]
impl MpdCommand for PauseCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["pause"]
    }

    async fn execute(&self, client: Arc<Client>, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        client.queue.toggle_playback();

        Ok(vec![])
    }
}

pub struct NextCommand;
#[async_trait]
impl MpdCommand for NextCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["next"]
    }

    async fn execute(&self, client: Arc<Client>, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        client.queue.next();

        Ok(vec![])
    }
}

pub struct PrevCommand;
#[async_trait]
impl MpdCommand for PrevCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["prev"]
    }

    async fn execute(&self, client: Arc<Client>, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        client.queue.previous();

        Ok(vec![])
    }
}

pub struct ClearCommand;
#[async_trait]
impl MpdCommand for ClearCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["clear"]
    }

    async fn execute(&self, client: Arc<Client>, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        client.queue.clear();

        Ok(vec![])
    }
}

pub struct PlaylistInfoCommand;
#[async_trait]
impl MpdCommand for PlaylistInfoCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["playlistinfo", "plchanges"]
    }

    async fn execute(&self, client: Arc<Client>, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        let queue = client.queue.queue.read().unwrap();
        let mut pos = 0;
        for track in (*queue).clone() {
            output.extend(track.to_mpd_format(pos));
            pos = pos + 1;
        }

        Ok(output)
    }
}

pub struct CurrentSongCommand;
#[async_trait]
impl MpdCommand for CurrentSongCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["currentsong"]
    }

    async fn execute(&self, client: Arc<Client>, _: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let mut output = vec![];
        if let Some(current_track) = client.queue.get_current() {
            output = current_track.to_mpd_format(0);
        }

        Ok(output)
    }
}

pub struct SetVolCommand;
#[async_trait]
impl MpdCommand for SetVolCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["setvol"]
    }

    async fn execute(&self, client: Arc<Client>, args: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let volume_level = &args.unwrap()[1];

        client.queue.set_volume(volume_level.parse::<u16>().unwrap());

        Ok(vec![])
    }
}

pub struct VolumeCommand;
#[async_trait]
impl MpdCommand for VolumeCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["volume"]
    }

    async fn execute(&self, client: Arc<Client>, args: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let volume_level_str = &args.unwrap()[1];
        println!("{:?}", volume_level_str);
        let volume_level = volume_level_str.parse::<i16>().unwrap();

        client.queue.set_volume(client.queue.get_volume().wrapping_add(volume_level as u16));

        Ok(vec![])
    }
}

pub struct DeleteIdCommand;
#[async_trait]
impl MpdCommand for DeleteIdCommand {
    fn get_type(&self) -> Vec<&str> {
        vec!["deleteid"]
    }

    async fn execute(&self, client: Arc<Client>, args: Option<regex::Captures<'_>>) -> Result<Vec<String>, Error> {
        let song_id_arg = &args.unwrap()[1];

        if let Ok(song_id) = usize::from_str(song_id_arg) {
            client.queue.remove(song_id);
        }

        Ok(vec![])
    }
}
