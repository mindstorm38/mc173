//! A Minecraft beta 1.7.3 server in Rust.

use std::sync::atomic::{AtomicBool, Ordering};

pub mod net;
pub mod proto;

// This modules use each others, this is usually a bad design but here this was too huge
// for a single module and it will be easier to maintain like this.  
pub mod world;
pub mod chunk;
pub mod entity;
pub mod offline;
pub mod player;

// This module link the previous ones to make a fully functional, multi-world server.
pub mod server;

/// Storing true while the server should run.
static RUNNING: AtomicBool = AtomicBool::new(true);

pub fn main() {

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    
    ctrlc::set_handler(|| RUNNING.store(false, Ordering::Relaxed)).unwrap();

    let mut server = server::Server::bind("127.0.0.1:25565".parse().unwrap()).unwrap();

    while RUNNING.load(Ordering::Relaxed) {
        server.tick_padded().unwrap();
    }

    server.save();
    
}
