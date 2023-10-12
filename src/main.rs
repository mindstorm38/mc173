//! A Minecraft beta 1.7.3 server in Rust.

pub mod util;

pub mod block;
pub mod item;
pub mod entity;

pub mod chunk;
pub mod world;

pub mod overworld;

pub mod server;
pub mod proto;


fn main() {

    use server::Server;

    let mut server = Server::bind("127.0.0.1:25565".parse().unwrap()).unwrap();

    loop {
        server.tick().unwrap();
    }

}
