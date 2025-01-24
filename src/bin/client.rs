use clap::Parser;
use networking_basic::{
    client::GameClient,
    common::{ClientMessage, Result, Vector2},
    server::GameServer,
};
use std::{thread, time::Duration};

#[derive(Parser)]
struct Opts {
    #[clap(short, long, default_value = "0")]
    client_id: usize,
}

fn main() -> Result<()> {
    let opts = Opts::parse();

    let mut client = GameClient::new("127.0.0.1:8080")?;
    client.connect()?;
    println!("Client {} connected", opts.client_id);

    loop {
        let direction = Vector2 {
            x: (opts.client_id as f32).cos(),
            y: (opts.client_id as f32).sin(),
        };
        client.move_player(direction)?;
        client.update()?;
        thread::sleep(Duration::from_millis(100));
    }
}
