//! The network server managing connected players and dispatching incoming packets.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::Duration;
use std::io;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use glam::{DVec3, Vec2};

use anyhow::Result as AnyResult;

use crate::chunk::{CHUNK_WIDTH, CHUNK_HEIGHT};
use crate::overworld::new_overworld;
use crate::entity::PlayerEntity;
use crate::world::{World, Event};

use crate::util::tcp::{TcpServer, TcpEvent, TcpEventKind};
use crate::proto::{ServerPacket, ClientPacket,
    ClientHandshakePacket, DisconnectPacket, ClientLoginPacket, PlayerSpawnPositionPacket, 
    UpdateTimePacket, ChatPacket, PlayerPositionLookPacket, MapChunkPacket, 
    PreChunkPacket};


/// This structure manages a whole server and its clients, dispatching incoming packets
/// to correct handlers.
pub struct Server {
    /// Global server resources.
    resources: Resources,
    /// The player manager.
    players: Players,
}

/// Common resources of the server, this is passed to players' handling function for 
/// packets.
struct Resources {
    /// The internal server used to accept new clients and receive network packets.
    tcp_server: TcpServer,
    /// Future packets to broadcast.
    broadcast_packets: Vec<ClientPacket>,
    /// The game driver.
    overworld_dim: World,
    /// Temporary queue of overworld events.
    overworld_events: Vec<Event>,
}

struct Players {
    /// List of connected players.
    players: Vec<Box<Player>>,
    /// Mapping of client id to the runtime player.
    players_client_map: HashMap<usize, usize>,
}

/// A handle for a player connected to the server.
#[derive(Debug)]
struct Player {
    /// The packet client id.
    client_id: usize,
    /// Initial player position, as sent when first joining.
    last_pos: DVec3,
    /// Initial player look, as sent when first joining.
    last_look: Vec2,
    /// Present if the player has logged in and is bound to an entity.
    playing: Option<PlayingPlayer>,
}

#[derive(Debug)]
struct PlayingPlayer {
    /// The entity id linked to this player, set when player is connected.
    entity_id: u32,
    /// Indicates if the player's pos and look has been sent for initialization.
    initialized: bool,
    /// List of chunks that should be loaded by the client.
    sent_chunks: HashSet<(i32, i32)>,
}

impl Server {

    /// Bind this server's TCP listener to the given address.
    pub fn bind(addr: SocketAddr) -> AnyResult<Self> {
        Ok(Self {
            resources: Resources { 
                tcp_server: TcpServer::bind(addr)?,
                broadcast_packets: Vec::new(),
                overworld_dim: new_overworld(),
                overworld_events: Vec::new(),
            },
            players: Players {
                players: Vec::new(),
                players_client_map: HashMap::new(),
            },
        })
    }

    /// Run a single tick in the server.
    pub fn tick(&mut self) -> AnyResult<()> {

        let mut events: Vec<TcpEvent<ServerPacket>> = Vec::new();
        self.resources.tcp_server.poll(&mut events, Some(Duration::from_secs_f32(1.0 / 20.0)))?;

        // Process each event with concerned client.
        for event in events.drain(..) {
            match event.kind {
                TcpEventKind::Accepted => {}
                TcpEventKind::Lost(err) => {
                    println!("[{}] Lost ({err:?})", event.client_id);
                    self.players.remove_player(event.client_id);
                }
                TcpEventKind::Packet(packet) => {
                    println!("[{}] Received {packet:?}", event.client_id);
                    self.players.ensure_player(event.client_id).handle_packet(&mut self.resources, packet)?;
                }
            }
        }

        // Process pending broadcast packets (only to playing players).
        for packet in self.resources.broadcast_packets.drain(..) {
            for player in &self.players.players {
                if player.playing.is_some() {
                    self.resources.tcp_server.send(player.client_id, &packet)?;
                }
            }
        }

        self.resources.overworld_dim.tick();

        // Send time every second.
        let time = self.resources.overworld_dim.time();
        if time % 20 == 0 {
            let time_packet = ClientPacket::UpdateTime(UpdateTimePacket { time });
            for player in &self.players.players {
                self.resources.tcp_server.send(player.client_id, &time_packet)?;
            }
        }

        // NOTE: In the future it could be much better to just take the events from the
        // dimension and swap it with another events queue. This will avoid repeatedly 
        // copying.
        self.resources.overworld_events.extend(self.resources.overworld_dim.drain_events());
        self.resources.overworld_events.clear();

        Ok(())

    }

}

impl Players {

    /// Ensure that a player exists with the given client id, the the player was not
    /// existing, a new one is created with no playing state.
    fn ensure_player(&mut self, client_id: usize) -> &mut Player {
        match self.players_client_map.entry(client_id) {
            Entry::Occupied(o) => &mut self.players[*o.into_mut()],
            Entry::Vacant(v) => {

                let player = Box::new(Player {
                    client_id,
                    last_pos: DVec3::new(8.5, 67.0, 8.5),
                    last_look: Vec2::ZERO,
                    playing: None,
                });

                let index = self.players.len();
                v.insert(index);
                self.players.push(player);
                &mut self.players[index]

            }
        }
    }

    /// Remove a connected player while ensuring internal coherency.
    fn remove_player(&mut self, client_id: usize) {

        let index = self.players_client_map.remove(&client_id).expect("unknown client id");
        let _player = self.players.swap_remove(index);

        // We need to update the player that was swapped with the removed one, because
        // its index within the players list changed
        if let Some(player) = self.players.get(index) {
            // Remap the client id to its new index, and debug check that we are correct.
            let old_index = self.players_client_map.insert(player.client_id, index);
            debug_assert_eq!(old_index, Some(self.players.len()));
        }

    }

}

impl Player {

    /// Handle a server side packet received by this client.
    fn handle_packet(&mut self, res: &mut Resources, packet: ServerPacket) -> io::Result<()> {
        match packet {
            ServerPacket::Handshake(packet) =>
                self.handle_handshake(res, packet.username),
            ServerPacket::Login(packet) =>
                self.handle_login(res, packet.protocol_version, packet.username),
            ServerPacket::PlayerPosition(packet) => 
                self.handle_move(res,
                    Some(PlayerPosition { pos: packet.pos, stance: packet.stance }),
                    None,
                    packet.on_ground),
            ServerPacket::PlayerLook(packet) => 
                self.handle_move(res,
                    None, 
                    Some(PlayerLook { look: packet.look }), 
                    packet.on_ground),
            ServerPacket::PlayerPositionLook(packet) => 
                self.handle_move(res,
                    Some(PlayerPosition { pos: packet.pos, stance: packet.stance }), 
                    Some(PlayerLook { look: packet.look }),
                    packet.on_ground),
            _ => Ok(())
        }
    }

    /// This function handles the initial handshake packet.
    fn handle_handshake(&mut self, res: &mut Resources, _username: String) -> io::Result<()> {

        if self.playing.is_some() { return Ok(()) }

        res.tcp_server.send(self.client_id, &ClientPacket::Handshake(ClientHandshakePacket {
            server: "-".to_string()
        }))

    }

    /// This function handles the initial login packet.
    fn handle_login(&mut self, res: &mut Resources, protocol_version: i32, username: String) -> io::Result<()> {

        if self.playing.is_some() { return Ok(()) }

        if protocol_version != 14 {
            res.tcp_server.send(self.client_id, &ClientPacket::Disconnect(DisconnectPacket {
                reason: "Protocol version mismatch!".to_string()
            }))?;
            return Ok(());
        }

        let mut entity = PlayerEntity::new(DVec3::new(8.5, 66.0, 8.5));
        entity.base.living.username = username.clone();
        
        let entity_id = res.overworld_dim.spawn_entity(entity);

        self.playing = Some(PlayingPlayer {
            entity_id,
            initialized: false,
            sent_chunks: HashSet::new(),
        });

        res.tcp_server.send(self.client_id, &ClientPacket::Login(ClientLoginPacket {
            entity_id: entity_id as i32,
            random_seed: 0,
            dimension: 0,
        }))?;

        res.tcp_server.send(self.client_id, &ClientPacket::PlayerSpawnPosition(PlayerSpawnPositionPacket {
            pos: res.overworld_dim.spawn_pos(),
        }))?;

        res.tcp_server.send(self.client_id, &ClientPacket::UpdateTime(UpdateTimePacket {
            time: res.overworld_dim.time(),
        }))?;

        res.broadcast_packets.push(ClientPacket::Chat(ChatPacket {
            message: format!("{username} joined the game."),
        }));

        Ok(())

    }

    /// This function handles various positioning packets.
    fn handle_move(&mut self, 
        res: &mut Resources,
        pos: Option<PlayerPosition>, 
        look: Option<PlayerLook>, 
        on_ground: bool
    ) -> io::Result<()> {

        let Some(playing) = &mut self.playing else { return Ok(()); };
        let entity = res.overworld_dim.entity_mut(playing.entity_id).unwrap();

        if !playing.initialized {
            
            res.tcp_server.send(self.client_id, &ClientPacket::PlayerPositionLook(PlayerPositionLookPacket {
                pos: self.last_pos,
                look: self.last_look,
                stance: self.last_pos.y + 1.62,
                on_ground: false,
            }))?;

            playing.initialized = true;

        }

        if let Some(pos) = &pos {
            self.last_pos = pos.pos;
        }

        if let Some(look) = &look {
            self.last_look = look.look;
        }

        // let world = self.world_manager.world_mut(world_id).unwrap();

        let mut map_chunk_packet = ClientPacket::MapChunk(MapChunkPacket {
            x: 0, y: 0, z: 0, 
            x_size: CHUNK_WIDTH as u8, y_size: CHUNK_HEIGHT as u8, z_size: CHUNK_WIDTH as u8,
            compressed_data: Vec::new(),
        });

        for cx in -2..2 {
            for cz in -2..2 {

                if let Some(chunk) = res.overworld_dim.chunk(cx, cz) {
                    if playing.sent_chunks.insert((cx, cz)) {

                        res.tcp_server.send(self.client_id, &ClientPacket::PreChunk(PreChunkPacket {
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

                        res.tcp_server.send(self.client_id, &map_chunk_packet)?;

                    }
                }

            }
        }

        Ok(())

    }

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
