use std::error::Error;
use std::{fs::File, io::Read};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::network::peers::Peers;

pub enum NetworkControllerEvent {
    CandidateConnection(String, TcpStream, bool),
}

pub struct NetworkController {}

impl NetworkController {
    pub async fn new(
        peers_file_path: &str,
        listen_port: u32,
        target_outgoing_connections: u64,
        max_incoming_connections: u64,
        max_simultaneous_outgoing_connection_attempts: u64,
        max_simultaneous_incoming_connection_attempts: u64,
        max_idle_peers: u64,
        max_banned_peers: u64,
        peer_file_dump_interval_seconds: u64,
    ) -> Result<(), Box<dyn Error>> {

        //Load peers from JSON file
        let mut peers_file = File::open(peers_file_path)?;
        let mut content = String::new();
        peers_file.read_to_string(&mut content)?;
        //TODO: add type
        let known_peers: Vec<Peers> = serde_json::from_str(&content)?;

        // Listen to TCP connection
        let listener = TcpListener::bind(format!("127.0.0.40:{}", listen_port)).await?;

        loop {
            // Accept new incoming connexion
            match listener.accept().await {
                Ok((mut _socket, addr)) => {
                    println!("new client: {:?}", addr);
                    // A new task is spawned for each inbound socket.
                    // The socket is moved to the new task and processed there.
                    tokio::spawn(async move {
                        
                    });
                }
                Err(e) => println!("couldn't get client: {:?}", e)
            }
        }
    }

    pub async fn feedback_peer_alive(ip: &str) {
        // should update last_alive
    }

    pub async fn feedback_peer_banned(ip: &str) {
        //set the peer status to Banned
        // should update last_failure
    }

    pub async fn feedback_peer_closed(ip: &str) {
        // set the peer status to Idle
    }

    /**
     * after handshake, and then again periodically, main.rs should ask alive peers for the list of peer IPs they know about, and feed them to the network controller:
     */
    pub async fn feedback_peer_list(list_of_ips: Vec<&str>) {
        // should merge the new peers to the existing peer list in a smart way
    }
}
