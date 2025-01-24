// Example
use crate::common::{ClientMessage, PlayerState, ServerMessage, Vector2};

struct GameClient {
    // Client-specific fields
}

impl GameClient {
    fn send_move(&mut self, direction: Vector2) {
        let msg = ClientMessage::Move { direction };
        // Send message logic...
    }

    fn handle_server_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::GameState(update) => {
                // Handle state update...
            }
            ServerMessage::JoinAccepted { player_id } => {
                // Handle join...
            } // ...
            _ => {} // Change to Err =>
        }
    }
}
