use networking_basic::{
    common::{ClientMessage, Result, Vector2},
    server::GameServer,
};
use std::{thread, time::Duration};

fn main() -> Result<()> {
    let mut server = GameServer::new("127.0.0.1:8080")?;
    server.run()
}
