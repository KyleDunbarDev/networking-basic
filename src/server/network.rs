use crate::common::{ClientMessage, GameError, InternalMessage, Result};
use std::{
    io::{BufRead, Write},
    sync::mpsc::{channel, Receiver, Sender},
};
pub struct PlayerConnection {
    pub player_id: String,
    pub sender: Sender<Vec<u8>>,
}

pub fn handle_connections(address: &str, message_sender: Sender<InternalMessage>) -> Result<()> {
    let listener = std::net::TcpListener::bind(address)?;
    println!("Listening for connections on {}", address);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let message_sender = message_sender.clone();

                let player_id = stream
                    .peer_addr()
                    .map(|addr| addr.to_string())
                    .unwrap_or_else(|_| "unknown".to_string());

                // Create message channel for this client
                let (client_sender, client_receiver) = channel();

                // Register the new connection
                message_sender
                    .send(InternalMessage::NewConnection {
                        player_id: player_id.clone(),
                        sender: client_sender,
                    })
                    .map_err(|_| GameError::NetworkError("Failed to register connection".into()))?;

                // Clone stream for writer thread
                let write_stream = stream.try_clone()?;

                // Spawn writer thread
                std::thread::spawn(move || {
                    if let Err(e) = handle_client_writer(write_stream, client_receiver) {
                        eprintln!("Writer thread error: {}", e);
                    }
                });

                // Spawn reader thread
                let message_sender_clone = message_sender.clone();
                let player_id_clone = player_id.clone();
                std::thread::spawn(move || {
                    if let Err(e) =
                        handle_client_reader(stream, player_id_clone, message_sender_clone)
                    {
                        eprintln!("Client error for {}: {}", player_id, e);
                    }
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }
    Ok(())
}

fn handle_client_reader(
    stream: std::net::TcpStream,
    player_id: String,
    message_sender: Sender<InternalMessage>,
) -> Result<()> {
    let mut reader = std::io::BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => match serde_json::from_str::<ClientMessage>(&line) {
                Ok(message) => {
                    message_sender
                        .send(InternalMessage::ClientMessage {
                            player_id: player_id.clone(),
                            message,
                        })
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
    let _ = message_sender.send(InternalMessage::ClientMessage {
        player_id: player_id.clone(),
        message: ClientMessage::Disconnect,
    });
    Ok(())
}

fn handle_client_writer(
    mut stream: std::net::TcpStream,
    receiver: Receiver<Vec<u8>>,
) -> Result<()> {
    for message in receiver {
        stream.write_all(&message)?;
        stream.flush()?;
    }
    Ok(())
}
