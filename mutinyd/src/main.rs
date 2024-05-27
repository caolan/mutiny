use tokio::{signal, io::BufReader, io::AsyncReadExt, io::AsyncWriteExt, net::UnixListener};
use serde::{Deserialize, Serialize};
use rmp_serde::Serializer;
use std::env::{var, consts::OS};
use std::path::{PathBuf, Path};
use std::os::unix::fs::PermissionsExt;
use std::error::Error;
use std::fs;

fn user_runtime_dir_path() -> Result<PathBuf, std::env::VarError> {
    if OS == "macos" {
        // Just pick something sensible
        Ok(PathBuf::from(var("HOME")?).join("Library/Caches/TemporaryItems"))
    } else {
        // Assume following freedesktop.org specification (Linux etc)
        Ok(PathBuf::from(var("XDG_RUNTIME_DIR")?))
    }
}

fn open_app_runtime_dir() -> Result<PathBuf, Box<dyn Error>> {
    // Determine application's runtime directory
    let p = user_runtime_dir_path()?.join("mutiny");
    // Ensure path exists
    fs::create_dir_all(&p)?;
    // Restrict to current user
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700))?;
    Ok(p)
}

fn get_socket_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(open_app_runtime_dir()?.join("mutinyd.socket"))
}

#[derive(Deserialize)]
enum Request {
    Ping,
}

#[derive(Serialize)]
enum Response {
    Pong,
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

#[tokio::main]
async fn main() {
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
    tokio::fs::remove_file(socket_path).await.unwrap();
}
