use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub enum Request {
    LocalPeerId,
    Ping,
}

#[derive(Serialize, Debug, Clone)]
pub enum Response {
    LocalPeerId(String),
    Pong,
}

