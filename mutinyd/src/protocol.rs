use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub id: String,
    pub version: String,
}

#[derive(Deserialize)]
pub enum Request {
    CreateAppInstance {
        label: String,
        manifest: Manifest
    },
    AppInstanceUuid(String),
    LocalPeerId,
    Peers,
    MessageInvite {
        peer: String,
        app_instance_uuid: String,
    },
    MessageSend {
        peer: String,
        app_instance_uuid: String,
        from_app_instance_uuid: String,
        message: Vec<u8>,
    },
    ReadMessage(String),
    NextMessage(String),
    // Invites,
}

#[derive(Serialize, Debug)]
pub enum Response {
    Success,
    Error(String),
    CreateAppInstance(String),
    AppInstanceUuid(Option<String>),
    LocalPeerId(String),
    Peers(Vec<String>),
    Message(Option<Message>),
    // Invites {
    //     peer: String,
    //     app_instance_uuid: String,
    // },
}

#[derive(Serialize, Debug)]
pub struct Message {
    pub peer: String,
    pub uuid: String,
    pub message: Vec<u8>,
}
