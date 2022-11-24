use std::error::Error;

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
       30,
    ).await?;

    Ok(())
    // loop over messages coming from the network controller
    /*loop {
        tokio::select! {
            evt = net.wait_event() => match evt {
                Ok(msg) => match msg {
                    network::controller::NetworkControllerEvent::CandidateConnection (ip, socket, is_outgoing) => {

                    }
                },
                Err(e) => return Err(e)
            }
        }
    }*/
    
    
}