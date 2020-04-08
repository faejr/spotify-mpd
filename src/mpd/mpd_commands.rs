use rspotify::spotify::client::Spotify;
use rspotify::spotify::model::playlist::{SimplifiedPlaylist};
use rspotify::spotify::model::artist::SimplifiedArtist;

pub trait MpdCommand {
    fn execute(&self, command: Option<regex::Captures>) -> Vec<String>;
}

pub struct StatusCommand { }
impl MpdCommand for StatusCommand {
    fn execute(&self, _: Option<regex::Captures>) -> Vec<String> {
        let mut output = vec![];
        output.push("repeat: 0");
        output.push("random: 0");
        output.push("single: 0");
        output.push("consume: 0");
        output.push("playlist: 1");
        output.push("playlistlength: 0");
        output.push("mixrampdb: 0.00000");
        output.push("state: stop");

        output.iter().map(|x| x.to_string()).collect::<Vec<String>>().into()
    }
}

pub struct StatsCommand { }
impl MpdCommand for StatsCommand {
    fn execute(&self, _: Option<regex::Captures>) -> Vec<String> {
        let mut output = vec![];
        output.push("uptime: 0");
        output.push("playtime: 0");

        output.iter().map(|x| x.to_string()).collect::<Vec<String>>().into()
    }
}

pub struct ListPlaylistsCommand {
    pub(crate) spotify: Spotify
}
impl MpdCommand for ListPlaylistsCommand {
    fn execute(&self, _: Option<regex::Captures>)-> Vec<String> {
        let playlists = self.spotify.current_user_playlists(None, None).unwrap();
        let mut string_builder = vec![];
        for playlist in playlists.items {
            string_builder.push(format!("playlist: {}", playlist.name));
            string_builder.push("Last-Modified: 2020-04-08T17:56:35Z".to_owned());
        }

        string_builder
    }
}

pub struct ListPlaylistInfoCommand {
    pub(crate) spotify: Spotify
}

impl MpdCommand for ListPlaylistInfoCommand {
    fn execute(&self, args: Option<regex::Captures>) -> Vec<String>{
        let playlist_name = &args.unwrap()[1];

        let simplified_playlist = self.get_playlist_by_name(playlist_name);
        let mut string_builder = vec![];
        if simplified_playlist.is_some() {
            let playlist = simplified_playlist.unwrap();
            let playlist_tracks = self.spotify.user_playlist_tracks(
                &self.spotify.current_user().unwrap().id,
                &playlist.id,
                None,
                None,
                None,
                None
            ).unwrap();
            for playlist_track in playlist_tracks.items {
                let track = playlist_track.track;
                string_builder.push(format!("file: {}", track.external_urls["spotify"]));
                string_builder.push(format!("Last-Modified: {}", track.album.release_date.unwrap()));
                string_builder.push(format!("Artist: {}", self.get_artists(track.artists)));
                string_builder.push(format!("AlbumArtist: {}", self.get_artists(track.album.artists)));
                string_builder.push(format!("Title: {}", track.name));
                string_builder.push(format!("Album: {}", track.album.name));
                let seconds = track.duration_ms / 1000;
                string_builder.push(format!("Time: {}", seconds));
                string_builder.push(format!("duration: {}", seconds));
            }
        }

        string_builder
    }
}
impl ListPlaylistInfoCommand {
    fn get_playlist_by_name(&self, name: &str) -> Option<SimplifiedPlaylist> {
        let playlists = self.spotify.current_user_playlists(None, None).unwrap();
        for playlist in playlists.items {
            if playlist.name == name {
                return Some(playlist)
            }
        }

        None
    }

    fn get_artists(&self, artists: Vec<SimplifiedArtist>) -> String {
        let artists_vec: Vec<String> = artists.iter().map(|x| x.name.to_owned()).collect();

        artists_vec.join(";")
    }
}