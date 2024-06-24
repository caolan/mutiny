use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub id: String,
    pub version: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag="type")]
pub enum Request {
    CreateAppInstance {
        label: String,
        manifest: Manifest
    },
    AppInstanceUuid {
        label: String,
    },
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
    ReadMessage {
        app_instance_uuid: String,
    },
    NextMessage {
        app_instance_uuid: String,
    },
    MessageInvites,
}

#[derive(Serialize, Debug)]
#[serde(tag="type")]
pub enum Response {
    Success,
    Error {
        message: String,
    },
    CreateAppInstance {
        uuid: String
    },
    AppInstanceUuid {
        uuid: Option<String>,
    },
    LocalPeerId {
        peer_id: String
    },
    Peers {
        peers: Vec<String>,
    },
    Message {
        message: Option<Message>,
    },
    MessageInvites {
        invites: Vec<MessageInvite>
    },
}

#[derive(Serialize, Debug)]
pub struct Message {
    pub peer: String,
    pub uuid: String,
    pub message: Vec<u8>,
}

#[derive(Serialize, Debug)]
pub struct MessageInvite {
    pub peer: String,
    pub app_instance_uuid: String,
    pub manifest_id: String,
    pub manifest_version: String,
}
