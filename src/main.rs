use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Instant,
};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
    time::{interval, Duration},
};

// Error handling
#[derive(Error, Debug)]
pub enum GameServerError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Time error: {0}")]
    TimeError(#[from] std::time::SystemTimeError),
    #[error("Server error: {0}")]
    ServerError(String),
}

type Result<T> = std::result::Result<T, GameServerError>;

// Basic types
#[derive(Serialize, Deserialize, Clone, Debug, Default, Copy)]
struct Vector2 {
    x: f32,
    y: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct PlayerState {
    position: Vector2,
    velocity: Vector2,
}

// Messages
#[derive(Serialize, Deserialize, Debug)]
enum ClientMessage {
    Join,
    Move { direction: Vector2 },
    Disconnect,
}

#[derive(Serialize, Deserialize, Debug)]
enum ServerMessage {
    JoinAccepted { player_id: String },
    GameState(GameStateUpdate),
    Error { message: String },
}

#[derive(Serialize, Deserialize, Debug)]
struct GameStateUpdate {
    tick: u64,
    players: HashMap<String, PlayerState>,
    server_time: u64,
}

// Input handling
#[derive(Debug)]
struct PlayerInput {
    timestamp: u64,
    message: ClientMessage,
}

// Player connection
#[derive(Debug)]
struct Player {
    connection: TcpStream,
    input_queue: VecDeque<PlayerInput>,
    state: PlayerState,
}

// Game state
#[derive(Clone, Debug)]
struct GameState {
    players: HashMap<String, PlayerState>,
}

impl GameState {
    fn new() -> Self {
        Self {
            players: HashMap::new(),
        }
    }
}

// Shared state management
#[derive(Debug)]
struct SharedState {
    game_state: Arc<Mutex<GameState>>,
    players: Arc<Mutex<HashMap<String, Player>>>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            game_state: Arc::new(Mutex::new(GameState::new())),
            players: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn with_game_state<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut GameState) -> Result<R>,
    {
        let mut game_state = self.game_state.lock().await;
        f(&mut game_state)
    }

    async fn with_players<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut HashMap<String, Player>) -> Result<R>,
    {
        let mut players = self.players.lock().await;
        f(&mut players)
    }

    async fn with_both<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut GameState, &mut HashMap<String, Player>) -> Result<R>,
    {
        let mut game_state = self.game_state.lock().await;
        let mut players = self.players.lock().await;
        f(&mut game_state, &mut players)
    }
}

impl Clone for SharedState {
    fn clone(&self) -> Self {
        Self {
            game_state: Arc::clone(&self.game_state),
            players: Arc::clone(&self.players),
        }
    }
}

// Main server implementation
struct GameServer {
    listener: TcpListener,
    tick_rate: Duration,
}

impl GameServer {
    pub async fn new(address: &str) -> Result<Self> {
        let listener = TcpListener::bind(address)
            .await
            .map_err(GameServerError::from)?;

        Ok(Self {
            listener,
            tick_rate: Duration::from_millis(16), // ~60Hz
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("Server starting...");

        let shared = SharedState::new();

        let game_loop = self.run_game_loop(shared.clone());
        let accept_loop = self.accept_connections(shared.clone());

        tokio::try_join!(game_loop, accept_loop)?;

        Ok(())
    }
    async fn handle_client(
        shared: SharedState,
        player_id: String,
        mut socket: TcpStream,
    ) -> Result<()> {
        // Split socket into read/write parts
        let (mut reader, _writer) = socket.split();
        let mut buffer = Vec::new();
        let mut temp_buffer = [0u8; 1024];

        loop {
            // Read data into temporary buffer
            let n = reader.read(&mut temp_buffer).await?;
            if n == 0 {
                // Connection closed
                return Ok(());
            }

            // Append to main buffer
            buffer.extend_from_slice(&temp_buffer[..n]);

            // Process complete messages
            while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                let message_bytes = buffer.drain(..=pos).collect::<Vec<_>>();
                let message = String::from_utf8_lossy(&message_bytes);

                // Parse the message
                match serde_json::from_str::<ClientMessage>(&message) {
                    Ok(client_message) => {
                        // Add to player's input queue
                        shared
                            .with_players(|players| {
                                if let Some(player) = players.get_mut(&player_id) {
                                    player.input_queue.push_back(PlayerInput {
                                        timestamp: std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_millis()
                                            as u64,
                                        message: client_message,
                                    });
                                }
                                Ok(())
                            })
                            .await?;
                    }
                    Err(e) => eprintln!("Failed to parse message: {}", e),
                }
            }
        }
    }
    async fn accept_connections(&self, shared: SharedState) -> Result<()> {
        loop {
            let (socket, addr) = self.listener.accept().await?;
            println!("New connection from: {}", addr);

            let player_id = addr.to_string();
            let player = Player {
                connection: socket.try_clone().await?,
                input_queue: VecDeque::new(),
                state: PlayerState::default(),
            };

            // Add player to shared state
            shared
                .with_both(|game_state, players| {
                    players.insert(player_id.clone(), player);
                    game_state
                        .players
                        .insert(player_id.clone(), PlayerState::default());
                    Ok(())
                })
                .await?;

            // Spawn client handler task
            let client_shared = shared.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(client_shared, player_id.clone(), socket).await
                {
                    eprintln!("Client handler error: {}", e);
                }
            });
        }
    }

    async fn run_game_loop(&self, shared: SharedState) -> Result<()> {
        let mut tick_interval = interval(self.tick_rate);
        let mut current_tick: u64 = 0;

        loop {
            tick_interval.tick().await;
            current_tick += 1;

            shared
                .with_both(|game_state, players| {
                    Self::process_pending_inputs(players, game_state)?;
                    Self::update_game_state(game_state)?;
                    Self::broadcast_game_state(players, game_state, current_tick)
                })
                .await?;
        }
    }

    fn process_pending_inputs(
        players: &mut HashMap<String, Player>,
        game_state: &mut GameState,
    ) -> Result<()> {
        // Process all pending inputs for each player
        for (player_id, player) in players.iter_mut() {
            while let Some(input) = player.input_queue.pop_front() {
                match input.message {
                    ClientMessage::Move { direction } => {
                        if let Some(state) = game_state.players.get_mut(player_id) {
                            // You might want to validate input here
                            state.velocity = direction;

                            // Update player's local state too
                            player.state.velocity = direction;
                        }
                    }
                    ClientMessage::Disconnect => {
                        // Handle disconnection in the next cleanup cycle
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    async fn broadcast_game_state(
        players: &mut HashMap<String, Player>,
        game_state: &GameState,
        tick: u64,
    ) -> Result<()> {
        let server_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(GameServerError::from)?
            .as_millis() as u64;

        // Create state update
        let update = ServerMessage::GameState(GameStateUpdate {
            tick,
            players: game_state.players.clone(),
            server_time,
        });

        let json = serde_json::to_string(&update)?;
        let mut message = json.into_bytes();
        message.push(b'\n');

        // Broadcast to all players
        let mut disconnected_players = Vec::new();

        // Get list of player IDs first to avoid borrow checker issues
        let player_ids: Vec<String> = players.keys().cloned().collect();

        for player_id in player_ids {
            if let Some(player) = players.get_mut(&player_id) {
                if let Err(e) = Self::send_to_player(player, &message).await {
                    eprintln!("Error sending to player {}: {}", player_id, e);
                    disconnected_players.push(player_id);
                }
            }
        }

        // Clean up disconnected players
        for player_id in disconnected_players {
            Self::remove_player(players, &player_id)?;
        }

        Ok(())
    }

    async fn send_to_player(player: &mut Player, message: &[u8]) -> Result<()> {
        player
            .connection
            .write_all(message)
            .await
            .map_err(|e| GameServerError::IoError(e))?;

        player
            .connection
            .flush()
            .await
            .map_err(|e| GameServerError::IoError(e))?;

        Ok(())
    }

    fn remove_player(players: &mut HashMap<String, Player>, player_id: &str) -> Result<()> {
        if players.remove(player_id).is_some() {
            println!("Player {} disconnected", player_id);
            Ok(())
        } else {
            Err(GameServerError::ServerError(format!(
                "Attempted to remove non-existent player: {}",
                player_id
            )))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut server = GameServer::new("127.0.0.1:8080").await?;
    server.run().await
}
