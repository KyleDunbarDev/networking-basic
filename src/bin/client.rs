use clap::Parser;
use networking_basic::{
    client::GameClient,
    common::{ClientMessage, Result, Vector2},
    logging,
};
use std::{thread, time::Duration};
use tracing::{debug, info, warn};

#[derive(Parser)]
struct Opts {
    #[clap(short, long, default_value = "0")]
    client_id: usize,

    #[clap(short, long)]
    debug: bool,
}

fn main() -> Result<()> {
    // Initialize logging
    logging::init();

    let opts = Opts::parse();
    info!("Starting client {}", opts.client_id);

    let mut client = GameClient::new("127.0.0.1:8080")?;
    info!("Connecting to server...");

    client.connect()?;
    info!("Client {} connected", opts.client_id);

    let mut last_debug = std::time::Instant::now();
    let mut last_move = std::time::Instant::now();
    let move_interval = Duration::from_millis(100);

    loop {
        // Send movement updates at fixed interval
        if last_move.elapsed() >= move_interval {
            let direction = Vector2 {
                x: (opts.client_id as f32).cos(),
                y: (opts.client_id as f32).sin(),
            };

            if let Err(e) = client.move_player(direction) {
                warn!("Failed to send movement: {}", e);
            } else {
                debug!("Sent movement: ({:.2}, {:.2})", direction.x, direction.y);
            }

            last_move = std::time::Instant::now();
        }

        // Update client state
        if let Err(e) = client.update() {
            warn!("Failed to update client: {}", e);
        }

        // Print debug info every second if enabled
        if opts.debug && last_debug.elapsed() >= Duration::from_secs(1) {
            // Clear screen and move cursor to top-left
            print!("\x1B[2J\x1B[1;1H");
            println!("=== Debug Information ===");
            println!("{}", client.debug_info());
            println!("========================");

            last_debug = std::time::Instant::now();
        }

        // Log periodic state information
        if last_debug.elapsed() >= Duration::from_secs(5) {
            if let Some(state) = client.get_own_state() {
                info!(
                    "Current state - pos: ({:.2}, {:.2}), vel: ({:.2}, {:.2})",
                    state.position.x, state.position.y, state.velocity.x, state.velocity.y
                );
            }
            last_debug = std::time::Instant::now();
        }

        // Small sleep to prevent busy-waiting
        thread::sleep(Duration::from_millis(16));
    }
}
