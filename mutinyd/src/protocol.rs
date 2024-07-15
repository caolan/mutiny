use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag="type")]
pub enum Request {
    CreateAppInstance {
        label: String,
    },
    AppInstanceUuid {
        label: String,
    },
    LocalPeerId,
    Peers,
    MessageInvite {
        peer: String,
        app_uuid: String,
    },
    MessageSend {
        peer: String,
        app_uuid: String,
        from_app_uuid: String,
        #[serde(with = "serde_bytes")]
        message: Vec<u8>,
    },
    MessageRead {
        app_uuid: String,
    },
    MessageNext {
        app_uuid: String,
    },
    MessageInvites,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Message {
    pub peer: String,
    pub uuid: String,
    #[serde(with = "serde_bytes")]
    pub message: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MessageInvite {
    pub peer: String,
    pub app_uuid: String,
}
