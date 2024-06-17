mod protocol;
mod server;
mod dirs;
mod config;
mod client;
mod swarm;
mod store;

#[tokio::main]
async fn main() {
    let config = config::Config::load_defaults().unwrap();
    server::Server::start(config).await.unwrap();
}
