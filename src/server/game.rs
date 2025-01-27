use crate::common::{
    ClientMessage, GameError, GameStateUpdate, InternalMessage, PlayerState, Result, ServerMessage,
    Timestamp, Vector2,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

#[derive(Debug)]
pub struct PlayerInput {
    timestamp: Timestamp,
    message: ClientMessage,
}

pub struct Player {
    sender: Sender<Vec<u8>>,
    input_queue: VecDeque<PlayerInput>,
    state: PlayerState,
}

// Game rules configuration
pub struct GameRules {
    pub map_bounds: (f32, f32),
    pub max_velocity: f32,
    pub collision_radius: f32,
}

impl Default for GameRules {
    fn default() -> Self {
        Self {
            map_bounds: (-100.0, 100.0),
            max_velocity: 10.0,
            collision_radius: 10.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GameState {
    players: HashMap<String, PlayerState>,
    last_update: Timestamp,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            last_update: Timestamp::now(),
        }
    }

    pub fn update(&mut self, delta_time: Duration) -> Result<()> {
        let rules = GameRules::default();

        // First pass: Update positions
        for player_state in self.players.values_mut() {
            // Update position based on velocity
            let position_delta = player_state.velocity.scale(delta_time.as_secs_f32());
            player_state.position = player_state.position.add(&position_delta);

            // Apply bounds
            player_state.position.x = player_state
                .position
                .x
                .clamp(rules.map_bounds.0, rules.map_bounds.1);
            player_state.position.y = player_state
                .position
                .y
                .clamp(rules.map_bounds.0, rules.map_bounds.1);

            // Clamp velocity
            player_state.velocity.x = player_state
                .velocity
                .x
                .clamp(-rules.max_velocity, rules.max_velocity);
            player_state.velocity.y = player_state
                .velocity
                .y
                .clamp(-rules.max_velocity, rules.max_velocity);

            player_state.last_update = Timestamp::now();
        }

        // Second pass: Check and resolve collisions
        self.resolve_collisions(&rules);

        self.last_update = Timestamp::now();
        Ok(())
    }

    fn resolve_collisions(&mut self, rules: &GameRules) {
        // Collect current positions to avoid borrow checker issues
        let positions: Vec<(String, Vector2)> = self
            .players
            .iter()
            .map(|(id, state)| (id.clone(), state.position))
            .collect();

        // Track collisions that need to be resolved
        let mut collisions = Vec::new();

        // Detect collisions
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                let (id1, pos1) = &positions[i];
                let (id2, pos2) = &positions[j];

                let dx = pos1.x - pos2.x;
                let dy = pos1.y - pos2.y;
                let distance = (dx * dx + dy * dy).sqrt();

                if distance < rules.collision_radius {
                    collisions.push((id1.clone(), id2.clone()));
                }
            }
        }

        // Resolve collisions
        for (id1, id2) in collisions {
            // First, collect the current states
            let (pos1, pos2, vel1, vel2) = {
                let player1 = &self.players[&id1];
                let player2 = &self.players[&id2];
                (
                    player1.position,
                    player2.position,
                    player1.velocity,
                    player2.velocity,
                )
            };

            // Calculate collision response
            let dx = pos1.x - pos2.x;
            let dy = pos1.y - pos2.y;
            let distance = (dx * dx + dy * dy).sqrt();

            let mut vel_updates = Vec::new();
            let mut pos_updates = Vec::new();

            if distance < rules.collision_radius {
                // Calculate position updates
                let overlap = rules.collision_radius - distance;
                let angle = dy.atan2(dx);
                let move_x = overlap * 0.5 * angle.cos();
                let move_y = overlap * 0.5 * angle.sin();

                // Store updates to apply later
                vel_updates.push((id1.clone(), vel2));
                vel_updates.push((id2.clone(), vel1));

                pos_updates.push((
                    id1.clone(),
                    Vector2 {
                        x: pos1.x + move_x,
                        y: pos1.y + move_y,
                    },
                ));
                pos_updates.push((
                    id2.clone(),
                    Vector2 {
                        x: pos2.x - move_x,
                        y: pos2.y - move_y,
                    },
                ));
            }

            // Apply updates
            for (id, vel) in vel_updates {
                if let Some(player) = self.players.get_mut(&id) {
                    player.velocity = vel;
                }
            }

            for (id, pos) in pos_updates {
                if let Some(player) = self.players.get_mut(&id) {
                    player.position = pos;
                }
            }
        }
    }

    pub fn add_player(&mut self, player_id: String, state: PlayerState) {
        self.players.insert(player_id, state);
    }

    pub fn remove_player(&mut self, player_id: &str) {
        self.players.remove(player_id);
    }

    pub fn get_player_state(&self, player_id: &str) -> Option<&PlayerState> {
        self.players.get(player_id)
    }

    pub fn get_player_count(&self) -> usize {
        self.players.len()
    }
}

pub struct GameServer {
    game_state: GameState,
    players: HashMap<String, Player>,
    tick_rate: Duration,
    input_receiver: Receiver<InternalMessage>,
    input_sender: Sender<InternalMessage>,
    address: String,
}

impl GameServer {
    pub fn new(address: &str) -> Result<Self> {
        let (input_sender, input_receiver) = std::sync::mpsc::channel();

        Ok(Self {
            game_state: GameState::new(),
            players: HashMap::new(),
            tick_rate: Duration::from_millis(16),
            input_receiver,
            input_sender,
            address: address.to_string(),
        })
    }

    pub fn add_connection(&mut self, player_id: String, sender: Sender<Vec<u8>>) {
        let player = Player {
            sender,
            input_queue: VecDeque::new(),
            state: PlayerState {
                position: Vector2::default(),
                velocity: Vector2::default(),
                last_update: Timestamp::now(),
            },
        };
        self.players.insert(player_id, player);
    }

    pub fn run(&mut self) -> Result<()> {
        println!("Game server starting on {}", self.address);

        let input_sender = self.input_sender.clone();
        let address = self.address.clone();

        // Spawn network handling thread
        std::thread::spawn(move || {
            if let Err(e) = super::network::handle_connections(&address, input_sender) {
                eprintln!("Network error: {}", e);
            }
        });

        self.run_game_loop()
    }

    fn run_game_loop(&mut self) -> Result<()> {
        let mut current_tick: u64 = 0;
        let mut last_tick = Timestamp::now();

        loop {
            let now = Timestamp::now();
            let delta_time = now.duration_since(&last_tick);

            if delta_time >= self.tick_rate {
                self.process_messages()?;
                self.update_game_state(delta_time)?;
                self.broadcast_state(current_tick)?;

                current_tick += 1;
                last_tick = now;
            } else {
                std::thread::sleep(self.tick_rate.saturating_sub(delta_time));
            }
        }
    }

    fn process_messages(&mut self) -> Result<()> {
        while let Ok(message) = self.input_receiver.try_recv() {
            match message {
                InternalMessage::NewConnection { player_id, sender } => {
                    let player = Player {
                        sender,
                        input_queue: VecDeque::new(),
                        state: PlayerState {
                            position: Vector2::default(),
                            velocity: Vector2::default(),
                            last_update: Timestamp::now(),
                        },
                    };
                    self.players.insert(player_id, player);
                }
                InternalMessage::ClientMessage { player_id, message } => {
                    self.handle_client_message(&player_id, message)?;
                }
            }
        }
        Ok(())
    }

    fn handle_client_message(&mut self, player_id: &str, message: ClientMessage) -> Result<()> {
        match message {
            ClientMessage::Join => {
                self.handle_player_join(player_id)?;
            }
            ClientMessage::Move { direction } => {
                if let Some(player) = self.game_state.players.get_mut(player_id) {
                    player.velocity = direction;
                    player.last_update = Timestamp::now();
                }
            }
            ClientMessage::Disconnect => {
                self.remove_player(player_id)?;
            }
        }
        Ok(())
    }

    fn handle_player_join(&mut self, player_id: &str) -> Result<()> {
        println!("Player {} joining", player_id);

        // Create the player state
        let player_state = PlayerState {
            position: Vector2::default(),
            velocity: Vector2::default(),
            last_update: Timestamp::now(),
        };

        // Add to game state
        self.game_state
            .players
            .insert(player_id.to_string(), player_state);

        // Send join confirmation if we have their sender
        if let Some(player) = self.players.get(player_id) {
            let join_message = ServerMessage::JoinAccepted {
                player_id: player_id.to_string(),
            };
            let json = serde_json::to_string(&join_message)?;
            player
                .sender
                .send(format!("{}\n", json).into_bytes())
                .map_err(|_| GameError::NetworkError("Failed to send join confirmation".into()))?;
        }

        Ok(())
    }

    fn update_game_state(&mut self, delta_time: Duration) -> Result<()> {
        self.game_state.update(delta_time)
    }

    fn broadcast_state(&mut self, tick: u64) -> Result<()> {
        let update = ServerMessage::GameState(GameStateUpdate {
            tick,
            players: self.game_state.players.clone(),
            server_time: Timestamp::now(),
        });

        let message =
            serde_json::to_string(&update).map(|json| format!("{}\n", json).into_bytes())?;

        let mut disconnected_players = Vec::new();

        for (player_id, player) in &self.players {
            if player.sender.send(message.clone()).is_err() {
                disconnected_players.push(player_id.clone());
            }
        }

        // Clean up disconnected players
        for player_id in disconnected_players {
            self.remove_player(&player_id)?;
        }

        Ok(())
    }

    fn remove_player(&mut self, player_id: &str) -> Result<()> {
        self.players.remove(player_id);
        self.game_state.players.remove(player_id);
        println!("Player {} disconnected", player_id);
        Ok(())
    }
}

// ----------- TESTS ---------
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_game_state_update() {
        let mut state = GameState::new();
        let player_id = "test_player".to_string();
        let initial_state = PlayerState {
            position: Vector2 { x: 0.0, y: 0.0 },
            velocity: Vector2 { x: 1.0, y: 0.0 },
            last_update: Timestamp::now(),
        };

        // Add player
        state.add_player(player_id.clone(), initial_state);
        assert_eq!(state.get_player_count(), 1);

        // Update game state
        let delta = Duration::from_millis(100); // 100ms
        state.update(delta).expect("Update failed");

        // Check position updated according to velocity
        if let Some(updated_state) = state.get_player_state(&player_id) {
            assert!(
                updated_state.position.x > 0.0,
                "Player should have moved right"
            );
            assert_eq!(
                updated_state.position.y, 0.0,
                "Player should not have moved vertically"
            );
        } else {
            panic!("Player state not found");
        }
    }

    #[test]
    fn test_collision_detection() {
        let mut state = GameState::new();
        let rules = GameRules::default();

        // Add two players close to each other
        let player1 = PlayerState {
            position: Vector2 { x: 0.0, y: 0.0 },
            velocity: Vector2 { x: 1.0, y: 0.0 },
            last_update: Timestamp::now(),
        };
        let player2 = PlayerState {
            position: Vector2 {
                x: rules.collision_radius - 1.0,
                y: 0.0,
            },
            velocity: Vector2 { x: -1.0, y: 0.0 },
            last_update: Timestamp::now(),
        };

        state.add_player("player1".to_string(), player1);
        state.add_player("player2".to_string(), player2);

        // Update should trigger collision resolution
        state
            .update(Duration::from_millis(16))
            .expect("Update failed");

        // Check that players were pushed apart
        let p1_state = state
            .get_player_state("player1")
            .expect("Player1 not found");
        let p2_state = state
            .get_player_state("player2")
            .expect("Player2 not found");

        let distance = ((p1_state.position.x - p2_state.position.x).powi(2)
            + (p1_state.position.y - p2_state.position.y).powi(2))
        .sqrt();

        assert!(
            distance >= rules.collision_radius,
            "Players should be pushed apart to at least collision radius"
        );
    }

    #[test]
    fn test_bounds_checking() {
        let mut state = GameState::new();
        let rules = GameRules::default();

        // Add player at edge of map with velocity pointing outward
        let player_state = PlayerState {
            position: Vector2 {
                x: rules.map_bounds.1,
                y: 0.0,
            },
            velocity: Vector2 { x: 10.0, y: 0.0 },
            last_update: Timestamp::now(),
        };

        state.add_player("player1".to_string(), player_state);
        state
            .update(Duration::from_millis(16))
            .expect("Update failed");

        // Check that player position is clamped to map bounds
        let updated_state = state.get_player_state("player1").expect("Player not found");
        assert!(
            updated_state.position.x <= rules.map_bounds.1,
            "Player should not move beyond map bounds"
        );
    }
}
