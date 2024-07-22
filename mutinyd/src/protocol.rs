use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Request {
    pub id: usize,
    pub body: RequestBody,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(tag="type")]
pub enum RequestBody {
    CreateAppInstance {
        label: String,
    },
    AppInstanceUuid {
        label: String,
    },
    LocalPeerId,
    Peers,
    Announce {
        peer: String,
        app_uuid: String,
        data: serde_json::Value,
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
    AppAnnouncements,
    SubscribePeerEvents,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Response {
    pub request_id: usize,
    pub body: ResponseBody,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(tag="type")]
pub enum ResponseBody {
    Success,
    Error {
        message: String,
    },
    PeerDiscovered {
        peer_id: String
    },
    PeerExpired {
        peer_id: String
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
    AppAnnouncements {
        announcements: Vec<AppAnnouncement>
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Message {
    pub peer: String,
    pub uuid: String,
    #[serde(with = "serde_bytes")]
    pub message: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AppAnnouncement {
    pub peer: String,
    pub app_uuid: String,
    pub data: serde_json::Value,
}
