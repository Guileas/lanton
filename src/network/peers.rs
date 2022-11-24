use chrono::{Utc,DateTime};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
enum Status {
    Idle,
    OutConnecting,
    OutHandshaking,
    OutAlive,
    InHandshaking,
    InAlive,
    Banned,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Peers {
    status: Status,
    last_alive: Option<DateTime<Utc>>,
    last_failure: Option<DateTime<Utc>>
}

impl Peers {

    pub fn new() -> Self {
        Peers{ 
            status: Status::Idle, 
            last_alive: None, 
            last_failure: None 
        }
    }


    pub fn get_good_peer_ips() {
        // get  list of peer IPs we know about,
        // excludes banned peers and sorts the peers from "best" to "worst"
    }

}