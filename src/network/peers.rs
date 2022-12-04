use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::error::Error;
use tokio::net::TcpStream;

#[derive(Debug, Copy, Clone, Serialize, Deserialize,PartialEq,Eq,Hash)]
pub enum Status {
    Idle,
    OutConnecting,
    OutHandshaking,
    OutAlive,
    InHandshaking,
    InAlive,
    Banned,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq,Eq,Hash)]
pub struct Peers {
    pub status: Status,
    pub last_alive: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
}

impl Peers {
    pub fn new() -> Self {
        Peers {
            status: Status::Idle,
            last_alive: None,
            last_failure: None,
        }
    }

    pub async fn make_handshake(
        ip: String,
        socket: TcpStream,
        is_outgoing: bool,
    ) -> Result<Peers, Box<dyn Error>> {
        // write data to socket
        /*_socket
        .write_all(&buf[0..n])
        .await
        .expect("failed to write data to socket");*/

        Ok(Peers::new())
    }

    /*pub fn update_peer(&mut self, status: Status) -> Result<&Self, Box<dyn Error>>{
        self.status = status;
        match self.status {

            Status::Banned  => self.last_failure = Some(Utc::now())
        };

        Ok(self)
    }*/

    pub async fn manage_peer_banned() {}

    pub async fn manage_idle_peers() {}
}
