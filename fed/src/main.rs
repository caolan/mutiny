use tokio::{signal, io::AsyncWriteExt, net::UnixListener};

const SOCKET_PATH: &str = "./fed.socket";

async fn listen() {
    let listener = UnixListener::bind(SOCKET_PATH).unwrap();
    println!("Listening at {}", SOCKET_PATH);
    loop {
        match listener.accept().await {
            Ok((mut stream, _addr)) => {
                stream.write_all(b"Hello from fed").await.unwrap();
            }
            Err(_e) => { /* connection failed */ }
        }
    }
}

#[tokio::main]
async fn main() {
    // Cleans up unix socket after ctrl-c
    // TODO: clean up if socket was created by this process but a subsequent
    // error occurs
    tokio::select! {
        r = tokio::spawn(async { listen().await }) => r.unwrap(),
        r = signal::ctrl_c() => r.unwrap(),
    };
    println!("Removing {}", SOCKET_PATH);
    tokio::fs::remove_file(SOCKET_PATH).await.unwrap();
}
