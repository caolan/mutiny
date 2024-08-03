use std::{fs, os::unix::net::UnixStream, path::PathBuf, process};

use clap::Parser;

mod protocol;
mod server;
mod dirs;
mod config;
mod client;
mod swarm;
mod store;

/// Runtime for peer-to-peer web apps
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Unix socket to bind to
    #[arg(short, long)]
    socket: Option<PathBuf>,

    /// Local peer's data directory
    #[arg(short, long)]
    data: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    println!("Starting server...");

    let args = Args::parse();
    println!("Parsed arguments: {:?}", args);

    let data_dir = args.data.unwrap_or_else(|| {
        println!("No data directory specified, using default.");
        dirs::open_app_data_dir().unwrap()
    });
    println!("Data directory: {:?}", data_dir);

    let socket_path = args.socket.unwrap_or_else(|| {
        println!("No socket specified, using default.");
        dirs::open_app_runtime_dir().unwrap().join("mutinyd.socket")
    });
    println!("Socket path: {:?}", socket_path);

    let keypair_path = data_dir.join("identity.key");
    let db_path = data_dir.join("data.db");

    if !data_dir.exists() {
        fs::create_dir_all(&data_dir).unwrap();
        println!("Created data directory: {:?}", data_dir);
    }
    println!("Database path: {:?}", db_path);

    if socket_path.exists() {
        match UnixStream::connect(&socket_path) {
            Ok(_) => {
                eprintln!("Error: Socket is already in use: {:?}", socket_path);
                process::exit(1); // Exit with an error code
            }
            Err(_) => {
                println!("Reusing unix socket: {:?}", socket_path);
                fs::remove_file(&socket_path).unwrap();
            }
        }
    }

    let config = config::Config::load(
        keypair_path.clone(),
        socket_path.clone(),
        db_path.clone()
    ).unwrap();

    server::Server::start(config).await.unwrap();
}