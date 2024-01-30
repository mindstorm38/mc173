//! A Minecraft beta 1.7.3 server in Rust.

use std::sync::atomic::{AtomicBool, Ordering};

use mc173::world::Dimension;

// The common configuration of the server.
pub mod config;

// The network modules, net is generic and proto is the implementation for b1.7.3.
pub mod net;
pub mod proto;

// This modules use each others, this is usually a bad design but here this was too huge
// for a single module and it will be easier to maintain like this.  
pub mod world;
pub mod chunk;
pub mod entity;
pub mod offline;
pub mod player;
pub mod command;

// This module link the previous ones to make a fully functional, multi-world server.
pub mod server;

/// Storing true while the server should run.
static RUNNING: AtomicBool = AtomicBool::new(true);


/// Entrypoint!
pub fn main() {

    init_tracing();

    ctrlc::set_handler(|| RUNNING.store(false, Ordering::Relaxed)).unwrap();

    let mut server = server::Server::bind("127.0.0.1:25565".parse().unwrap()).unwrap();
    server.register_world(format!("overworld"), Dimension::Overworld);

    while RUNNING.load(Ordering::Relaxed) {
        server.tick_padded().unwrap();
    }

    server.stop();
    
}

/// Initialize tracing to output into the console.
fn init_tracing() {

    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::EnvFilter;

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("debug"))
        .unwrap();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false);
    
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

}
