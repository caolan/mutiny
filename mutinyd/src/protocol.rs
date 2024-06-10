use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
pub struct Peer {
    pub id: String,
    pub addr: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub id: String,
    pub version: String,
}

#[derive(Deserialize)]
pub enum Request {
    CreateAppInstance {
        name: String,
        manifest: Manifest
    },
    AppInstanceUuid(String),
    LocalPeerId,
    Peers,
    Ping,
    AcceptMessages {
        peer: String,
        application_instance_uuid: String,
    },
}

#[derive(Serialize, Debug)]
pub enum Response {
    Success,
    Error(String),
    CreateAppInstance(String),
    AppInstanceUuid(Option<String>),
    LocalPeerId(String),
    Peers(Vec<Peer>),
    Pong,
}
