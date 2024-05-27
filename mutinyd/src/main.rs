use tokio::{signal, io::BufReader, io::AsyncReadExt, io::AsyncWriteExt, net::UnixListener};
use libp2p::{mdns, swarm::SwarmEvent, futures::stream::StreamExt, core::ConnectedPoint};
use std::os::unix::fs::PermissionsExt;
use serde::Serialize;
use rmp_serde::Serializer;
use std::path::{PathBuf, Path};
use std::error::Error;
use libp2p::identity::Keypair;
use std::time::Duration;
use std::io::Write;
use std::fs;

mod protocol;
mod dirs;

use protocol::{Request, Response};

fn get_socket_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(dirs::open_app_runtime_dir()?.join("mutinyd.socket"))
}

fn get_keypair_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(dirs::open_app_data_dir()?.join("identity.key"))
}

async fn handle_request(_request: Request) -> Response {
    return Response::Pong {};
}

async fn listen(socket_path: &Path) {
    let listener = UnixListener::bind(socket_path).unwrap();
    println!("Listening at {:?}", socket_path);
    loop {
        match listener.accept().await {
            Ok((mut stream, _addr)) => {
                let (read, mut write) = stream.split();
                let mut reader = BufReader::new(read);
                loop {
                    match reader.read_u32().await {
                        Err(err) => {
                            if err.kind() == std::io::ErrorKind::UnexpectedEof {
                                // Client disconnected
                                break
                            }
                            panic!("{:?}", err);
                        },
                        Ok(length) => {
                            let mut buf = vec![0; length as usize];
                            reader.read_exact(&mut buf).await.unwrap();
                            let request: Request = rmp_serde::from_slice(&buf).unwrap();
                            let response = handle_request(request).await;
                            // The rmp_serde::Serializer is not async and can not write
                            // directly to an AsyncWrite, write to a buffer first.
                            let mut serialized = Vec::<u8>::new();
                            response.serialize(&mut Serializer::new(&mut serialized)).unwrap();
                            let len = u32::try_from(serialized.len()).unwrap();
                            write.write_all(&len.to_be_bytes()).await.unwrap();
                            write.write_all(&serialized).await.unwrap();
                            write.flush().await.unwrap();
                        },
                    }
                }
            }
            Err(_e) => { /* connection failed */ }
        }
    }
}

async fn swarm(keypair: Keypair) -> Result<(), Box<dyn Error>> {
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

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::Behaviour(mdns::Event::Discovered(list)) => {
                for (peer_id, _multiaddr) in list {
                    println!("mDNS discovered a new peer: {peer_id}");
                }
            },
            SwarmEvent::Behaviour(mdns::Event::Expired(list)) => {
                for (peer_id, _multiaddr) in list {
                    println!("mDNS discover peer has expired: {peer_id}");
                }
            },
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("New listener: {address}");
            },
            SwarmEvent::ExpiredListenAddr { address, .. } => {
                println!("Expired listener: {address}");
            },
            SwarmEvent::ConnectionEstablished { endpoint, .. } => {
                if let ConnectedPoint::Dialer { address, .. } = endpoint {
                    println!("Connection established: {address}");
                }
            },
            SwarmEvent::ConnectionClosed { endpoint, .. } => {
                if let ConnectedPoint::Dialer { address, .. } = endpoint {
                    println!("Connection closed: {address}");
                }
            },
            _ => {}
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let keypair_path = get_keypair_path()?;
    println!("Reading identity {:?}", keypair_path);
    let keypair = if keypair_path.exists() {
        let encoded = fs::read(keypair_path)?;
        Keypair::from_protobuf_encoding(&encoded)?
    } else {
        println!("  Generating new keypair");
        let k = Keypair::generate_ed25519();
        let encoded = k.to_protobuf_encoding()?;
        let mut f = fs::File::create(keypair_path)?;
        f.set_permissions(fs::Permissions::from_mode(0o600))?;
        f.write_all(&encoded)?;
        k
    };
    println!("  Local peer ID: {}", libp2p::identity::PeerId::from_public_key(
        &keypair.public(),
    ));

    let socket_path = get_socket_path().expect("Open unix socket");
    let socket_path2 = socket_path.clone();
    // Cleans up unix socket after ctrl-c
    // TODO: clean up if socket was created by this process but a subsequent
    // error occurs
    tokio::select! {
        r = tokio::spawn(async move {
            listen(socket_path2.as_path()).await
        }) => r.unwrap(),
        _ = tokio::spawn(async move {
            swarm(keypair).await.unwrap()
        }) => {},
        r = signal::ctrl_c() => r.unwrap(),
    };
    println!("Removing {:?}", socket_path);
    tokio::fs::remove_file(socket_path).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    run().await.unwrap()
}
