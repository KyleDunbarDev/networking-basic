use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Join,
    Move { direction: Vector2 },
    Disconnect,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    JoinAccepted { player_id: String },
    GameState(GameStateUpdate),
    Error { message: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GameStateUpdate {
    pub tick: u64,
    pub players: HashMap<String, PlayerState>,
    pub server_time: Timestamp,
}
