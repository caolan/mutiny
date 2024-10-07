use libp2p::request_response::{InboundRequestId, OutboundRequestId, ResponseChannel};
use tokio::net::UnixStream;
use tokio::{signal, net::UnixListener, net::unix::SocketAddr, sync::mpsc};
use libp2p::{mdns, swarm::SwarmEvent, futures::stream::StreamExt, core::ConnectedPoint, Multiaddr, PeerId, request_response};
use std::collections::{HashSet, HashMap};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::swarm::{self, Swarm, MutinyBehaviourEvent};
use crate::config::Config;
use crate::protocol::{RequestBody, ResponseBody, Message};
use crate::client::{create_client, ClientRequest};
use crate::store::Store;

pub struct Server {
    swarm: Swarm,
    listener: UnixListener,
    peer_subscribers: HashMap<usize, mpsc::Sender<ResponseBody>>,
    announce_subscribers: HashMap<usize, mpsc::Sender<ResponseBody>>,
    inbox_subscribers: HashMap<i64, HashMap<usize, mpsc::Sender<ResponseBody>>>,
    client_request_receiver: mpsc::Receiver<ClientRequest>,
    client_request_sender: mpsc::Sender<ClientRequest>,
    peers: HashMap<PeerId, HashSet<Multiaddr>>,
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
            peer_subscribers: HashMap::new(),
            announce_subscribers: HashMap::new(),
            inbox_subscribers: HashMap::new(),
            swarm: swarm::start(config.keypair).await?,
            client_request_receiver: rx,
            client_request_sender: tx,
            peers: HashMap::new(),
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
                    self.client_request(client_request.unwrap()).await;
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

    async fn swarm_message(&mut self, peer_id: libp2p::PeerId, message: swarm::Message) -> Result<(), Box<dyn Error>> {
        match message {
            swarm::Message::Request {request_id, request, channel} => {
                self.swarm_request(peer_id, request_id, request, channel).await
            },
            swarm::Message::Response {request_id, response} => {
                self.swarm_response(peer_id, request_id, response)
            },
        }
    }

    async fn swarm_request(
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
            swarm::Request::Announce { app_uuid, data } => {
                let app = tx.get_or_put_app(peer_id, &app_uuid)?;
                tx.set_app_announcement(app, received, &data)?;
                tx.commit()?;
                self.announce_subscribers_send(ResponseBody::AppAnnouncement {
                    peer: peer.to_base58(),
                    app_uuid,
                    data,
                }).await;
            },
            swarm::Request::Message {
                from_app_uuid,
                to_app_uuid,
                message,
            } => {
                let local_peer_id = tx.get_peer(&self.peer_id.to_base58())?.ok_or("Cannot find local peer ID in database")?;
                let from = tx.get_app(peer_id, &from_app_uuid)?.ok_or("Cannot find 'from' app in database")?;
                let to = tx.get_app(local_peer_id, &to_app_uuid)?.ok_or("Cannot find 'to' app in database")?;
                let message_id = tx.get_or_put_message_data(&message)?;
                let id = tx.put_message_inbox(received, from, to, message_id)?;
                tx.commit()?;
                self.inbox_subscribers_send(to, ResponseBody::Message ( Message {
                    id: id.try_into()?,
                    peer: peer.to_base58(),
                    uuid: from_app_uuid,
                    message,
                })).await;
            },
        }
        let _ = self.swarm.behaviour_mut().request_response.send_response(
            channel,
            swarm::Response::Acknowledge,
        );
        Ok(())
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

    async fn peer_subscribers_send(&mut self, message: ResponseBody) -> () {
        let mut to_remove: Vec<usize> = vec![];
        for (request_id, sender) in &self.peer_subscribers {
            if let Err(err) = sender.send(message.clone()).await {
                eprintln!("Error sending message to peer event subscriber: {}", err);
                // Remove this subscriber
                to_remove.push(*request_id);
            }
        }
        for request_id in to_remove {
            self.peer_subscribers.remove(&request_id);
        }
    }

    async fn announce_subscribers_send(&mut self, message: ResponseBody) -> () {
        let mut to_remove: Vec<usize> = vec![];
        for (request_id, sender) in &self.announce_subscribers {
            if let Err(err) = sender.send(message.clone()).await {
                eprintln!("Error sending message to announce event subscriber: {}", err);
                // Remove this subscriber
                to_remove.push(*request_id);
            }
        }
        for request_id in to_remove {
            self.announce_subscribers.remove(&request_id);
        }
    }

    async fn inbox_subscribers_send(&mut self, app_id: i64, message: ResponseBody) -> () {
        let mut to_remove: Vec<usize> = vec![] ;
        if let Some(subscribers) = self.inbox_subscribers.get(&app_id) {
            for (request_id, sender) in subscribers {
                if let Err(err) = sender.send(message.clone()).await {
                    eprintln!("Error sending message to announce event subscriber: {}", err);
                    // Remove this subscriber
                    to_remove.push(*request_id);
                }
            }
            for request_id in to_remove {
                self.announce_subscribers.remove(&request_id);
            }
        }
    }

    async fn add_peer_address(&mut self, peer_id: PeerId, addr: Multiaddr) {
        match self.peers.entry(peer_id) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                entry.into_mut().insert(addr);
            },
            std::collections::hash_map::Entry::Vacant(entry) => {
                let mut addrs = HashSet::new();
                addrs.insert(addr);
                entry.insert(addrs);
                self.peer_subscribers_send(ResponseBody::PeerDiscovered {
                    peer_id: peer_id.to_base58(),
                }).await;
            },
        }
    }

    async fn remove_peer_address(&mut self, peer_id: PeerId, addr: Multiaddr) {
        let mut expired = false;
        if let Some(addrs) = self.peers.get_mut(&peer_id) {
            addrs.remove(&addr);
            expired = addrs.len() == 0;
        }
        if expired {
            self.peers.remove(&peer_id);
            self.peer_subscribers_send(ResponseBody::PeerExpired {
                peer_id: peer_id.to_base58(),
            }).await;
        }
    }

    async fn swarm_event(&mut self, event: SwarmEvent<MutinyBehaviourEvent>) -> Result<(), Box<dyn Error>> {
        match event {
            SwarmEvent::Behaviour(swarm::MutinyBehaviourEvent::Mdns(ev)) => match ev {
                mdns::Event::Discovered(list) => {
                    for (peer_id, addr) in list {
                        println!("mDNS discovered a new peer: {peer_id}");
                        self.add_peer_address(peer_id, addr).await
                    }
                },
                mdns::Event::Expired(list) => {
                    for (peer_id, addr) in list {
                        println!("mDNS discover peer has expired: {peer_id}");
                        self.remove_peer_address(peer_id, addr).await;
                    }
                },
            },
            SwarmEvent::Behaviour(swarm::MutinyBehaviourEvent::RequestResponse(ev)) => match ev {
                request_response::Event::Message {peer, message} => {
                    self.swarm_message(peer, message).await?;
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
            SwarmEvent::Dialing {..} => {
                println!("Dialing...");
            },
            SwarmEvent::OutgoingConnectionError { error, .. } => {
                println!("Outgoing connection error: {error}");
            },
            SwarmEvent::Behaviour(swarm::MutinyBehaviourEvent::Identify(ev)) => match ev {
                // Identification information of the local node has been sent to a peer in response to an identification request.
                libp2p::identify::Event::Sent { peer_id, .. } => {
                    println!("Sent identify info to {peer_id:?}")
                },
                // Identification information has been received from a peer.
                libp2p::identify::Event::Received { info, .. } => {
                    println!("Received identify info {info:?}");
                    let peer_id = libp2p::identity::PeerId::from_public_key(&info.public_key);
                    for addr in info.listen_addrs {
                        self.add_peer_address(peer_id, addr).await
                    }
                },
                // Identification information of the local node has been actively pushed to a peer.
                libp2p::identify::Event::Pushed { peer_id, .. } => {
                    println!("Pushed identify info to {peer_id:?}")
                },
                // Error while attempting to identify the remote.
                libp2p::identify::Event::Error { peer_id, .. } => {
                    println!("Error identifying remote {peer_id:?}")
                },
            },
            _ => {}
        };
        Ok(())
    }

    async fn client_request(&mut self, request: ClientRequest) {
        let sender = request.response.clone();
        if let Err(err) = self.handle_request(request).await {
            // ignore response failures, the client might be gone
            let _ = sender.send(ResponseBody::Error {
                message: format!("{}", err),
            }).await;
        }
    }

    fn create_app(&mut self, label: &str) -> Result<String, Box<dyn Error>> {
        let tx = self.store.transaction()?;
        let uuid = Store::generate_app_uuid();
        let peer_id = tx.get_or_put_peer(&self.peer_id.to_base58())?;
        let app_id = tx.get_or_put_app(peer_id, &uuid)?;
        tx.put_app_label(app_id, label)?;
        tx.commit()?;
        Ok(uuid)
    }

    fn get_app_uuid(&mut self, label: &str) -> Result<Option<String>, Box<dyn Error>> {
        let tx = self.store.transaction()?;
        if let Some(app_id) = tx.get_app_by_label(label)? {
            return Ok(tx.get_app_uuid(app_id)?)
        }
        Ok(None)
    }

    fn send_announce(&mut self, to_peer: &str, app_uuid: String, data: serde_json::Value) -> Result<(), Box<dyn Error>> {
        let peer: PeerId = to_peer.parse()?;
        self.swarm.behaviour_mut().request_response.send_request(&peer, swarm::Request::Announce {
            app_uuid,
            data,
        });
        Ok(())
    }

    fn send_message(&mut self, to_peer: String, to_uuid: String, from_uuid: String, message: Vec<u8>) -> Result<(), Box<dyn Error>> {
        let tx = self.store.transaction()?;
        let queued: i64 = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().try_into()?;
        let message_id = tx.get_or_put_message_data(&message)?;
        let from_peer_id = tx.get_or_put_peer(&self.peer_id.to_base58())?;
        let to_peer_id = tx.get_or_put_peer(&to_peer)?;
        let from = tx.get_app(from_peer_id, &from_uuid)?.ok_or("Cannot find 'from' app instance in database")?;
        let to = tx.get_app(to_peer_id, &to_uuid)?.ok_or("Cannot find 'to' app instance in database")?;
        let outbox_id = tx.put_message_outbox(queued, from, to, message_id)?;
        let peer: PeerId = to_peer.parse()?;
        let request_id = self.swarm.behaviour_mut().request_response.send_request(&peer, swarm::Request::Message {
            to_app_uuid: to_uuid,
            from_app_uuid: from_uuid,
            message,
        });
        self.delivery_attempts.insert(request_id, outbox_id);
        Ok(tx.commit()?)
    }

    // fn read_message(&mut self, uuid: String) -> Result<Option<Message>, Box<dyn Error>> {
    //     let tx = self.store.transaction()?;
    //     let local_peer_id = tx.get_peer(&self.peer_id.to_base58())?.ok_or("Cannot find local peer ID in database")?;
    //     let app_id = tx.get_app(local_peer_id, &uuid)?.ok_or("Cannot find 'to' app instance in database")?;
    //     Ok(tx.read_message(app_id)?)
    // }

    // fn next_message(&mut self, uuid: String) -> Result<(), Box<dyn Error>> {
    //     let tx = self.store.transaction()?;
    //     let local_peer_id = tx.get_peer(&self.peer_id.to_base58())?.ok_or("Cannot find local peer ID in database")?;
    //     let app_id = tx.get_app(local_peer_id, &uuid)?.ok_or("Cannot find 'to' app instance in database")?;
    //     tx.next_message(app_id)?;
    //     Ok(tx.commit()?)
    // }

    async fn handle_request(&mut self, request: ClientRequest) -> Result<(), Box<dyn Error>> {
        match request.request.body {
            RequestBody::CreateAppInstance {label} => {
                let uuid = self.create_app(&label)?;
                let _ = request.response.send(ResponseBody::CreateAppInstance {uuid}).await;
            },
            RequestBody::AppInstanceUuid {label} => {
                let uuid = self.get_app_uuid(&label)?;
                let _ = request.response.send(ResponseBody::AppInstanceUuid {uuid}).await;
            },
            RequestBody::GetLastPort {app_uuid} => {
                let tx = self.store.transaction()?;
                let peer_id = tx.get_or_put_peer(&self.peer_id.to_base58())?;
                let app_id = tx.get_or_put_app(peer_id, &app_uuid)?;
                let port = tx.get_last_port(app_id)?;
                let _ = request.response.send(ResponseBody::GetLastPort {port}).await;
            },
            RequestBody::SetLastPort {app_uuid, port} => {
                let tx = self.store.transaction()?;
                let peer_id = tx.get_or_put_peer(&self.peer_id.to_base58())?;
                let app_id = tx.get_or_put_app(peer_id, &app_uuid)?;
                tx.set_last_port(app_id, port)?;
                tx.commit()?;
                let _ = request.response.send(ResponseBody::Success).await;
            },
            RequestBody::LocalPeerId => {
                let _ = request.response.send(ResponseBody::LocalPeerId {
                    peer_id: self.swarm.local_peer_id().to_base58()
                }).await;
            },
            RequestBody::Peers => {
                let mut peers: Vec<String> = Vec::new();
                for (id, _addrs) in self.peers.iter() {
                    peers.push(id.to_base58());
                }
                let _ = request.response.send(ResponseBody::Peers {peers}).await;
            },
            RequestBody::DialAddress {address} => {
                let remote = address.parse::<Multiaddr>()?;
                self.swarm.dial(remote)?;
                // TODO: use connection id to track success or not of
                // Indentify by checking for outgoing connection error,
                // connection established, and identify sent/received events.
                // TODO: use identify::Behaviour::push() to actively push
                // identity info to peer after this dial request.
                let _ = request.response.send(ResponseBody::Success).await;
            },
            RequestBody::AppAnnouncements => {
                let tx = self.store.transaction()?;
                let _ = request.response.send(ResponseBody::AppAnnouncements {
                    announcements: tx.list_app_announcements()?,
                }).await;
            },
            RequestBody::Announce {peer, app_uuid, data} => {
                // If the announce is directed at local peer just write directly to database
                if peer.eq(&self.peer_id.to_base58()) {
                    // Can't store u64 timestamp directly in sqlite, would have to store as blob
                    let received: i64 = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().try_into()?;
                    let tx = self.store.transaction()?;
                    let peer_id = tx.get_or_put_peer(&peer)?;
                    let app = tx.get_or_put_app(peer_id, &app_uuid)?;
                    tx.set_app_announcement(app, received, &data)?;
                    tx.commit()?;
                    self.announce_subscribers_send(ResponseBody::AppAnnouncement {
                        peer,
                        app_uuid,
                        data,
                    }).await;
                } else {
                    // Else if announce is directed at another peer, send libp2p request
                    self.send_announce(&peer, app_uuid, data)?;
                }
                let _ = request.response.send(ResponseBody::Success).await;
            },
            RequestBody::SendMessage {peer, app_uuid, from_app_uuid, message} => {
                self.send_message(peer, app_uuid, from_app_uuid, message)?;
                let _ = request.response.send(ResponseBody::Success).await;
            },
            RequestBody::InboxMessages {app_uuid} => {
                let tx = self.store.transaction()?;
                let peer_id = tx.get_or_put_peer(&self.peer_id.to_base58())?;
                let app_id = tx.get_or_put_app(peer_id, &app_uuid)?;
                let _ = request.response.send(ResponseBody::InboxMessages {
                    messages: tx.list_app_inbox_messages(app_id)?,
                }).await;
            },
            RequestBody::DeleteInboxMessage {app_uuid, message_id} => {
                let tx = self.store.transaction()?;
                let peer_id = tx.get_or_put_peer(&self.peer_id.to_base58())?;
                let app_id = tx.get_or_put_app(peer_id, &app_uuid)?;
                tx.delete_inbox_message(app_id, message_id.try_into()?)?;
                let _ = request.response.send(ResponseBody::Success).await;
            },
            RequestBody::SubscribePeerEvents => {
                self.peer_subscribers.insert(request.request.id, request.response);
            },
            RequestBody::SubscribeAnnounceEvents => {
                self.announce_subscribers.insert(request.request.id, request.response);
            },
            RequestBody::SubscribeInboxEvents {app_uuid} => {
                let tx = self.store.transaction()?;
                let peer_id = tx.get_or_put_peer(&self.peer_id.to_base58())?;
                let app_id = tx.get_or_put_app(peer_id, &app_uuid)?;
                let subscribers = self.inbox_subscribers.entry(app_id).or_insert(HashMap::new());
                subscribers.insert(request.request.id, request.response);
            }
        }
        Ok(())
    }
}
