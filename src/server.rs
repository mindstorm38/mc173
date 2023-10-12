//! The network server managing connected players and dispatching incoming packets.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::Duration;
use std::io;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use glam::{DVec3, Vec2};

use crate::chunk::{CHUNK_WIDTH, CHUNK_HEIGHT};
use crate::overworld::new_overworld;
use crate::entity::PlayerEntity;
use crate::world::World;

use crate::proto::{PacketServer, ServerPacket, ClientPacket, ClientId, 
    ClientHandshakePacket, DisconnectPacket, ClientLoginPacket, PlayerSpawnPositionPacket, 
    UpdateTimePacket, ChatPacket, PlayerPositionLookPacket, MapChunkPacket, 
    PreChunkPacket, Packets};


/// This structure manages a whole server and its clients, dispatching incoming packets
/// to correct handlers.
pub struct Server {
    /// Inner server, split to avoid borrowing issues with the received packets list.
    inner: InnerServer,
    /// Received packets queue, split from the inner server to avoid borrowing issues.
    packets: Packets,
}

struct InnerServer {
    /// The internal server used to accept new clients and receive network packets.
    packet_server: PacketServer,
    /// The player manager.
    player_manager: PlayerManager,
    /// The game driver.
    overworld_dim: World,
}

impl Server {

    /// Bind this server's TCP listener to the given address.
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            inner: InnerServer {
                packet_server: PacketServer::bind(addr)?,
                player_manager: PlayerManager { 
                    players: HashMap::new(),
                    runtime_players: HashMap::new(),
                },
                overworld_dim: new_overworld(),
            },
            packets: Packets::new(),
        })
    }

    /// Run a single tick in the server.
    pub fn tick(&mut self) -> io::Result<()> {

        self.inner.packet_server.poll(&mut self.packets, Some(Duration::from_secs_f32(1.0 / 20.0)))?;

        for (client_id, packet) in self.packets.drain() {
            self.inner.handle(client_id, packet)?;
        }

        self.inner.overworld_dim.tick();

        Ok(())

    }

}

impl InnerServer {

    /// Handle a server side packet received by this client.
    fn handle(&mut self, client_id: ClientId, packet: ServerPacket) -> io::Result<()> {
        match packet {
            ServerPacket::Handshake(packet) => {
                self.handle_handshake(client_id, packet.username)
            }
            ServerPacket::Login(packet) => {
                self.handle_login(client_id, packet.protocol_version, packet.username)
            }
            ServerPacket::PlayerPosition(packet) => self.handle_move(
                client_id,
                Some(PlayerPosition { pos: packet.pos, stance: packet.stance }),
                None,
                packet.on_ground
            ),
            ServerPacket::PlayerLook(packet) => self.handle_move(
                client_id,
                None, 
                Some(PlayerLook { look: packet.look }), 
                packet.on_ground
            ),
            ServerPacket::PlayerPositionLook(packet) => self.handle_move(
                client_id,
                Some(PlayerPosition { pos: packet.pos, stance: packet.stance }), 
                Some(PlayerLook { look: packet.look }),
                packet.on_ground
            ),
            _ => Ok(())
        }
    }

    /// This function handles the initial handshake packet.
    fn handle_handshake(&mut self, client_id: ClientId, username: String) -> io::Result<()> {
        println!("  Username: {username}");
        self.packet_server.send(client_id, &ClientPacket::Handshake(ClientHandshakePacket {
            server: "-".to_string()
        }))
    }

    /// This function handles the initial login packet.
    fn handle_login(&mut self, client_id: ClientId, protocol_version: i32, username: String) -> io::Result<()> {

        println!("  Protocol version: {protocol_version}");
        println!("  Username: {username}");

        if protocol_version != 14 {
            self.packet_server.send(client_id, &ClientPacket::Disconnect(DisconnectPacket {
                reason: "Protocol version mismatch!".to_string()
            }))?;
            return Ok(());
        }

        let player = self.player_manager.connect_player(username.clone()).unwrap();

        let mut entity = PlayerEntity::new(DVec3::new(8.5, 66.0, 8.5));
        entity.base.living.username = username.clone();
        
        let entity_id = self.overworld_dim.spawn_entity(entity);

        self.player_manager.runtime_players.insert(client_id, RuntimePlayer { 
            entity_id,
            sent_chunks: HashSet::new(),
            last_position: None,
        });

        self.packet_server.send(client_id, &ClientPacket::Login(ClientLoginPacket {
            entity_id: entity_id as i32,
            random_seed: 0,
            dimension: 0,
        }))?;

        self.packet_server.send(client_id, &ClientPacket::PlayerSpawnPosition(PlayerSpawnPositionPacket {
            pos: self.overworld_dim.spawn_pos(),
        }))?;

        self.packet_server.send(client_id, &ClientPacket::UpdateTime(UpdateTimePacket {
            time: self.overworld_dim.time(),
        }))?;

        let join_message = ClientPacket::Chat(ChatPacket {
            message: format!("{username} joined the game."),
        });

        for &client_id in self.player_manager.runtime_players.keys() {
            self.packet_server.send(client_id, &join_message)?;
        }

        Ok(())

    }

    /// This function handles various positioning packets.
    fn handle_move(&mut self, 
        client_id: ClientId,
        pos: Option<PlayerPosition>, 
        look: Option<PlayerLook>, 
        on_ground: bool
    ) -> io::Result<()> {

        let player = self.player_manager.runtime_players.get_mut(&client_id).unwrap();

        // let chunk_pos = calc_chunk_pos(entity.pos.as_ivec3()).0;

        if let None = player.last_position {
            
            self.packet_server.send(client_id, &ClientPacket::PlayerPositionLook(PlayerPositionLookPacket {
                pos: DVec3::new(8.5, 66.0, 8.5),
                look: Vec2::ZERO,
                stance: 67.62,
                on_ground: false,
            }))?;

            player.last_position = Some(PlayerPosition {
                pos: DVec3::new(8.5, 66.0, 8.5),
                stance: 67.16,
            });

        }

        // if let Some(pos) = &pos {
        //     entity.pos = pos.pos;
        //     player.last_position = Some(pos.clone());
        // }

        // if let Some(look) = &look {
        //     entity.look = look.look;
        // }

        // let world = self.world_manager.world_mut(world_id).unwrap();

        let mut map_chunk_packet = ClientPacket::MapChunk(MapChunkPacket {
            x: 0, y: 0, z: 0, 
            x_size: CHUNK_WIDTH as u8, y_size: CHUNK_HEIGHT as u8, z_size: CHUNK_WIDTH as u8,
            compressed_data: Vec::new(),
        });

        for cx in -2..2 {
            for cz in -2..2 {

                if let Some(chunk) = self.overworld_dim.chunk(cx, cz) {
                    if player.sent_chunks.insert((cx, cz)) {

                        self.packet_server.send(client_id, &ClientPacket::PreChunk(PreChunkPacket {
                            cx, cz, init: true
                        }))?;

                        if let ClientPacket::MapChunk(packet) = &mut map_chunk_packet {

                            packet.x = cx * CHUNK_WIDTH as i32;
                            packet.z = cz * CHUNK_WIDTH as i32;

                            packet.compressed_data.clear();
                            let mut encoder = ZlibEncoder::new(&mut packet.compressed_data, Compression::fast());
                            chunk.write_data_to(&mut encoder)?;
                            encoder.finish()?;

                        } else {
                            unreachable!();
                        }

                        self.packet_server.send(client_id, &map_chunk_packet)?;

                    }
                }

            }
        }

        Ok(())

    }

}


#[derive(Debug)]
pub struct PlayerManager {
    /// All players registered in this server, connected or not, mapped to their username.
    players: HashMap<String, Player>,
    /// Mapping of client id to the runtime player.
    runtime_players: HashMap<ClientId, RuntimePlayer>,
}

impl PlayerManager {

    pub fn connect_player(&mut self, username: String) -> Option<&Player> {

        let player = self.players.entry(username)
            .or_insert_with_key(|username| {
                Player { 
                    username: username.clone(),
                }
            });
        
        Some(player)

    }

}

/// Represent a player known to the server.
#[derive(Debug)]
pub struct Player {
    /// The username of the player.
    username: String,
}

#[derive(Debug)]
pub struct RuntimePlayer {
    /// The entity id linked to this player, set when player is connected.
    entity_id: u32,
    /// List of chunks that should be loaded by the client.
    sent_chunks: HashSet<(i32, i32)>,
    /// Last known position of this player, this is used to resend it when player is not
    /// yet initialized.
    last_position: Option<PlayerPosition>,
}

#[derive(Debug, Clone)]
struct PlayerPosition {
    pos: DVec3,
    stance: f64,
}

#[derive(Debug, Clone)]
struct PlayerLook {
    look: Vec2,
}
