use rspotify::model::track::FullTrack;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Clone, Deserialize, Serialize)]
pub struct Track {
    pub id: Option<String>,
    pub title: String,
    pub track_number: u32,
    pub disc_number: i32,
    pub duration: u32,
    pub artists: Vec<String>,
    pub album: String,
    pub album_id: Option<String>,
    pub album_artists: Vec<String>,
    pub url: String,
    pub added_at: Option<DateTime<Utc>>,
    pub date: String,
}

impl Track {
    pub fn to_mpd_format(&self, pos: i32) -> Vec<String> {
        let mut output = vec![];

        output.push(format!("file: {}", self.id.as_ref().unwrap()));
        output.push(format!("Artist: {}", self.artists.join(";")));
        output.push(format!("AlbumArtist: {}", self.album_artists.join(";")));
        output.push(format!("Title: {}", self.title));
        output.push(format!("Album: {}", self.album));
        output.push(format!("Track: {}", self.track_number));
        output.push(format!("Date: {}", self.date));
        output.push(format!("Time: {}", self.duration / 1000));
        output.push(format!("duration: {}", self.duration / 1000));
        output.push(format!("Pos: {}", pos));
        output.push(format!("Id: {}", pos));

        output
    }
}

impl From<&FullTrack> for Track {
    fn from(track: &FullTrack) -> Self {
        let artists = track
            .artists
            .iter()
            .map(|ref artist| artist.name.clone())
            .collect::<Vec<String>>();
        let album_artists = track
            .album
            .artists
            .iter()
            .map(|ref artist| artist.name.clone())
            .collect::<Vec<String>>();

        let date = track.album.release_date.to_owned().unwrap();

        Self {
            id: track.id.clone(),
            title: track.name.clone(),
            track_number: track.track_number,
            disc_number: track.disc_number,
            duration: track.duration_ms,
            artists,
            album: track.album.name.clone(),
            album_id: track.album.id.clone(),
            album_artists,
            url: track.uri.clone(),
            added_at: None,
            date,
        }
    }
}