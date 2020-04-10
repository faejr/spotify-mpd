use std::net::{TcpListener, TcpStream};
use std::thread;
use std::io::{Write, BufReader, BufRead};
use std::collections::HashMap;
use crate::mpd::mpd_commands::*;
use rspotify::client::Spotify;
use regex::Regex;
use anyhow::{Result, Error};
use std::sync::Arc;
use crate::queue::Queue;

mod mpd_commands;

pub(crate) struct MpdServer<'a> {
    host: &'a str,
    spotify: Arc<Spotify>,
    queue: Arc<Queue>,
}

impl MpdServer<'static> {
    pub fn new(host: &'static str, spotify: Arc<Spotify>, queue: Arc<Queue>) -> Self {
        Self {
            host,
            spotify,
            queue,
        }
    }

    pub fn run(&mut self) {
        let listener = TcpListener::bind(self.host.to_owned()).unwrap();
        println!("Server listening on {}", self.host);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    println!("New connection: {}", stream.peer_addr().unwrap());
                    let spotify = Arc::clone(&self.spotify);
                    let queue = Arc::clone(&self.queue);

                    thread::spawn(move || {
                        let mut mpd_handler = MpdRequestHandler::new(spotify, queue);
                        mpd_handler.handle_client(stream)
                    });
                }
                Err(e) => {
                    println!("Error: {}", e);
                }
            }
        }

        // close the socket server
        drop(listener);
    }
}

struct MpdRequestHandler {
    commands: HashMap<&'static str, Box<dyn MpdCommand + 'static>>,
    spotify: Arc<Spotify>,
    queue: Arc<Queue>,
}

impl MpdRequestHandler {
    pub fn new(spotify: Arc<Spotify>, queue: Arc<Queue>) -> Self {
        Self {
            spotify,
            queue,
            commands: HashMap::new(),
        }
    }

    fn commands(&self) -> HashMap<&'static str, Box<dyn MpdCommand>> {
        let mut commands: HashMap<&'static str, Box<dyn MpdCommand>> = HashMap::new();
        commands.insert("status", Box::new(StatusCommand::new(Arc::clone(&self.queue))));
        commands.insert("stats", Box::new(StatsCommand {}));
        commands.insert("listplaylists", Box::new(ListPlaylistsCommand { spotify: Arc::clone(&self.spotify) }));
        commands.insert("listplaylistinfo", Box::new(ListPlaylistInfoCommand::new(Arc::clone(&self.spotify))));
        commands.insert("add", Box::new(AddCommand::new(Arc::clone(&self.queue), Arc::clone(&self.spotify))));
        commands.insert("addid", Box::new(AddCommand::new(Arc::clone(&self.queue), Arc::clone(&self.spotify))));
        commands.insert("play", Box::new(PlayCommand::new(Arc::clone(&self.queue))));
        commands.insert("playid", Box::new(PlayCommand::new(Arc::clone(&self.queue))));
        commands.insert("pause", Box::new(PauseCommand::new(Arc::clone(&self.queue))));
        commands.insert("next", Box::new(NextCommand::new(Arc::clone(&self.queue))));
        commands.insert("prev", Box::new(PrevCommand::new(Arc::clone(&self.queue))));
        commands.insert("clear", Box::new(ClearCommand::new(Arc::clone(&self.queue))));
        commands.insert("playlistinfo", Box::new(PlaylistInfoCommand::new(Arc::clone(&self.queue))));
        commands.insert("plchanges", Box::new(PlaylistInfoCommand::new(Arc::clone(&self.queue))));
        commands.insert("currentsong", Box::new(CurrentSongCommand::new(Arc::clone(&self.queue))));
        commands.insert("setvol", Box::new(SetVolCommand::new(Arc::clone(&self.queue))));
        commands.insert("volume", Box::new(VolumeCommand::new(Arc::clone(&self.queue))));
        commands.insert("deleteid", Box::new(DeleteIdCommand::new(Arc::clone(&self.queue))));

        commands
    }

    #[tokio::main]
    async fn handle_client(&mut self, mut stream: TcpStream) {
        self.commands = self.commands();
        let welcome = b"OK MPD 0.21.11\n";
        stream.write(welcome).expect("Unable to send OK msg");

        loop {
            let mut reader = BufReader::new(&stream);
            let mut response = String::new();
            let mut command_list = vec![];
            reader.read_line(&mut response).expect("could not read");

            if response.trim() == "command_list_begin" {
                while response.trim() != "command_list_end" {
                    response = String::new();
                    reader.read_line(&mut response).expect("could not read");
                    if response.trim() != "command_list_end" {
                        command_list.push(response.trim().to_owned());
                    }
                }
            } else if response.len() > 0 && response.trim() != "idle" {
                command_list.push(response.trim().to_owned());
            }

            if command_list.len() > 0 {
                self.execute_command(&mut stream, command_list).await;
            }
        }
    }

    async fn execute_command(&self, stream: &mut TcpStream, command_list: Vec<String>) {
        let mut output = vec![];
        for command in command_list {
            println!("-> {:?}", command);
            match self.do_command(command).await {
                Ok(result) => output.extend(result),
                _ => {}
            }
            if self.has_error(&output) {
                break;
            }
        }
        if self.has_error(&output) {
            println!("< {}", output.last().unwrap());
        } else {
            output.push("OK\n".to_owned());
            println!("< OK");
        }

        stream.write(output.join("\n").as_bytes()).unwrap();
    }

    fn has_error(&self, output: &Vec<String>) -> bool {
        for s in output {
            if s.starts_with("ACK") {
                return true;
            }
        }

        false
    }

    async fn do_command(&self, command: String) -> Result<Vec<String>, Error> {
        lazy_static! {
            static ref RE: Regex = Regex::new("\\s+\"?([^\"]*)\"?").unwrap();
        }

        let command_name = command
            .split_whitespace()
            .next()
            .unwrap_or("");
        println!("Command name: {}", command_name);
        for (name, mpd_command) in &self.commands {
            if command_name.eq(*name) {
                let args: Option<regex::Captures<'_>> = RE.captures(&command);
                return match mpd_command.execute(args).await {
                    Ok(cmd) => Ok(cmd),
                    Err(e) => Err(e)
                };
            }
        }

        let mut output = vec![];

        if command.starts_with("lsinfo") {
            //stream.write(b"ACK [5@0] {lsinfo} Unsupported URI scheme");
        }
        if command == "urlhandlers" {
            output.push("handler: spotify:");
        }
        if command == "outputs" {
            output.push("outputsoutputid: 0");
            output.push("outputname: default detected output");
            output.push("plugin: alsa");
            output.push("outputenabled: 1");
            output.push("attribute: allowed_formats=");
            output.push("attribute: dop=0");
        }
        if command == "decoders" {
            output.push("plugin: mad");
            output.push("suffix: mp3");
            output.push("suffix: mp2");
            output.push("mime_type: audio/mpeg");
            output.push("plugin: mpcdec");
            output.push("suffix: mpc");
        }
        if command == "tagtypes" {
            output.push("tagtype: Artist");
            output.push("tagtype: ArtistSort");
            output.push("tagtype: Album");
            output.push("tagtype: AlbumSort");
            output.push("tagtype: AlbumArtist");
            output.push("tagtype: AlbumArtistSort");
            output.push("tagtype: Title");
            output.push("tagtype: Name");
            output.push("tagtype: Genre");
            output.push("tagtype: Date");
        }
        if command == "commands" {
            output.push("command: play");
            output.push("command: stop");
            output.push("command: pause");
            output.push("command: status");
            output.push("command: stats");
            output.push("command: decoders");
        }

        Ok(output.iter().map(|x| x.to_string()).collect::<Vec<String>>().into())
    }
}