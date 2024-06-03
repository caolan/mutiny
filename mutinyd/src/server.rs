use tokio::net::UnixStream;
use tokio::{signal, net::UnixListener, net::unix::SocketAddr, sync::mpsc};
use libp2p::{mdns, swarm::SwarmEvent, futures::stream::StreamExt, core::ConnectedPoint};
use std::error::Error;

use crate::swarm;
use crate::config::Config;
use crate::protocol::{Request, Response};
use crate::client::{create_client, ClientRequest};

type Swarm = libp2p::swarm::Swarm<libp2p::mdns::tokio::Behaviour>;

pub struct Server {
    swarm: Swarm,
    listener: UnixListener,
    client_request_receiver: mpsc::Receiver<ClientRequest>,
    client_request_sender: mpsc::Sender<ClientRequest>,
}

impl Server {
    pub async fn start(config: Config) -> Result<(), Box<dyn Error>> {
        println!("  Local peer ID: {}", libp2p::identity::PeerId::from_public_key(
            &config.keypair.public(),
        ));
        let (tx, rx) = mpsc::channel(100);
        let mut server = Self {
            listener: UnixListener::bind(config.socket_path.as_path())?,
            swarm: swarm::start(config.keypair).await?,
            client_request_receiver: rx,
            client_request_sender: tx,
        };
        server.run().await;
        println!("Removing {:?}", config.socket_path);
        tokio::fs::remove_file(config.socket_path.as_path()).await?;
        Ok(())
    }

    async fn run(&mut self) -> () {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.swarm_event(event).await.unwrap();
                },
                connection = self.listener.accept() => {
                    self.spawn_client(connection.unwrap()).await;
                },
                client_request = self.client_request_receiver.recv() => {
                    self.client_request(client_request.unwrap()).await.unwrap();
                },
                _signal = signal::ctrl_c() => break,
            }
        }
    }

    async fn spawn_client(&self, connection: (UnixStream, SocketAddr)) -> () {
        let (stream, _addr) = connection;
        let client = create_client(stream, self.client_request_sender.clone());
        tokio::spawn(client.start());
    }

    async fn swarm_event(&self, event: SwarmEvent<mdns::Event>) -> Result<(), Box<dyn Error>> {
        match event {
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
        };
        Ok(())
    }

    async fn client_request(&self, request: ClientRequest) -> Result<(), Box<dyn Error>> {
        // ignore response failures, it means the client is gone
        let _ = request.response.send(self.handle_request(request.request).await?);
        Ok(())
    }

    async fn handle_request(&self, request: Request) -> Result<Response, Box<dyn Error>> {
        match request {
            Request::Ping => Ok(Response::Pong),
            Request::LocalPeerId => Ok(Response::LocalPeerId(self.swarm.local_peer_id().to_base58())),
        }
    }
}
