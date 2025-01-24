use crate::common::{ClientMessage, GameError, Result};
use std::{io::BufRead, sync::mpsc::Sender};

pub fn handle_connections(
    address: &str,
    input_sender: Sender<(String, ClientMessage)>,
) -> Result<()> {
    let listener = std::net::TcpListener::bind(address)?;
    println!("Listening for connections on {}", address);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let input_sender = input_sender.clone();
                let player_id = stream
                    .peer_addr()
                    .map(|addr| addr.to_string())
                    .unwrap_or_else(|_| "unknown".to_string());

                std::thread::spawn(move || {
                    if let Err(e) = handle_client(stream, player_id.clone(), input_sender) {
                        eprintln!("Client error for {}: {}", player_id, e);
                    }
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }
    Ok(())
}

fn handle_client(
    stream: std::net::TcpStream,
    player_id: String,
    input_sender: Sender<(String, ClientMessage)>,
) -> Result<()> {
    let mut reader = std::io::BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => match serde_json::from_str::<ClientMessage>(&line) {
                Ok(message) => {
                    input_sender
                        .send((player_id.clone(), message))
                        .map_err(|_| GameError::NetworkError("Failed to forward message".into()))?;
                }
                Err(e) => eprintln!("Failed to parse message from {}: {}", player_id, e),
            },
            Err(e) => {
                return Err(GameError::IoError(e));
            }
        }
    }

    // Send disconnect message when client disconnects
    let _ = input_sender.send((player_id.clone(), ClientMessage::Disconnect));

    Ok(())
}
