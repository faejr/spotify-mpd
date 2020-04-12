use std::net::{TcpListener, TcpStream};
use std::thread;
use std::io::{Write, BufReader, BufRead};
use crate::mpd::mpd_commands::*;
use rspotify::client::Spotify;
use regex::Regex;
use anyhow::{Result, Error};
use std::sync::Arc;
use crate::queue::Queue;

mod mpd_commands;

pub struct Client {
    spotify: Arc<Spotify>,
    queue: Arc<Queue>,
}

impl Client {
    fn new(spotify: Arc<Spotify>, queue: Arc<Queue>) -> Self {
        Self {
            spotify,
            queue,
        }
    }
}

pub(crate) struct MpdServer {
    host: String,
    handler: Arc<MpdRequestHandler>,
}

impl MpdServer {
    pub fn new(host: String, spotify: Arc<Spotify>, queue: Arc<Queue>) -> Self {
        Self {
            host,
            handler: Arc::new(MpdRequestHandler::new(Arc::new(Client::new(spotify, queue)))),
        }
    }

    pub fn run(&mut self) {
        let listener = TcpListener::bind(self.host.to_owned()).unwrap();
        println!("Server listening on {}", self.host);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    println!("New connection: {}", stream.peer_addr().unwrap());

                    let handler = Arc::clone(&self.handler);
                    thread::spawn(move || {
                        handler.handle_client(stream)
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
    commands: Vec<Box<dyn MpdCommand + 'static + Sync + Send>>,
    client: Arc<Client>,
}

impl MpdRequestHandler {
    pub fn new(client: Arc<Client>) -> Self {
        let mut handler = Self {
            client,
            commands: vec![],
        };
        handler.commands = handler.commands();

        handler
    }

    fn commands(&self) -> Vec<Box<dyn MpdCommand + Sync + Send>> {
        let mut commands: Vec<Box<dyn MpdCommand + Sync + Send>> = vec![];
        commands.push(Box::new(StatusCommand {}));
        commands.push(Box::new(StatsCommand {}));
        commands.push(Box::new(ListPlaylistsCommand {}));
        commands.push(Box::new(ListPlaylistInfoCommand {}));
        commands.push(Box::new(AddCommand {}));
        commands.push(Box::new(PlayCommand {}));
        commands.push(Box::new(PauseCommand {}));
        commands.push(Box::new(NextCommand {}));
        commands.push(Box::new(PrevCommand {}));
        commands.push(Box::new(ClearCommand {}));
        commands.push(Box::new(PlaylistInfoCommand {}));
        commands.push(Box::new(CurrentSongCommand {}));
        commands.push(Box::new(SetVolCommand {}));
        commands.push(Box::new(VolumeCommand {}));
        commands.push(Box::new(DeleteIdCommand {}));
        commands.push(Box::new(UrlHandlersCommand {}));
        commands.push(Box::new(OutputsCommand {}));
        commands.push(Box::new(DecodersCommand {}));
        commands.push(Box::new(TagTypesCommand {}));

        commands
    }

    #[tokio::main]
    async fn handle_client(&self, mut stream: TcpStream) {
        let welcome = b"OK MPD 0.21.11\n";
        stream.write(welcome).expect("Unable to send OK msg");

        loop {
            let command_list = self.get_command_list(&mut stream);

            if command_list.len() > 0 {
                self.run_commands(&mut stream, command_list).await;
            }
        }
    }

    fn get_command_list(&self, stream: &mut TcpStream) -> Vec<String> {
        let mut reader = BufReader::new(stream);
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

        command_list
    }

    async fn run_commands(&self, stream: &mut TcpStream, command_list: Vec<String>) {
        let mut output = vec![];
        for command in command_list {
            println!("-> {:?}", command);
            match self.execute_command(command).await {
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

    async fn execute_command(&self, command: String) -> Result<Vec<String>, Error> {
        lazy_static! {
            static ref RE: Regex = Regex::new("\\s+\"?([^\"]*)\"?").unwrap();
        }

        let command_name = command
            .split_whitespace()
            .next()
            .unwrap_or("");
        for mpd_command in &self.commands {
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
            for mpd_command in &self.commands {
                for t in mpd_command.get_type() {
                    output.push(format!("command: {}", t));
                }
            }
        }

        Ok(output.iter().map(|x| x.to_string()).collect::<Vec<String>>().into())
    }
}