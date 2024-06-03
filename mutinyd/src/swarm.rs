use libp2p::identity::Keypair;
use std::time::Duration;
use std::error::Error;

pub async fn start(keypair: Keypair) -> Result<
    libp2p::swarm::Swarm<libp2p::mdns::tokio::Behaviour>,
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
            // Find peers on local network using multicast DNS
            let mdns = libp2p::mdns::tokio::Behaviour::new(
                libp2p::mdns::Config::default(), key.public().to_peer_id()
            )?;
            Ok(mdns)
        })?
        .with_swarm_config(
            |c| c.with_idle_connection_timeout(Duration::from_secs(60))
        )
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    Ok(swarm)
}

