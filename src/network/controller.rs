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
use tokio::time::sleep;
use tokio::time::Duration;

#[derive(Debug)]
pub enum NetworkControllerEvent {
    CandidateConnection(String, TcpStream, bool),
}

pub struct NetworkController {
    pub peers: Arc<Mutex<HashMap<String, Peers>>>,
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
        let known_peers: HashMap<String, Peers> = serde_json::from_str(&content)?;

        let mut network_controller: NetworkController = NetworkController {
            peers: Arc::new(Mutex::new(known_peers)),
            channel: channel(4096),
        };
        // Listen to new TCP connections
        network_controller
            .create_tcp_listener(
                listen_port,
                max_incoming_connections,
                max_simultaneous_incoming_connection_attempts,
            )
            .await?;

        // Update the peer file
        network_controller
            .manage_peer_file(peers_file_path, peer_file_dump_interval_seconds)
            .await?;

        // TODO: define who is "the most promising peer"
        // Select peer in list who has the right status and is most promising to be connected with
        network_controller
            .peer_connection("127.0.2.51", max_simultaneous_outgoing_connection_attempts)
            .await?;

        // Remove Idle using there last_alive value or if they are None
        Ok(network_controller)
    }

    // Listen to new collection and propagate the info
    async fn create_tcp_listener(
        &mut self,
        port: u32,
        max_incoming_connections: u64,
        max_simultaneous_connection_attempts: u64,
    ) -> Result<(), Box<dyn Error>> {
        // Listener channel
        let tx = self.channel.0.clone();
        let peer_list = self.peers.lock().await;

        // Listen to TCP connection
        let listener = TcpListener::bind(format!("127.0.0.40:{}", port)).await?;
        let mut peer_list = self.peers.lock().await;
        // TODO: enable this
        //tokio::spawn(async move {
        loop {
            let current_inAlive_peer_sum = peer_list
                .clone()
                .into_iter()
                .filter(|(_, v)| v.status == Status::InAlive)
                .collect::<HashMap<String, Peers>>()
                .len() as u64;

            let current_inHandshaking_peer_sum = peer_list
                .clone()
                .into_iter()
                .filter(|(_, v)| v.status == Status::InHandshaking)
                .collect::<HashMap<String, Peers>>()
                .len() as u64;

            println!("{:?}", peer_list.len());
            println!("{:?}", max_incoming_connections);

            // Check if there's still some place to add InAlive and InHandshaking in the peer list else reject connection
            if current_inAlive_peer_sum > max_incoming_connections
                || current_inHandshaking_peer_sum > max_simultaneous_connection_attempts
            {
                println!("Two much peer in this state");
                return Ok(());
            }
            // Accept new incoming connexion
            match listener.accept().await {
                Ok((mut _socket, addr)) => {
                    println!("new client: {:?}", addr.ip());
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

                    if !peer_list.contains_key(&addr.ip().to_string()) {
                        // Add new to connected list
                        peer_list.insert(
                            addr.ip().to_string(),
                            Peers {
                                status: Status::InHandshaking,
                                last_alive: None,
                                last_failure: None,
                            },
                        );
                    } else {
                        // Update peer status to InHandshaking
                        match peer_list.get(&addr.ip().to_string()) {
                            Some(peer) => {
                                peer_list.clone().insert(
                                    addr.ip().to_string(),
                                    Peers {
                                        status: Status::InHandshaking,
                                        last_alive: peer.last_alive,
                                        last_failure: peer.last_failure,
                                    },
                                );
                            }
                            None => {}
                        };
                    }

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
                    println!("couldn't get client: {:?}", e)
                }
            }
        }
        //});

        Ok(())
    }

    // Node try to connect to a peer
    async fn peer_connection(
        &self,
        peer_adress: &str,
        max_outgoing_connection_attempts: u64,
    ) -> Result<(), Box<dyn Error>> {
        let tx = self.channel.0.clone();
        let peer_list = self.peers.lock().await;

        let connection_sum = peer_list
            .clone()
            .into_iter()
            .filter(|(_, v)| {
                v.status == Status::OutConnecting || v.status == Status::OutHandshaking
            })
            .collect::<HashMap<String, Peers>>()
            .len() as u64;

        if connection_sum < max_outgoing_connection_attempts {
            match peer_list.get(&peer_adress.to_string()) {
                Some(peer) => {
                    peer_list.clone().insert(
                        peer_adress.to_string(),
                        Peers {
                            status: Status::OutConnecting,
                            last_alive: peer.last_alive,
                            last_failure: peer.last_failure,
                        },
                    );
                }
                None => {}
            };

            match TcpStream::connect(peer_adress).await {
                Ok(stream) => {
                    match peer_list.get(&peer_adress.to_string()) {
                        Some(peer) => {
                            peer_list.clone().insert(
                                peer_adress.to_string(),
                                Peers {
                                    status: Status::OutHandshaking,
                                    last_alive: peer.last_alive,
                                    last_failure: peer.last_failure,
                                },
                            );
                        }
                        None => {}
                    };

                    tx.send(NetworkControllerEvent::CandidateConnection(
                        String::from(peer_adress),
                        stream,
                        true,
                    ))
                    .await;

                    println!("Connected to the server!");
                }
                Err(err) => {
                    println!("Couldn't connect to server...");
                }
            }
        }
        Ok(())
    }

    // Check if the list have been updated if so update the file
    async fn manage_peer_file(
        &self,
        peers_file_path: &str,
        interval: u64,
    ) -> Result<(), Box<dyn Error>> {
        // TODO: implement tokio spawn
        loop {
            sleep(Duration::from_secs(interval)).await;

            // TODO: Do I need to open it again?
            let mut peers_file = File::open(peers_file_path)?;
            let mut content = String::new();
            peers_file.read_to_string(&mut content)?;
            let known_peers: HashMap<String, Peers> = serde_json::from_str(&content)?;

            // TODO: find a better way
            // Extract data from Arc
            let current_peers_clone = self.peers.lock().await;
            let current_peers: HashMap<String, Peers> = current_peers_clone.clone();

            // Hashmap comparition to rewrite to replace file content with right peers
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

    // Compare equality of two peer list
    // The current list we have and the one in the JSON file
    pub fn compare_list<Peers: Eq + Hash>(
        a: &HashMap<String, Peers>,
        b: &HashMap<String, Peers>,
    ) -> bool {
        let a: HashSet<_> = a.iter().collect();
        let b: HashSet<_> = b.iter().collect();

        a == b
    }

    pub async fn feedback_peer_alive(&mut self, ip: &str) {
        let mut peer_list = self.peers.lock().await;
        match peer_list.get(ip).cloned() {
            Some(peer) => {
                let mut status: Status = Status::InAlive;

                if peer.status == Status::OutHandshaking {
                    status = Status::OutAlive;
                }
                
                peer_list.insert(
                    String::from(ip),
                    Peers {
                        status: status,
                        last_alive: Some(Utc::now()),
                        last_failure: peer.last_failure,
                    },
                )
            }
            None => None,
        };
    }

    pub async fn feedback_peer_banned(&mut self, ip: &str) -> () {
        let mut peer_list = self.peers.lock().await;
        match peer_list.get(ip).cloned() {
            Some(peer) => peer_list.insert(
                String::from(ip),
                Peers {
                    status: Status::Banned,
                    last_alive: peer.last_alive,
                    last_failure: Some(Utc::now()),
                },
            ),
            None => None,
        };
    }

    pub async fn feedback_peer_closed(&mut self, ip: &str) -> () {
        let mut peer_list = self.peers.lock().await;
        match peer_list.get(ip).cloned() {
            Some(peer) => peer_list.insert(
                String::from(ip),
                Peers {
                    status: Status::Idle,
                    last_alive: peer.last_alive,
                    last_failure: peer.last_failure,
                },
            ),
            None => None,
        };
    }

    pub async fn feedback_peer_failed(&self, ip: &str) -> () {
        /*let mut peer_list = self.peers.lock().await;
        match peer_list.get(ip) {
            Some(peer) => {
                peer_list.insert(
                    String::from(ip),
                    Peers {
                        status: Status::Idle,
                        last_alive: peer.last_alive,
                        last_failure: Some(Utc::now()),
                    }
                )
            }
            None => { println!("No peer found for this Ip");}
        };*/
    }

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
}
