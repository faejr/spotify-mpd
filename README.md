# spotify-mpd

An MPD server implementation for Spotify using [librespot](https://github.com/librespot-org/librespot).
Created with the motivation to not be confined to Spotify's resource heavy desktop application and instead be able to use my preferred MPD client.

### Prerequisites

* Rust
* A Spotify Premium account

# Todo

* Fix auto refresh of spotify token
* Implement repeat queue
* Implement shuffle queue
* Implement consume queue
* Implement `seekid` command
* Implement `move` command
* Find out what other commands we need to implement
* Walk through each track page to find all songs in a playlist
* Add tests
* Look into if we should utilize librespot for fetching more data instead of rspotify