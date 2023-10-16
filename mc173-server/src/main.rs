//! A Minecraft beta 1.7.3 server in Rust.

pub mod util;

pub mod packet;
pub mod proto;

pub mod server;


pub fn main() {
    use server::Server;
    let mut server = Server::bind("127.0.0.1:25565".parse().unwrap()).unwrap();
    server.run().unwrap();
}
