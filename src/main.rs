use networking_basic::{common::Result, server::GameServer};

fn main() -> Result<()> {
    let mut server = GameServer::new("127.0.0.1:8080")?;
    server.run()
}

// How to run:
// # Terminal 1
// cargo run --bin server

// # Terminal 2
// cargo run --bin client -- --client-id 0

// # Terminal 3
// cargo run --bin client -- --client-id
