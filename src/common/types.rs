use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Timestamp(u64);

impl Timestamp {
    pub fn now() -> Self {
        Self(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )
    }
    pub fn duration_since(&self, other: &Timestamp) -> Duration {
        Duration::from_millis((self.0.saturating_sub(other.0)) as u64)
    }

    pub fn elapsed(&self) -> Duration {
        Timestamp::now().duration_since(self)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, Copy)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector2 {
    pub fn add(&self, other: &Vector2) -> Vector2 {
        Vector2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
    pub fn scale(&self, factor: f32) -> Vector2 {
        Vector2 {
            x: self.x * factor,
            y: self.y * factor,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PlayerState {
    pub position: Vector2,
    pub velocity: Vector2,
    pub last_update: Timestamp,
}
