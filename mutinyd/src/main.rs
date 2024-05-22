use tokio::{signal, io::BufReader, io::AsyncBufReadExt, io::AsyncWriteExt, net::UnixListener};
use std::env::{var, consts::OS};
use std::path::PathBuf;
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

async fn listen(socket_path: PathBuf) {
    let listener = UnixListener::bind(&socket_path).unwrap();
    println!("Listening at {:?}", socket_path);
    loop {
        match listener.accept().await {
            Ok((mut stream, _addr)) => {
                let (read, mut write) = stream.split();
                let reader = BufReader::new(read);
                let mut lines = reader.lines();
                while let Some(line) = lines.next_line().await.unwrap() {
                    println!("Received: {line}");
                    let msg = b"Hello from mutinyd\n";
                    write.write_all(msg).await.unwrap();
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
        r = tokio::spawn(async move { listen(socket_path2).await }) => r.unwrap(),
        r = signal::ctrl_c() => r.unwrap(),
    };
    println!("Removing {:?}", socket_path);
    tokio::fs::remove_file(socket_path).await.unwrap();
}
