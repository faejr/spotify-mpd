use std::net::{TcpListener, TcpStream};
use net2::TcpStreamExt;
use std::thread;
use std::io::{Write, BufReader, BufRead, Read};
use rspotify::client::Spotify;
use regex::Regex;
use anyhow::{Result, Error};
use std::sync::{Arc, Mutex};
use core::fmt;
use bus::{Bus, BusReader};

use crate::mpd::mpd_commands::*;
use crate::queue::Queue;

mod mpd_commands;

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub enum SubsystemEvent {
    Database,
    Update,
    StoragePlaylist,
    Playlist,
    Mixer,
    Output,
    Options,
    Partition,
    Sticker,
    Subscription,
    Message,
}

impl fmt::Display for SubsystemEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct Client {
    spotify: Arc<Spotify>,
    queue: Arc<Queue>,
    event_bus: Arc<Mutex<Bus<SubsystemEvent>>>,
}

impl Client {
    fn new(spotify: Arc<Spotify>, queue: Arc<Queue>) -> Self {
        Self {
            spotify,
            queue,
            event_bus: Arc::new(Mutex::new(Bus::new(100))),
        }
    }
}

pub(crate) struct MpdServer {
    host: String,
    client: Arc<Client>,
}

impl MpdServer {
    pub fn new(host: String, spotify: Arc<Spotify>, queue: Arc<Queue>) -> Self {
        Self {
            host,
            client: Arc::new(Client::new(spotify, queue)),
        }
    }

    pub fn run(&mut self) {
        let listener = TcpListener::bind(self.host.to_owned()).unwrap();
        println!("Server listening on {}", self.host);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    println!("New connection: {}", stream.peer_addr().unwrap());

                    let mut handler = MpdRequestHandler::new(Arc::clone(&self.client));
                    let event_receiver = self.client.event_bus.lock().unwrap().add_rx();
                    thread::spawn(move || {
                        handler.handle_client(stream, event_receiver);
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

lazy_static! {
    static ref COMMANDS: Vec<Box<dyn MpdCommand + Sync + Send>> = vec![
        Box::new(StatusCommand),
        Box::new(StatsCommand),
        Box::new(ListPlaylistsCommand),
        Box::new(ListPlaylistInfoCommand),
        Box::new(AddCommand),
        Box::new(PlayCommand),
        Box::new(PauseCommand),
        Box::new(NextCommand),
        Box::new(PrevCommand),
        Box::new(ClearCommand),
        Box::new(PlaylistInfoCommand),
        Box::new(CurrentSongCommand),
        Box::new(SetVolCommand),
        Box::new(VolumeCommand),
        Box::new(DeleteIdCommand),
        Box::new(UrlHandlersCommand),
        Box::new(OutputsCommand),
        Box::new(DecodersCommand),
        Box::new(TagTypesCommand),
    ];
}

struct MpdRequestHandler {
    client: Arc<Client>,
    idle: bool,
    subsystems_changed: Vec<SubsystemEvent>
}

impl MpdRequestHandler {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            idle: false,
            subsystems_changed: vec![]
        }
    }

    #[tokio::main]
    async fn handle_client(&mut self, mut stream: TcpStream, mut event_receiver: BusReader<SubsystemEvent>) {
        let welcome = b"OK MPD 0.21.11\n";
        stream.write_all(welcome).expect("Unable to send OK msg");
        self.enable_timeout(&mut stream);

        loop {
            if let Ok(event) = event_receiver.try_recv() {
                self.subsystems_changed.push(event);
                if self.idle {
                    self.send_subsystem_changed(&mut stream);
                }
            }

            let mut command_list = self.get_command_list(&mut stream);

            if !command_list.is_empty() {
                let first_command = command_list.first().unwrap().to_owned();
                if first_command == "idle" {
                    println!("-> {:?}", command_list);
                    if !self.subsystems_changed.is_empty() {
                        self.send_subsystem_changed(&mut stream);
                    } else {
                        self.idle = true;
                        self.disable_timeout(&mut stream);
                    }
                    command_list.remove(0);
                } else if first_command == "noidle" {
                    self.idle = false;
                    self.enable_timeout(&mut stream);
                }
                if !self.idle {
                    self.run_commands(&mut stream, command_list).await;
                }
            }
        } // Loop
    }

    fn enable_timeout(&mut self, stream: &mut TcpStream) {
        stream.set_write_timeout_ms(Some(6000)).unwrap();
    }

    fn disable_timeout(&mut self, stream: &mut TcpStream) {
        stream.set_write_timeout_ms(None).unwrap();
    }

    fn send_subsystem_changed(&mut self, stream: &mut TcpStream) {
        self.subsystems_changed.sort();
        self.subsystems_changed.dedup();
        let subsystems: Vec<String> = self.subsystems_changed.iter().map(|e| e.to_string().to_lowercase()).collect();
        stream.write_all(format!("changed: {}\nOK\n", subsystems.join(", ")).as_bytes()).unwrap();
        self.subsystems_changed.clear();
        self.idle = false;
        assert!(self.subsystems_changed.is_empty());
        assert!(!self.idle);
        self.enable_timeout(stream);
        println!("<- OK (Subsystems changed)");
    }

    fn get_command_list(&self, stream: &mut TcpStream) -> Vec<String> {
        let mut command_list = vec![];
        if let Some(mut command) = MpdRequestHandler::get_cmd(stream) {
            if command == "command_list_start" {
                while command != "command_list_end" {
                    if let Some(command) = MpdRequestHandler::get_cmd(stream) {
                        if command != "command_list_end" {
                            command_list.push(command);
                        }
                    }
                }
            } else {
                command_list.push(command);
            }
        }

        command_list
    }

    fn get_cmd(stream: &mut TcpStream) -> Option<String>{
        let mut response = String::new();
        let mut buf_reader = BufReader::new(stream);
        buf_reader.read_line(&mut response).expect("could not read");

        Some(response.trim().to_owned())
    }

    async fn run_commands(&self, stream: &mut TcpStream, command_list: Vec<String>) {
        let mut output = vec![];
        for command in command_list {
            println!("-> {:?}", command);
            if let Ok(result) = self.execute_command(command).await {
                output.extend(result);
            }
            if self.has_error(&output) {
                break;
            }
        }
        if self.has_error(&output) {
            println!("<- {}", output.last().unwrap());
        } else {
            output.push("OK\n".to_owned());
            println!("<- OK");
        }

        stream.write_all(output.join("\n").as_bytes()).unwrap();
    }

    fn has_error(&self, output: &[String]) -> bool {
        for s in output {
            if s.starts_with("ACK") {
                return true;
            }
        }

        false
    }

    async fn execute_command(&self, command: String) -> Result<Vec<String>, Error> {
        lazy_static! {
            static ref RE: Regex = Regex::new("\\s+\"?([^\"]*)\"?").unwrap();
        }

        let command_name = command
            .split_whitespace()
            .next()
            .unwrap_or("");
        for mpd_command in COMMANDS.iter() {
            if mpd_command.get_type().contains(&command_name) {
                let args: Option<regex::Captures<'_>> = RE.captures(&command);
                let client = Arc::clone(&self.client);

                return match mpd_command.handle(client, args).await {
                    Ok(cmd) => Ok(cmd),
                    Err(e) => Err(e)
                };
            }
        }

        let mut output = vec![];
        if command == "commands" {
            for mpd_command in COMMANDS.iter() {
                for t in mpd_command.get_type() {
                    output.push(format!("command: {}", t));
                }
            }
        }

        Ok(output)
    }
}