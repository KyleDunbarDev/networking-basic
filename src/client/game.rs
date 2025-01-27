use crate::common::{ClientMessage, GameError, PlayerState, Result, ServerMessage, Vector2};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

pub struct GameClient {
    stream: TcpStream,
    server_message_receiver: Receiver<ServerMessage>,
    game_command_sender: Sender<ClientMessage>,
    player_id: Option<String>,
    current_state: Option<HashMap<String, PlayerState>>,
}

impl GameClient {
    pub fn new(address: &str) -> Result<Self> {
        let stream = TcpStream::connect(address)?;
        stream.set_nonblocking(true)?;

        // Channel for receiving parsed server messages
        let (server_msg_sender, server_message_receiver) = mpsc::channel();

        // Channel for sending game commands
        let (game_command_sender, game_command_receiver) = mpsc::channel();

        // Spawn reader thread
        let reader_stream = stream.try_clone()?;
        thread::spawn(move || {
            if let Err(e) = Self::handle_server_messages(reader_stream, server_msg_sender) {
                eprintln!("Server message handler error: {}", e);
            }
        });

        // Spawn writer thread
        let writer_stream = stream.try_clone()?;
        thread::spawn(move || {
            if let Err(e) = Self::handle_client_messages(writer_stream, game_command_receiver) {
                eprintln!("Client message handler error: {}", e);
            }
        });

        Ok(Self {
            stream,
            server_message_receiver,
            game_command_sender,
            player_id: None,
            current_state: None,
        })
    }

    fn handle_server_messages(stream: TcpStream, sender: Sender<ServerMessage>) -> Result<()> {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();

        println!("Started server message handler");

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    println!("Received raw message: {}", line.trim());
                    match serde_json::from_str::<ServerMessage>(&line) {
                        Ok(msg) => {
                            println!("Parsed server message: {:?}", msg);
                            if sender.send(msg).is_err() {
                                break;
                            }
                        }
                        Err(e) => eprintln!("Failed to parse server message: {}", e),
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => return Err(GameError::IoError(e)),
            }
        }
        Ok(())
    }

    fn handle_client_messages(
        mut stream: TcpStream,
        receiver: Receiver<ClientMessage>,
    ) -> Result<()> {
        loop {
            match receiver.recv() {
                Ok(msg) => {
                    let json = serde_json::to_string(&msg)?;
                    stream.write_all(format!("{}\n", json).as_bytes())?;
                    stream.flush()?;
                }
                Err(_) => break,
            }
        }
        Ok(())
    }

    pub fn connect(&mut self) -> Result<()> {
        // Send join message
        self.game_command_sender
            .send(ClientMessage::Join)
            .map_err(|_| GameError::NetworkError("Failed to send join message".into()))?;

        // Wait for join acceptance
        let timeout = Duration::from_secs(5);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Ok(msg) = self.server_message_receiver.try_recv() {
                match msg {
                    ServerMessage::JoinAccepted { player_id } => {
                        self.player_id = Some(player_id);
                        return Ok(());
                    }
                    ServerMessage::Error { message } => {
                        return Err(GameError::NetworkError(message));
                    }
                    _ => continue,
                }
            }
            thread::sleep(Duration::from_millis(100));
        }

        Err(GameError::NetworkError("Connection timeout".into()))
    }

    pub fn move_player(&mut self, direction: Vector2) -> Result<()> {
        self.game_command_sender
            .send(ClientMessage::Move { direction })
            .map_err(|_| GameError::NetworkError("Failed to send move command".into()))?;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        // Process any pending server messages
        while let Ok(msg) = self.server_message_receiver.try_recv() {
            match msg {
                ServerMessage::GameState(update) => {
                    self.current_state = Some(update.players);
                }
                ServerMessage::Error { message } => {
                    eprintln!("Server error: {}", message);
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn get_player_state(&self, player_id: &str) -> Option<&PlayerState> {
        self.current_state.as_ref()?.get(player_id)
    }

    pub fn get_own_state(&self) -> Option<&PlayerState> {
        self.player_id
            .as_ref()
            .and_then(|id| self.get_player_state(id))
    }

    pub fn disconnect(&mut self) -> Result<()> {
        self.game_command_sender
            .send(ClientMessage::Disconnect)
            .map_err(|_| GameError::NetworkError("Failed to send disconnect message".into()))?;
        Ok(())
    }

    pub fn debug_info(&self) -> String {
        let mut info = String::new();

        // Connection status
        info.push_str(&format!(
            "Connection: {}\n",
            if self.player_id.is_some() {
                "Connected"
            } else {
                "Disconnected"
            }
        ));

        // Player info
        if let Some(player_id) = &self.player_id {
            info.push_str(&format!("Player ID: {}\n", player_id));

            if let Some(state) = self.get_own_state() {
                info.push_str(&format!(
                    "Position: ({:.2}, {:.2})\n",
                    state.position.x, state.position.y
                ));
                info.push_str(&format!(
                    "Velocity: ({:.2}, {:.2})\n",
                    state.velocity.x, state.velocity.y
                ));
            }
        }

        // Other players
        if let Some(state) = &self.current_state {
            info.push_str(&format!("Other players: {}\n", state.len() - 1));

            for (id, player) in state {
                if Some(id) != self.player_id.as_ref() {
                    info.push_str(&format!(
                        "  {} at ({:.2}, {:.2})\n",
                        id, player.position.x, player.position.y
                    ));
                }
            }
        }

        info
    }
}

// Example usage
fn main() -> Result<()> {
    let mut client = GameClient::new("127.0.0.1:8080")?;

    // Connect and join game
    client.connect()?;

    println!("Connected to server!");

    // Game loop
    let mut last_print = std::time::Instant::now();
    loop {
        // Send movement
        client.move_player(Vector2 { x: 1.0, y: 0.0 })?;

        // Update game state
        client.update()?;

        // Print state every second
        if last_print.elapsed() >= Duration::from_secs(1) {
            if let Some(state) = client.get_own_state() {
                println!("Current position: {:?}", state.position);
            }
            last_print = std::time::Instant::now();
        }

        thread::sleep(Duration::from_millis(16));
    }
}
