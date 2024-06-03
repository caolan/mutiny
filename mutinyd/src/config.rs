use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::error::Error;
use libp2p::identity::Keypair;
use std::io::Write;
use std::fs;

use crate::dirs;

#[derive(Debug)]
pub struct Config {
    pub keypair: Keypair,
    pub socket_path: PathBuf,
}

impl Config {
    pub fn load(
        keypair_path: PathBuf,
        socket_path: PathBuf,
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
        Ok(Self { keypair, socket_path })
    }

    pub fn load_defaults() -> Result<Self, Box<dyn Error>> {
        let socket_path = dirs::open_app_runtime_dir()?.join("mutinyd.socket");
        let keypair_path = dirs::open_app_data_dir()?.join("identity.key");
        Self::load(keypair_path, socket_path)
    }
}
