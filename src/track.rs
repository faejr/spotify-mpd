use rspotify::model::track::SimplifiedTrack;
use rspotify::model::album::FullAlbum;
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
    pub artist_ids: Vec<String>,
    pub album: String,
    pub album_id: Option<String>,
    pub album_artists: Vec<String>,
    pub cover_url: String,
    pub url: String,
    pub added_at: Option<DateTime<Utc>>,
}

impl Track {
    pub fn from_simplified_track(track: &SimplifiedTrack, album: &FullAlbum) -> Self {
        let artists = track
            .artists
            .iter()
            .map(|ref artist| artist.name.clone())
            .collect::<Vec<String>>();
        let artist_ids = track
            .artists
            .iter()
            .filter(|a| a.id.is_some())
            .map(|ref artist| artist.id.clone().unwrap())
            .collect::<Vec<String>>();
        let album_artists = album
            .artists
            .iter()
            .map(|ref artist| artist.name.clone())
            .collect::<Vec<String>>();

        let cover_url = match album.images.get(0) {
            Some(image) => image.url.clone(),
            None => "".to_owned(),
        };

        Self {
            id: track.id.clone(),
            title: track.name.clone(),
            track_number: track.track_number,
            disc_number: track.disc_number,
            duration: track.duration_ms,
            artists,
            artist_ids,
            album: album.name.clone(),
            album_id: Some(album.id.clone()),
            album_artists,
            cover_url,
            url: track.uri.clone(),
            added_at: None,
        }
    }

    pub fn duration_str(&self) -> String {
        let minutes = self.duration / 60_000;
        let seconds = (self.duration / 1000) % 60;
        format!("{:02}:{:02}", minutes, seconds)
    }
}