use crate::common::{ClientMessage, GameError, InternalMessage, Result, ServerMessage, Vector2};
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

// ------------- TESTS -----------
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    struct TestClient {
        stream: TcpStream,
    }

    impl TestClient {
        fn new(address: &str) -> Result<Self> {
            let stream = TcpStream::connect(address)?;
            stream.set_nonblocking(true)?;
            Ok(Self { stream })
        }

        fn send_message(&mut self, message: &ClientMessage) -> Result<()> {
            let json = serde_json::to_string(message)?;
            self.stream.write_all(format!("{}\n", json).as_bytes())?;
            self.stream.flush()?;
            Ok(())
        }

        fn receive_message(&mut self) -> Result<Option<ServerMessage>> {
            let mut reader = std::io::BufReader::new(&self.stream);
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => Ok(None),
                Ok(_) => Ok(Some(serde_json::from_str(&line)?)),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                Err(e) => Err(GameError::IoError(e)),
            }
        }
    }

    struct TestServer {
        address: String,
        shutdown: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
        message_sender: std::sync::mpsc::Sender<InternalMessage>,
    }

    impl TestServer {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind test server");
            listener
                .set_nonblocking(true)
                .expect("Failed to set non-blocking");
            let server_addr = listener
                .local_addr()
                .expect("Failed to get local address")
                .to_string();

            let (tx, _rx) = std::sync::mpsc::channel();
            let message_sender = tx.clone();
            let shutdown = Arc::new(AtomicBool::new(false));
            let shutdown_flag = shutdown.clone();

            let handle = thread::spawn(move || {
                while !shutdown_flag.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let message_sender = message_sender.clone();
                            let player_id = stream
                                .peer_addr()
                                .map(|addr| addr.to_string())
                                .unwrap_or_else(|_| "unknown".to_string());

                            let (client_sender, _) = channel();

                            let _ = message_sender.send(InternalMessage::NewConnection {
                                player_id: player_id.clone(),
                                sender: client_sender,
                            });

                            let message_sender_clone = message_sender.clone();
                            thread::spawn(move || {
                                if let Err(e) = handle_client_reader(
                                    stream,
                                    player_id.clone(),
                                    message_sender_clone,
                                ) {
                                    eprintln!("Test client error: {}", e);
                                }
                            });
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        Err(e) => eprintln!("Accept failed: {}", e),
                    }
                }
            });

            TestServer {
                address: server_addr,
                shutdown,
                handle: Some(handle),
                message_sender: tx,
            }
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::Relaxed);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    #[test]
    fn test_client_connection() {
        let server = TestServer::new();

        // Try to connect with backoff
        let mut client = None;
        for i in 0..5 {
            match TestClient::new(&server.address) {
                Ok(c) => {
                    client = Some(c);
                    break;
                }
                Err(e) => {
                    if i == 4 {
                        panic!("Failed to connect after 5 attempts: {}", e);
                    }
                    thread::sleep(Duration::from_millis(100 * (i + 1)));
                }
            }
        }
        let mut client = client.unwrap();

        // Send Join message
        client
            .send_message(&ClientMessage::Join)
            .expect("Failed to send join");

        // Wait a bit for server processing
        thread::sleep(Duration::from_millis(100));

        // Cleanup happens automatically when server and client are dropped
    }

    #[test]
    fn test_client_movement() {
        let server = TestServer::new();

        // Connect client with retry
        let mut client = None;
        for i in 0..5 {
            match TestClient::new(&server.address) {
                Ok(c) => {
                    client = Some(c);
                    break;
                }
                Err(e) => {
                    if i == 4 {
                        panic!("Failed to connect after 5 attempts: {}", e);
                    }
                    thread::sleep(Duration::from_millis(100 * (i + 1)));
                }
            }
        }
        let mut client = client.unwrap();

        // Join game
        client
            .send_message(&ClientMessage::Join)
            .expect("Failed to send join");

        // Wait for processing
        thread::sleep(Duration::from_millis(100));

        // Send movement
        let move_msg = ClientMessage::Move {
            direction: Vector2 { x: 1.0, y: 0.0 },
        };
        client
            .send_message(&move_msg)
            .expect("Failed to send movement");

        // Wait for processing
        thread::sleep(Duration::from_millis(100));

        // Cleanup happens automatically when server and client are dropped
    }
}
