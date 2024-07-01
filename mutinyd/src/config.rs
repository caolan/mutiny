use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::error::Error;
use libp2p::identity::Keypair;
use std::io::Write;
use std::fs;
use rusqlite;

#[derive(Debug)]
pub struct Config {
    pub keypair: Keypair,
    pub socket_path: PathBuf,
    pub db_connection: rusqlite::Connection,
}

impl Config {
    pub fn load(
        keypair_path: PathBuf,
        socket_path: PathBuf,
        db_path: PathBuf,
    ) -> Result<Self, Box<dyn Error>> {
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
        let db_connection = rusqlite::Connection::open(db_path)?;
        Ok(Self { keypair, socket_path, db_connection })
    }
}
