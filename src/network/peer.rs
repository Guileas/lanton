use tokio::io::AsyncWriteExt;
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
pub struct Peer {
    pub status: Status,
    pub last_alive: Option<DateTime<Utc>>,  
    pub last_failure: Option<DateTime<Utc>>,
}

impl Peer {
    pub fn new() -> Self {
        Peer {
            status: Status::Idle,
            last_alive: None,
            last_failure: None,
        }
    }

    pub async fn make_handshake(
        mut socket: TcpStream,
    ) -> Result<(), Box<dyn Error>> {
        // write data to socket
        socket
        .write_all(b"handshake")
        .await?;

        Ok(())
    }
}
