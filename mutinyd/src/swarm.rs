use libp2p::{identity::Keypair, mdns, request_response::{self, ProtocolSupport}, swarm::{NetworkBehaviour, StreamProtocol}};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::error::Error;

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    NotifyAccept {
        peer: String,
        application_instance_uuid: String,
        application_id: String,
        application_version: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Success,
}

// A custom network behaviour that combines Request/Response and MDNS.
#[derive(NetworkBehaviour)]
pub struct MutinyBehaviour {
    request_response: request_response::cbor::Behaviour<Request, Response>,
    mdns: mdns::tokio::Behaviour,
}

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
            let mdns = libp2p::mdns::tokio::Behaviour::new(
                libp2p::mdns::Config::default(), key.public().to_peer_id()
            )?;
            Ok(MutinyBehaviour { request_response, mdns })
        })?
        .with_swarm_config(
            |c| c.with_idle_connection_timeout(Duration::from_secs(60))
        )
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    Ok(swarm)
}

