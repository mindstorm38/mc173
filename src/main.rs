//! A Minecraft beta 1.7.3 server in Rust.

use crate::proto::{ServerEvent, ClientEvent};

pub mod block;
pub mod chunk;
pub mod world;
pub mod proto;
pub mod server;


fn main() {

    use proto::PacketServer;

    let mut server = PacketServer::bind("127.0.0.1:25565".parse().unwrap()).unwrap();
    let mut packets = Vec::new();

    loop {

        server.poll(&mut packets).unwrap();

        for packet in packets.drain(..) {

            println!("[{}] {:?}", packet.client, packet.event);

            match packet.event {
                ServerEvent::Handshake {
                    ref username
                } => {

                    println!("  Username: {username}");
                    server.send(packet.answer(ClientEvent::Handshake {
                        server: "-".to_string()
                    })).unwrap();

                }
                ServerEvent::Login { 
                    protocol_version,
                    ref username, 
                    ..
                } => {

                    println!("  Protocol version: {protocol_version}");
                    println!("  Username: {username}");

                    if protocol_version != 14 {
                        server.send(packet.answer(ClientEvent::Kick { 
                            reason: "Outdated server!".to_string()
                        })).unwrap();
                    } else {

                        server.send(packet.answer(ClientEvent::Login {
                            entity_id: 0,
                            random_seed: 0,
                            dimension: 0,
                        })).unwrap();

                    }

                }
                _ => {}
            }

        }

    }

}
