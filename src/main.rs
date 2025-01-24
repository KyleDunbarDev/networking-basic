use networking_basic::{common::Result, server::GameServer};

fn main() -> Result<()> {
    let mut server = GameServer::new("127.0.0.1:8080")?;
    server.run()
}
