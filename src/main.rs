use crate::network::peers::Peers;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::channel;
use network::controller::NetworkControllerEvent;
use std::error::Error;
use tokio::net::{TcpListener, TcpStream};

mod network;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    

    // launch network controller
    let mut net = network::controller::NetworkController::new(
        // peers_file,
        // listen_port,
        // target_outgoing_connections,
        // max_incoming_connections,
        // max_simultaneous_outgoing_connection_attempts,
        // max_simultaneous_incoming_connection_attempts,
        // max_idle_peers,
        // max_banned_peers,
        // peer_file_dump_interval_seconds
        "peers.json",
        8080,
        2,
        2,
        2,
        2,
        4,
        8,
        5,
    ).await?;

    
    // Create a new channel with a capacity of at most 32 (32 message can be receive, the others will wait till some are removed by the receivers)
    let (tx, mut rx): (Sender<NetworkControllerEvent>, Receiver<NetworkControllerEvent>) = channel(32);
    

    // loop over messages coming from the network controller
    loop {
        tokio::select! {
            // Receive info from listener
            /*evt = rx.recv() => match evt {
                Some(msg) => match msg {
                },
                None => println!("Error with channel"),
            };*/

            evt = net.wait_event() => match evt {
                Ok(msg) => match msg {
                        network::controller::NetworkControllerEvent::CandidateConnection (ip, socket, is_outgoing) => {
                            
                            match Peers::make_handshake(ip, socket, is_outgoing).await {
                                Ok(peer) => {
                                    //net.feedback_peer_alive(ip).await;

                                    //net.feedback_peer_list(list_of_ips).await;
                                }
                                Err(e) => {
                                    //net.feedback_peer_failed(ip).await;
                                }
                            }
                    }

                },
                Err(e) => return Err(e.into())
            }
        }
    }
}
