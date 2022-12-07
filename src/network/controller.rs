use crate::network::peer::{Peer, Status};
use chrono::Utc;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fs::OpenOptions;
use std::hash::Hash;
use std::io::Write;
use std::sync::Arc;
use std::{fs::File, io::Read};
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
    pub peers: Arc<Mutex<HashMap<String, Peer>>>,
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
        let known_peers: HashMap<String, Peer> = serde_json::from_str(&content)?;

        let mut network_controller: NetworkController = NetworkController {
            peers: Arc::new(Mutex::new(known_peers)),
            channel: channel(4096),
        };

        //network_controller.regulate_idle_peers(max_idle_peers).await;
        //network_controller.regulate_banned_peers(max_banned_peers).await;

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

        // Launch TCP connection to peer when there's not enought outgoing connections
        if network_controller.regulate_outgoing_connection().await < target_outgoing_connections {
            // TODO: define who is "the most promising peer"
            network_controller
                .peer_connection("127.0.2.51", max_simultaneous_outgoing_connection_attempts)
                .await?;
        }

        // Remove Idle using there last_alive value or if they are None
        Ok(network_controller)
    }

    // Count the number of current OutAlive peers
    async fn regulate_outgoing_connection(&self) -> u64 {
        let peer_list = self.peers.lock().await;
        peer_list
            .clone()
            .into_iter()
            .filter(|(_, v)| v.status == Status::OutAlive)
            .collect::<HashMap<String, Peer>>()
            .len() as u64
    }

    // Remove idle peers from list if there's too much of them
    async fn regulate_idle_peers(&self, max_idle_peers: u64) {
        let peer_list = self.peers.lock().await;
        let current_idle_peer_sum = peer_list
            .clone()
            .into_iter()
            .filter(|(_, v)| v.status == Status::Idle)
            .collect::<HashMap<String, Peer>>();

        // TODO: Remove from peer list in this order:
        // 1 - peer with Idle, last_alive None and oldest last_failure
        // 2 - peer with Idle, last_alive oldest and oldest last_failure
        // 2 - peer with Idle, last_alive oldest and last_failure None
    }

    // Remove banned peers from list if there's too much of them
    async fn regulate_banned_peers(&self, max_banned_peers: u64) {
        let peer_list = self.peers.lock().await;
        let current_idle_peer_sum = peer_list
            .clone()
            .into_iter()
            .filter(|(_, v)| v.status == Status::Banned)
            .collect::<HashMap<String, Peer>>();

        // TODO: Remove from peer list in this order:
        // 1 - peer with Banned, last_alive None and oldest last_failure
        // 2 - peer with Idle, last_alive oldest and oldest last_failure
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
        let mut peer_list = self.peers.lock().await;

        // Listen to TCP connection
        let listener = TcpListener::bind(format!("127.0.0.40:{}", port)).await?;

        // TODO: enable tokio async loop
        //tokio::spawn(async move {
        loop {
            let current_in_alive_peer_sum = peer_list
                .clone()
                .into_iter()
                .filter(|(_, v)| v.status == Status::InAlive)
                .collect::<HashMap<String, Peer>>()
                .len() as u64;

            let current_in_handshaking_peer_sum = peer_list
                .clone()
                .into_iter()
                .filter(|(_, v)| v.status == Status::InHandshaking)
                .collect::<HashMap<String, Peer>>()
                .len() as u64;

            // Check if there's still some place to add InAlive and InHandshaking in the peer list else reject connection
            if current_in_alive_peer_sum > max_incoming_connections
                || current_in_handshaking_peer_sum > max_simultaneous_connection_attempts
            {
                return Ok(());
            }
            // Accept new incoming connexion
            match listener.accept().await {
                Ok((mut _socket, addr)) => {
                    if !peer_list.contains_key(&addr.ip().to_string()) {
                        // Add new to connected list
                        peer_list.insert(
                            addr.ip().to_string(),
                            Peer {
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
                                    Peer {
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
                Err(e) => {}
            }
        }
        //});
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
            .collect::<HashMap<String, Peer>>()
            .len() as u64;

        if connection_sum < max_outgoing_connection_attempts {
            // Set peer status to OutConnecting
            match peer_list.get(&peer_adress.to_string()) {
                Some(peer) => {
                    peer_list.clone().insert(
                        peer_adress.to_string(),
                        Peer {
                            status: Status::OutConnecting,
                            last_alive: peer.last_alive,
                            last_failure: peer.last_failure,
                        },
                    );
                }
                None => {}
            };
            // Connect to peer, set it status to OutHandshaking & send msg in channel
            match TcpStream::connect(peer_adress).await {
                Ok(stream) => {
                    match peer_list.get(&peer_adress.to_string()) {
                        Some(peer) => {
                            peer_list.clone().insert(
                                peer_adress.to_string(),
                                Peer {
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
                }
                Err(err) => {}
            }
        }
        Ok(())
    }

    // If peers have been updated write them in the JSON file
    async fn manage_peer_file(
        &self,
        peers_file_path: &str,
        interval: u64,
    ) -> Result<(), Box<dyn Error>> {
        // TODO: implement tokio spawn
        loop {
            // Wait some times before checking again
            sleep(Duration::from_secs(interval)).await;

            // TODO: Do I need to open it again?
            let mut peers_file = File::open(peers_file_path)?;
            let mut content = String::new();
            peers_file.read_to_string(&mut content)?;
            let known_peers: HashMap<String, Peer> = serde_json::from_str(&content)?;

            // TODO: find a better way to extract data from Arc
            // Extract data from Arc
            let current_peers_clone = self.peers.lock().await;
            let current_peers: HashMap<String, Peer> = current_peers_clone.clone();

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
    pub fn compare_list<Peer: Eq + Hash>(
        a: &HashMap<String, Peer>,
        b: &HashMap<String, Peer>,
    ) -> bool {
        let a: HashSet<_> = a.iter().collect();
        let b: HashSet<_> = b.iter().collect();

        a == b
    }

    // Set peer status to InAlive or OutAlive & update last_alive
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
                    Peer {
                        status: status,
                        last_alive: Some(Utc::now()),
                        last_failure: peer.last_failure,
                    },
                )
            }
            None => None,
        };
    }

    // Set peer status to Banned & update last_failure
    pub async fn feedback_peer_banned(&mut self, ip: &str) -> () {
        let mut peer_list = self.peers.lock().await;
        match peer_list.get(ip).cloned() {
            Some(peer) => peer_list.insert(
                String::from(ip),
                Peer {
                    status: Status::Banned,
                    last_alive: peer.last_alive,
                    last_failure: Some(Utc::now()),
                },
            ),
            None => None,
        };
    }

    // Set peer status to Idle
    pub async fn feedback_peer_closed(&mut self, ip: &str) -> () {
        let mut peer_list = self.peers.lock().await;
        match peer_list.get(ip).cloned() {
            Some(peer) => peer_list.insert(
                String::from(ip),
                Peer {
                    status: Status::Idle,
                    last_alive: peer.last_alive,
                    last_failure: peer.last_failure,
                },
            ),
            None => None,
        };
    }

    // Set peer status to Idle & update last_failure
    pub async fn feedback_peer_failed(&self, ip: &str) -> () {
        let mut peer_list = self.peers.lock().await;
        match peer_list.get(ip).cloned() {
            Some(peer) => peer_list.insert(
                String::from(ip),
                Peer {
                    status: Status::Idle,
                    last_alive: peer.last_alive,
                    last_failure: Some(Utc::now()),
                },
            ),
            None => None,
        };
    }

    //after handshake, and then again periodically, main.rs should ask alive peers for the list of peer IPs they know about, and feed //them to the network controller:
    pub async fn feedback_peer_list(&mut self, list_of_ips: Vec<&str>) -> () {
        let mut peer_list = self.peers.lock().await;

        // TODO: merge the new peers to the existing peer list in a smart way
        for ip in list_of_ips {
            // If peer is already known don't do anything
            if peer_list.contains_key(ip) {
                return;
            }

            // Create a new peer and add it to the current known peers
            let new_peer: Peer = Peer::new();
            peer_list.insert(ip.to_string(), new_peer);
        }
    }

    // Send list of peer IPs we know about to peers
    pub async fn get_good_peer_ips(&mut self) -> Vec<String> {
        let peer_list = self.peers.lock().await;

        // TODO: sorts the peers from "best" to "worst"
        /*
         * Best
         * last_alive newest, last_failure None,
         * last_alive newest, last_failure oldest,
         *
         * last_alive oldest, last_failure newest
         * last_alive None, last_failure newest
         * Worst
         *
         * or by status
         * or combining both
         */
        let good_peer_ips = peer_list
            .clone()
            .into_iter()
            .filter(|(_, v)| v.status != Status::Banned)
            .collect::<HashMap<String, Peer>>();
        let mut ip_vec: Vec<String> = Vec::new();

        for ip in good_peer_ips.keys() {
            ip_vec.push(ip.to_string());
        }
        ip_vec
    }

    // Get message from channel
    pub async fn wait_event(&mut self) -> Result<NetworkControllerEvent, String> {
        self.channel
            .1
            .recv()
            .await
            .ok_or(String::from("Channel closed"))
    }
}
