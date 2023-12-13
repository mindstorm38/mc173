//! A Minecraft beta 1.7.3 server in Rust.

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


pub fn main() {
    use server::Server;
    let mut server = Server::bind("127.0.0.1:25565".parse().unwrap()).unwrap();
    server.run().unwrap();
}
