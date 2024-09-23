use libp2p::{identity::Keypair, mdns, request_response::{self, ProtocolSupport}, swarm::{NetworkBehaviour, StreamProtocol}};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::error::Error;

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    Announce {
        app_uuid: String,
        data: serde_json::Value,
    },
    Message {
        from_app_uuid: String,
        to_app_uuid: String,
        message: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Acknowledge,
}

// A custom network behaviour that combines Request/Response and MDNS.
#[derive(NetworkBehaviour)]
pub struct MutinyBehaviour {
    pub request_response: request_response::cbor::Behaviour<Request, Response>,
    pub mdns: mdns::tokio::Behaviour,
    pub kad: libp2p::kad::Behaviour<libp2p::kad::store::MemoryStore>,
}

pub type Swarm = libp2p::swarm::Swarm<MutinyBehaviour>;
pub type Message = request_response::Message<Request, Response>;

pub async fn start(keypair: Keypair) -> Result<
    libp2p::swarm::Swarm<MutinyBehaviour>,
    Box<dyn Error>,
> {
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_behaviour(|key| {
            let request_response = libp2p::request_response::cbor::Behaviour::<Request, Response>::new(
                [(StreamProtocol::new("/mutiny-request-response-protocol"), ProtocolSupport::Full)],
                libp2p::request_response::Config::default(),
            );
            // Find peers on local network using multicast DNS
            let mdns = libp2p::mdns::Behaviour::new(
                libp2p::mdns::Config::default(), key.public().to_peer_id()
            )?;
            // Create a Kademlia behaviour.
            let protocol_name = StreamProtocol::new("/mutiny/kad/1.0.0");
            let mut cfg = libp2p::kad::Config::new(protocol_name);
            cfg.set_query_timeout(Duration::from_secs(5 * 60));
            let store = libp2p::kad::store::MemoryStore::new(key.public().to_peer_id());
            let kad = libp2p::kad::Behaviour::with_config(key.public().to_peer_id(), store, cfg);
            Ok(MutinyBehaviour { request_response, mdns, kad })
        })?
        .with_swarm_config(
            |c| c.with_idle_connection_timeout(Duration::from_secs(60))
        )
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    Ok(swarm)
}

