use libp2p::swarm::NetworkBehaviour;
use tokio::net::UnixStream;
use tokio::{signal, net::UnixListener, net::unix::SocketAddr, sync::mpsc};
use libp2p::{mdns, swarm::SwarmEvent, futures::stream::StreamExt, core::ConnectedPoint, Multiaddr, PeerId, request_response};
use std::collections::HashSet;
use std::error::Error;
use rusqlite::OptionalExtension;
use rusqlite;
use uuid::Uuid;

use crate::swarm::{self, MutinyBehaviourEvent};
use crate::config::Config;
use crate::protocol::{Request, Response, Peer, Manifest};
use crate::client::{create_client, ClientRequest};

type Swarm = libp2p::swarm::Swarm<libp2p::mdns::tokio::Behaviour>;

pub struct Server {
    swarm: Swarm,
    listener: UnixListener,
    client_request_receiver: mpsc::Receiver<ClientRequest>,
    client_request_sender: mpsc::Sender<ClientRequest>,
    peers: HashSet<(PeerId, Multiaddr)>,
    db: rusqlite::Connection,
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
            peers: HashSet::new(),
            db: config.db_connection,
        };
        server.migrate()?;
        server.run().await;
        println!("Removing {:?}", config.socket_path);
        tokio::fs::remove_file(config.socket_path.as_path()).await?;
        Ok(())
    }

    fn version(&self) -> rusqlite::Result<i64> {
        let mut stmt = self.db.prepare_cached("SELECT user_version FROM pragma_user_version")?;
        return stmt.query_row([], |row| row.get::<_, i64>(0))
    }

    // fn set_version(&self, version: i64) -> rusqlite::Result<()> {
    //     self.db.pragma_update(None, "user_version", version)
    // }

    fn migrate(&self) -> rusqlite::Result<()> {
        while let version = self.version()? {
            match version {
                0 => {
                    println!("Migrating database to version 1");
                    self.db.execute_batch(
                        "BEGIN;
                         CREATE TABLE application (
                             id INTEGER PRIMARY KEY,
                             manifest_id TEXT UNIQUE NOT NULL
                         );
                         CREATE TABLE application_version (
                             id INTEGER PRIMARY KEY,
                             application_id INTEGER REFERENCES application(id) NOT NULL,
                             manifest_version TEXT NOT NULL,
                             UNIQUE(application_id, manifest_version)
                         );
                         CREATE TABLE application_instance (
                             id INTEGER PRIMARY KEY,
                             uuid TEXT UNIQUE NOT NULL,
                             name TEXT UNIQUE NOT NULL,
                             application_version_id INTEGER REFERENCES application_version(id) NOT NULL
                         );
                         PRAGMA user_version = 1;
                         COMMIT;",
                    )?;
                },
                1 => {
                    println!("Migrating database to version 2");
                    self.db.execute_batch(
                        "BEGIN;
                         CREATE TABLE peer (
                             id INTEGER PRIMARY KEY,
                             addr TEXT UNIQUE NOT NULL
                         );
                         CREATE TABLE remote_application_instance (
                             peer_id INTEGER REFERENCES peer(id) NOT NULL,
                             uuid TEXT UNIQUE NOT NULL,
                             application_version_id INTEGER REFERENCES application_version(id) NOT NULL
                         );
                         CREATE TABLE accept (
                             peer_id INTEGER REFERENCES peer(id) NOT NULL,
                             application_instance_id INTEGER REFERENCES application_instance(id) NOT NULL
                         );
                         PRAGMA user_version = 2;
                         COMMIT;",
                    )?;
                }
            }
        }
        Ok(())
    }

    fn get_application_id(&self, manifest_id: &str) -> rusqlite::Result<Option<i64>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT id FROM application WHERE manifest_id = ?1"
        )?;
        stmt.query_row([manifest_id], |row| row.get::<_, i64>(0)).optional()
    }

    fn put_application(&self, manifest_id: &str) -> rusqlite::Result<i64> {
        let mut stmt = self.db.prepare_cached(
            "INSERT INTO application (manifest_id) VALUES (?1) RETURNING id"
        )?;
        stmt.query_row([manifest_id], |row| row.get::<_, i64>(0))
    }

    fn get_application_version_id(&self, application_id: i64, manifest_version: &str) -> rusqlite::Result<Option<i64>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT id FROM application_version WHERE application_id = ?1 AND manifest_version = ?2"
        )?;
        stmt.query_row(rusqlite::params![application_id, manifest_version], |row| row.get::<_, i64>(0)).optional()
    }

    fn put_application_version(&self, application_id: i64, manifest_version: &str) -> rusqlite::Result<i64> {
        let mut stmt = self.db.prepare_cached(
            "INSERT INTO application_version (application_id, manifest_version) VALUES (?1, ?2) RETURNING id"
        )?;
        stmt.query_row(rusqlite::params![application_id, manifest_version], |row| row.get::<_, i64>(0))
    }

    fn get_application_instance_uuid(&self, name: &str) -> rusqlite::Result<Option<String>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT uuid FROM application_instance WHERE name = ?1"
        )?;
        stmt.query_row([name], |row| row.get::<_, String>(0)).optional()
    }

    fn get_application_instance_id_from_uuid(&self, uuid: &str) -> rusqlite::Result<i64> {
        let mut stmt = self.db.prepare_cached(
            "SELECT id FROM application_instance WHERE uuid = ?1"
        )?;
        stmt.query_row([uuid], |row| row.get::<_, i64>(0))
    }

    fn get_peer_id(&self, addr: &str) -> rusqlite::Result<Option<i64>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT id FROM peer WHERE addr = ?1"
        )?;
        stmt.query_row([addr], |row| row.get::<_, i64>(0)).optional()
    }

    fn put_peer(&self, addr: &str) -> rusqlite::Result<i64> {
        let mut stmt = self.db.prepare_cached(
            "INSERT INTO peer (addr) VALUES (?1) RETURNING id"
        )?;
        stmt.query_row([addr], |row| row.get::<_, i64>(0))
    }

    fn create_application_instance(&self, name: &str, manifest: &Manifest) -> Result<String, Box<dyn Error>> {
        if name.is_empty() {
            return Err(Box::<dyn Error>::from("Application instance name cannot be empty"));
        }
        let application_id;
        if let Some(id) = self.get_application_id(&manifest.id)? {
            application_id = id;
        } else {
            application_id = self.put_application(&manifest.id)?;
        }
        let application_version_id;
        if let Some(id) = self.get_application_version_id(application_id, &manifest.version)? {
            application_version_id = id;
        } else {
            application_version_id = self.put_application_version(application_id, &manifest.version)?;
        }
        let mut stmt = self.db.prepare_cached(
            "INSERT INTO application_instance (uuid, name, application_version_id)
             VALUES (?1, ?2, ?3) RETURNING uuid"
        )?;
        let buffer = &mut Uuid::encode_buffer();
        let uuid: &str = Uuid::new_v4().hyphenated().encode_lower(buffer);
        Ok(stmt.query_row(
            rusqlite::params![uuid, name, application_version_id],
            |row| row.get::<_, String>(0),
        )?)
    }

    fn accept_messages(&self, from_addr: &str, to_uuid: &str) -> Result<(), Box<dyn Error>> {
        let application_instance_id = self.get_application_instance_id_from_uuid(to_uuid)?;
        let peer_id;
        if let Some(id) = self.get_peer_id(from_addr)? {
            peer_id = id;
        } else {
            peer_id = self.put_peer(from_addr)?;
        }
        let mut stmt = self.db.prepare_cached(
            "INSERT INTO accept (peer_id, application_instance_id)
             VALUES (?1, ?2)"
        )?;
        stmt.execute([peer_id, application_instance_id])?;
        Ok(())
    }

    // Lets remote peer know we're now accepting messages
    fn notify_accept_messages(&self, from_peer: String, to_uuid: String) -> Result<(), Box<dyn Error>> {
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
                    match message {
                        Request
                    }
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

    async fn client_request(&self, request: ClientRequest) -> Result<(), Box<dyn Error>> {
        // ignore response failures, it means the client is gone
        let _ = request.response.send(self.handle_request(request.request).await?);
        Ok(())
    }

    async fn handle_request(&self, request: Request) -> Result<Response, Box<dyn Error>> {
        match request {
            Request::Ping => Ok(Response::Pong),
            Request::CreateAppInstance {name, manifest} => {
                match self.create_application_instance(&name, &manifest) {
                    Ok(uuid) => Ok(Response::CreateAppInstance(uuid)),
                    Err(err) => Ok(Response::Error(format!("{}", err))),
                }
            },
            Request::AppInstanceUuid(name) => {
                match self.get_application_instance_uuid(&name) {
                    Ok(uuid) => Ok(Response::AppInstanceUuid(uuid)),
                    Err(err) => Ok(Response::Error(format!("{}", err))),
                }
            },
            Request::LocalPeerId => Ok(Response::LocalPeerId(
                self.swarm.local_peer_id().to_base58()
            )),
            Request::Peers => {
                let mut peers: Vec<Peer> = Vec::new();
                for (id, addr) in self.peers.iter() {
                    peers.push(Peer {
                        id: id.to_base58(),
                        addr: addr.to_string(),
                    });
                }
                Ok(Response::Peers(peers))
            },
        }
    }
}
