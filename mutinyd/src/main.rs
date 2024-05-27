use tokio::{signal, io::BufReader, io::AsyncReadExt, io::AsyncWriteExt, net::UnixListener};
use std::os::unix::fs::PermissionsExt;
use serde::Serialize;
use rmp_serde::Serializer;
use std::path::{PathBuf, Path};
use std::error::Error;
use libp2p::identity::Keypair;
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
