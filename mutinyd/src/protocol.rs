use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
pub struct Peer {
    pub id: String,
    pub addr: String,
}

#[derive(Deserialize)]
pub enum Request {
    LocalPeerId,
    Peers,
    Ping,
}

#[derive(Serialize, Debug)]
pub enum Response {
    LocalPeerId(String),
    Peers(Vec<Peer>),
    Pong,
}

