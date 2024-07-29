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
    SendMessage {
        peer: String,
        app_uuid: String,
        from_app_uuid: String,
        #[serde(with = "serde_bytes")]
        message: Vec<u8>,
    },
    InboxMessages {
        app_uuid: String,
    },
    DeleteInboxMessage {
        app_uuid: String,
        message_id: usize,
    },
    AppAnnouncements,
    SubscribePeerEvents,
    SubscribeAnnounceEvents,
    SubscribeInboxEvents {
        app_uuid: String,
    },
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
    Message (Message),
    InboxMessages {
        messages: Vec<Message>
    },
    AppAnnouncements {
        announcements: Vec<AppAnnouncement>
    },
    AppAnnouncement {
        peer: String,
        app_uuid: String,
        data: serde_json::Value,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(tag="type")]
pub struct Message {
    pub id: usize,
    pub peer: String,
    pub uuid: String,
    #[serde(with = "serde_bytes")]
    pub message: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(tag="type")]
pub struct AppAnnouncement {
    pub peer: String,
    pub app_uuid: String,
    pub data: serde_json::Value,
}
