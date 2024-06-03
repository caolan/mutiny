use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub enum Request {
    Ping,
}

#[derive(Serialize, Debug, Clone)]
pub enum Response {
    Pong,
}

