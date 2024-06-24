use libp2p::request_response::{InboundRequestId, OutboundRequestId, ResponseChannel};
use tokio::net::UnixStream;
use tokio::{signal, net::UnixListener, net::unix::SocketAddr, sync::mpsc};
use libp2p::{mdns, swarm::SwarmEvent, futures::stream::StreamExt, core::ConnectedPoint, Multiaddr, PeerId, request_response};
use std::collections::{HashSet, HashMap};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::swarm::{self, Swarm, MutinyBehaviourEvent};
use crate::config::Config;
use crate::protocol::{Request, Response, Manifest, Message};
use crate::client::{create_client, ClientRequest};
use crate::store::Store;

pub struct Server {
    swarm: Swarm,
    listener: UnixListener,
    client_request_receiver: mpsc::Receiver<ClientRequest>,
    client_request_sender: mpsc::Sender<ClientRequest>,
    peers: HashSet<(PeerId, Multiaddr)>,
    peer_id: libp2p::PeerId,
    delivery_attempts: HashMap<OutboundRequestId, i64>,
    store: Store,
}

impl Server {
    pub async fn start(config: Config) -> Result<(), Box<dyn Error>> {
        let pubkey = &config.keypair.public();
        let (tx, rx) = mpsc::channel(100);
        let mut server = Self {
            listener: UnixListener::bind(config.socket_path.as_path())?,
            swarm: swarm::start(config.keypair).await?,
            client_request_receiver: rx,
            client_request_sender: tx,
            peers: HashSet::new(),
            peer_id: libp2p::identity::PeerId::from_public_key(pubkey),
            delivery_attempts: HashMap::new(),
            store: Store::new(config.db_connection),
        };
        println!("  Local peer ID: {}", server.peer_id);
        {
            let tx = server.store.transaction()?;
            tx.migrate()?;
            tx.commit()?;
        }
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

    fn swarm_message(&mut self, peer_id: libp2p::PeerId, message: swarm::Message) -> Result<(), Box<dyn Error>> {
        match message {
            swarm::Message::Request {request_id, request, channel} => {
                self.swarm_request(peer_id, request_id, request, channel)
            },
            swarm::Message::Response {request_id, response} => {
                self.swarm_response(peer_id, request_id, response)
            },
        }
    }

    fn swarm_request(
        &mut self,
        peer: libp2p::PeerId,
        _request_id: InboundRequestId,
        request: swarm::Request,
        channel: ResponseChannel<swarm::Response>,
    ) -> Result<(), Box<dyn Error>> {
        // Can't store u64 timestamp directly in sqlite, would have to store as blob
        let received: i64 = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().try_into()?;
        let tx = self.store.transaction()?;
        let peer_id = tx.get_or_put_peer(&peer.to_base58())?;
        match request {
            swarm::Request::Invite {
                app_instance_uuid,
                app_id,
                app_version
            } => {
                let app = tx.get_or_put_app(&app_id)?;
                let version = tx.get_or_put_app_version(app, &app_version)?;
                let instance = tx.get_or_put_app_instance(peer_id, version, &app_instance_uuid)?;
                tx.get_or_put_message_invite(received, instance)?;
            },
            swarm::Request::Message {
                from_app_instance_uuid,
                to_app_instance_uuid,
                message,
            } => {
                let local_peer_id = tx.get_peer(&self.peer_id.to_base58())?.ok_or("Cannot find local peer ID in database")?;
                let from = tx.get_app_instance(peer_id, &from_app_instance_uuid)?.ok_or("Cannot find 'from' app instance in database")?;
                let to = tx.get_app_instance(local_peer_id, &to_app_instance_uuid)?.ok_or("Cannot find 'to' app instance in database")?;
                let message_id = tx.get_or_put_message_data(&message)?;
                tx.put_message_inbox(received, from, to, message_id)?;
            },
        }
        let _ = self.swarm.behaviour_mut().request_response.send_response(
            channel,
            swarm::Response::Acknowledge,
        );
        Ok(tx.commit()?)
    }

    fn swarm_response(
        &mut self,
        _peer: libp2p::PeerId,
        request_id: OutboundRequestId,
        response: swarm::Response,
    ) -> Result<(), Box<dyn Error>> {
        let tx = self.store.transaction()?;
        match response {
            swarm::Response::Acknowledge => {
                if let Some(outbox_id) = self.delivery_attempts.remove(&request_id) {
                    tx.delete_message_outbox(outbox_id)?;
                    // TODO: attempt to deliver next message for peer
                }
            }
        }
        Ok(tx.commit()?)
    }

    async fn swarm_event(&mut self, event: SwarmEvent<MutinyBehaviourEvent>) -> Result<(), Box<dyn Error>> {
        match event {
            SwarmEvent::Behaviour(swarm::MutinyBehaviourEvent::Mdns(ev)) => match ev {
                mdns::Event::Discovered(list) => {
                    for (peer_id, multiaddr) in list {
                        println!("mDNS discovered a new peer: {peer_id}");
                        self.peers.insert((peer_id, multiaddr));
                    }
                },
                mdns::Event::Expired(list) => {
                    for (peer_id, multiaddr) in list {
                        println!("mDNS discover peer has expired: {peer_id}");
                        self.peers.remove(&(peer_id, multiaddr));
                    }
                },
            },
            SwarmEvent::Behaviour(swarm::MutinyBehaviourEvent::RequestResponse(ev)) => match ev {
                request_response::Event::Message {peer, message} => {
                    self.swarm_message(peer, message)?;
                },
                _ => {},
                // request_response::Event::OutboundFailure {peer, request_id, error} => {
                // },
                // request_response::Event::InboundFailure {peer, request_id, error} => {
                // },
                // request_response::Event::ResponseSent {peer, request_id} => {
                // },
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

    async fn client_request(&mut self, request: ClientRequest) -> Result<(), Box<dyn Error>> {
        let response = match self.handle_request(request.request).await {
            Ok(response) => response,
            Err(err) => Response::Error {message: format!("{}", err)},
        };
        // ignore response failures, it means the client is gone
        let _ = request.response.send(response);
        Ok(())
    }

    fn create_app_instance(&mut self, label: &str, manifest: &Manifest) -> Result<String, Box<dyn Error>> {
        let tx = self.store.transaction()?;
        let uuid = Store::generate_app_instance_uuid();
        let app_id = tx.get_or_put_app(&manifest.id)?;
        let app_version_id = tx.get_or_put_app_version(app_id, &manifest.version)?;
        let peer_id = tx.get_or_put_peer(&self.peer_id.to_base58())?;
        let app_instance_id = tx.get_or_put_app_instance(
            peer_id,
            app_version_id,
            &uuid,
        )?;
        tx.put_app_instance_label(app_instance_id, label)?;
        tx.commit()?;
        Ok(uuid)
    }

    fn get_app_instance_uuid(&mut self, label: &str) -> Result<Option<String>, Box<dyn Error>> {
        let tx = self.store.transaction()?;
        if let Some(app_instance_id) = tx.get_app_instance_by_label(label)? {
            return Ok(tx.get_app_instance_uuid(app_instance_id)?)
        }
        Ok(None)
    }

    fn send_message_invite(&mut self, to_peer: &str, uuid: String) -> Result<(), Box<dyn Error>> {
        let peer: PeerId = to_peer.parse()?;
        let tx = self.store.transaction()?;
        let (app_id, app_version) = tx.get_app_id_and_version(&self.peer_id.to_base58(), &uuid)?.ok_or("Cannot find app id and version in database")?;
        self.swarm.behaviour_mut().request_response.send_request(&peer, swarm::Request::Invite {
            app_instance_uuid: uuid,
            app_id,
            app_version,
        });
        Ok(())
    }

    fn send_message(&mut self, to_peer: String, to_uuid: String, from_uuid: String, message: Vec<u8>) -> Result<(), Box<dyn Error>> {
        let tx = self.store.transaction()?;
        let queued: i64 = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().try_into()?;
        let message_id = tx.get_or_put_message_data(&message)?;
        let from_peer_id = tx.get_or_put_peer(&self.peer_id.to_base58())?;
        let to_peer_id = tx.get_or_put_peer(&to_peer)?;
        let from = tx.get_app_instance(from_peer_id, &from_uuid)?.ok_or("Cannot find 'from' app instance in database")?;
        let to = tx.get_app_instance(to_peer_id, &to_uuid)?.ok_or("Cannot find 'to' app instance in database")?;
        tx.put_message_outbox(queued, from, to, message_id)?;
        let peer: PeerId = to_peer.parse()?;
        self.swarm.behaviour_mut().request_response.send_request(&peer, swarm::Request::Message {
            to_app_instance_uuid: to_uuid,
            from_app_instance_uuid: from_uuid,
            message,
        });
        Ok(tx.commit()?)
    }

    fn read_message(&mut self, uuid: String) -> Result<Option<Message>, Box<dyn Error>> {
        let tx = self.store.transaction()?;
        let local_peer_id = tx.get_peer(&self.peer_id.to_base58())?.ok_or("Cannot find local peer ID in database")?;
        let app_instance_id = tx.get_app_instance(local_peer_id, &uuid)?.ok_or("Cannot find 'to' app instance in database")?;
        Ok(tx.read_message(app_instance_id)?)
    }

    fn next_message(&mut self, uuid: String) -> Result<(), Box<dyn Error>> {
        let tx = self.store.transaction()?;
        let local_peer_id = tx.get_peer(&self.peer_id.to_base58())?.ok_or("Cannot find local peer ID in database")?;
        let app_instance_id = tx.get_app_instance(local_peer_id, &uuid)?.ok_or("Cannot find 'to' app instance in database")?;
        Ok(tx.next_message(app_instance_id)?)
    }

    async fn handle_request(&mut self, request: Request) -> Result<Response, Box<dyn Error>> {
        match request {
            Request::CreateAppInstance {label, manifest} => {
                match self.create_app_instance(&label, &manifest) {
                    Ok(uuid) => Ok(Response::CreateAppInstance {uuid}),
                    Err(err) => Ok(Response::Error {message: format!("{}", err)}),
                }
            },
            Request::AppInstanceUuid {label} => {
                match self.get_app_instance_uuid(&label) {
                    Ok(uuid) => Ok(Response::AppInstanceUuid {uuid}),
                    Err(err) => Ok(Response::Error {message: format!("{}", err)}),
                }
            },
            Request::LocalPeerId => Ok(Response::LocalPeerId {
                peer_id: self.swarm.local_peer_id().to_base58()
            }),
            Request::Peers => {
                let mut peers: Vec<String> = Vec::new();
                for (id, _addr) in self.peers.iter() {
                    peers.push(id.to_base58());
                }
                Ok(Response::Peers {peers})
            },
            Request::MessageInvites => {
                let tx = self.store.transaction()?;
                Ok(Response::MessageInvites {
                    invites: tx.list_message_invites()?,
                })
            },
            Request::MessageInvite {peer, app_instance_uuid} => {
                self.send_message_invite(&peer, app_instance_uuid)?;
                Ok(Response::Success)
            },
            Request::MessageSend {peer, app_instance_uuid, from_app_instance_uuid, message} => {
                self.send_message(peer, app_instance_uuid, from_app_instance_uuid, message)?;
                Ok(Response::Success)
            },
            Request::ReadMessage {app_instance_uuid} => {
                let message = self.read_message(app_instance_uuid)?;
                Ok(Response::Message {message})
            },
            Request::NextMessage {app_instance_uuid} => {
                self.next_message(app_instance_uuid)?;
                Ok(Response::Success)
            }
        }
    }
}
