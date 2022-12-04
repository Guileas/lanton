use tokio::time::sleep;
use tokio::time::Duration;
use crate::network::peers::{Peers, Status};
use chrono::Utc;
use std::any::Any;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fs::OpenOptions;
use std::hash::Hash;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::{fs::File, io::Read};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;

pub enum NetworkControllerEvent {
    CandidateConnection(String, TcpStream, bool),
}

pub struct NetworkController {
    pub peers: Arc<Mutex<Vec<Peers>>>,
    pub channel: (
        Sender<NetworkControllerEvent>,
        Receiver<NetworkControllerEvent>,
    ),
}

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
    ) -> Result<NetworkController, Box<dyn Error>> {
        //Load peers from JSON file
        let mut peers_file = File::open(peers_file_path)?;
        let mut content = String::new();
        peers_file.read_to_string(&mut content)?;
        //TODO: add type
        let mut known_peers: Vec<Peers> = serde_json::from_str(&content)?;

        let networkController: NetworkController = NetworkController {
            peers: Arc::new(Mutex::new(known_peers)),
            channel: channel(4096),
        };

        // wait peer_file_dump_interval_seconds before playing this function (loop)
        networkController.manage_peer_file(peers_file_path, peer_file_dump_interval_seconds).await?;

        networkController.create_tcp_listener(listen_port).await?;
        // TODO: define who is "the most promising peer"
        //networkController.peer_connection().await?;
        //networkController.manage_peer_file(peers_file,peer_file_dump_interval_seconds).await?;

        Ok(networkController)
    }

    // Listen to new collection and propagate the info
    async fn create_tcp_listener(&self, port: u32) -> Result<(), Box<dyn Error>> {
        // Listener channel
        let tx = self.channel.0.clone();

        // Listen to TCP connection
        let listener = TcpListener::bind(format!("127.0.0.40:{}", port)).await?;

        //tokio::spawn(async move {
        loop {
            // Accept new incoming connexion
            match listener.accept().await {
                Ok((mut _socket, addr)) => {
                    println!("new client: {:?}", addr);
                    //TO REMOVE TEST TCP CONNECTION
                    /*loop {
                        println!("Blob is here!");

                        // In a loop, read data from the socket and write the data back.
                        loop {
                            let mut buf = vec![0; 1024];
                            let n = _socket
                                .read(&mut buf)
                                .await
                                .expect("failed to read data from socket");

                            if n == 0 {
                                //return;
                            }

                            _socket
                                .write_all(&buf[0..n])
                                .await
                                .expect("failed to write data to socket");
                        }
                    }*/

                    // If peer exist
                    // -> Set peer status to InHandshaking
                    // Else
                    // -> add it to peer list

                    // Peer is connected to you node
                    tx.send(NetworkControllerEvent::CandidateConnection(
                        addr.ip().to_string(),
                        _socket,
                        false,
                    ))
                    .await;
                }
                Err(e) => {
                    // Set peer status to
                    // Set peer last_failure
                    //println!("couldn't get client: {:?}", e)
                }
            }
        }
        //});

        Ok(())
    }

    // Node try to connect to a peer
    async fn peer_connection(&self, peer_adress: &str) -> Result<(), Box<dyn Error>> {
        let tx = self.channel.0.clone();

        match TcpStream::connect(peer_adress).await {
            Ok(stream) => {
                tx.send(NetworkControllerEvent::CandidateConnection(
                    String::from(peer_adress),
                    stream,
                    true,
                ));
                println!("Connected to the server!");
            }
            Err(err) => {
                println!("Couldn't connect to server...");
            }
        }

        Ok(())
    }

    // Check if the list have been updated if so update the file
    async fn manage_peer_file(&self, peers_file_path: &str, interval: u64) -> Result<(), Box<dyn Error>> {
        // TODO: implement tokio spawn
        loop {
            sleep(Duration::from_secs(interval)).await;
            
            // TODO: Do I need to open it again?
            let mut peers_file = File::open(peers_file_path)?;
            let mut content = String::new();
            peers_file.read_to_string(&mut content)?;
            let known_peers = serde_json::from_str(&content)?;

            // TODO: find a better way
            // Extract data from Arc
            let current_peers_clone = self.peers.lock().await;
            let current_peers: Vec<Peers> = current_peers_clone.to_vec();

            // Vec comparition to rewrite to replace file content with right peers
            if !NetworkController::compare_list(&current_peers, &known_peers) {

                let peers_to_write = serde_json::to_string(&current_peers)?;
                // Empty file and write current peers in it
                let mut file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(peers_file_path)?;
                file.write(peers_to_write.as_bytes())?;
            }
        }
    }

    pub fn compare_list<Peers: Eq + Hash>(a: &Vec<Peers>, b: &Vec<Peers>) -> bool {
        let a: HashSet<_> = a.iter().collect();
        let b: HashSet<_> = b.iter().collect();

        a == b
    }

    // pub async fn feedback_peer_alive(&mut self, ip: &str) -> () {
    //     // TODO: set the peer in InAlive or OutAlive state
    //     let peer = self.peers.get(ip).unwrap();
    //     self.peers.insert(
    //         String::from(ip),
    //         Peers {
    //             status: peer.status,
    //             last_alive: Some(Utc::now()),
    //             last_failure: peer.last_failure,
    //         },
    //     );
    // }

    // pub async fn feedback_peer_banned(&mut self, ip: &str) -> () {
    //     self.peers.update_peer(Status::Banned)
    // }

    // pub async fn feedback_peer_closed(&mut self, ip: &str) -> () {
    //     let peer = self.peers.get(ip).unwrap();
    //     self.peers.insert(
    //         String::from(ip),
    //         Peers {
    //             status: Status::Idle,
    //             last_alive: peer.last_alive,
    //             last_failure: peer.last_failure,
    //         },
    //     );
    // }

    // pub async fn feedback_peer_failed(&mut self, ip: &str) -> () {
    //     let peer = self.peers.get(ip).unwrap();
    //     self.peers.insert(
    //         String::from(ip),
    //         Peers {
    //             status: Status::Idle,
    //             last_alive: peer.last_alive,
    //             last_failure: Some(Utc::now()),
    //         },
    //     );
    // }

    //after handshake, and then again periodically, main.rs should ask alive peers for the list of peer IPs they know about, and feed //them to the network controller:
    pub async fn feedback_peer_list(&mut self, list_of_ips: Vec<&str>) -> () {
        // should merge the new peers to the existing peer list in a smart way
    }

    pub fn get_good_peer_ips(&mut self) -> () {
        // get  list of peer IPs we know about,
        // excludes banned peers and sorts the peers from "best" to "worst"
    }

    // Get message from peer channel
    pub async fn wait_event(&mut self) -> Result<NetworkControllerEvent, String> {
        self.channel
            .1
            .recv()
            .await
            .ok_or(String::from("Channel closed"))
    }

    /*pub async fn wait_event(&self) -> Result<Box<dyn Any>, Box<dyn Error>> {
        // TODO: remove this
        Ok(Box::new(String::from("Heya!")))
        rx.recv().await {

        }
        //let connection = TcpStream::connect("127.0.0.40:8080");
        //Ok(NetworkControllerEvent::CandidateConnection("".to_string(),connection,true))
    }*/
}
