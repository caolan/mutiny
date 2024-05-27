use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub enum Request {
    Ping,
}

#[derive(Serialize)]
pub enum Response {
    Pong,
}

