use std::{fs, path::PathBuf};
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
    let args = Args::parse();
    let data_dir = args.data.unwrap_or_else(|| dirs::open_app_data_dir().unwrap());
    let socket_path = args.socket.unwrap_or_else(|| dirs::open_app_runtime_dir().unwrap().join("mutinyd.socket"));
    let keypair_path = data_dir.join("identity.key");
    let db_path = data_dir.join("data.db");
    // Ensure data directory exists
    fs::create_dir_all(data_dir).unwrap();
    let config = config::Config::load(keypair_path, socket_path, db_path).unwrap();
    server::Server::start(config).await.unwrap();
}
