//! The network server managing connected players and dispatching incoming packets.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::Duration;
use std::io;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use glam::{DVec3, Vec2, IVec3};

use anyhow::Result as AnyResult;

use crate::chunk::{CHUNK_WIDTH, CHUNK_HEIGHT, calc_chunk_pos};
use crate::entity::{EntityGeneric, PlayerEntity, ItemEntity};
use crate::overworld::new_overworld;
use crate::world::{World, Event};

use crate::util::tcp::{TcpServer, TcpEvent, TcpEventKind};
use crate::proto::{ServerPacket, ClientPacket,
    ClientHandshakePacket, DisconnectPacket, ClientLoginPacket, SpawnPositionPacket, 
    UpdateTimePacket, ChatPacket, PositionLookPacket, ChunkDataPacket, 
    ChunkStatePacket, BreakBlockPacket, BlockChangePacket, ItemSpawnPacket, PlayerSpawnPacket};


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
                    self.players.remove_player(event.client_id).handle_lost(&mut self.resources);
                }
                TcpEventKind::Packet(packet) => {
                    // println!("[{}] Received {packet:?}", event.client_id);
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
        for event in self.resources.overworld_events.drain(..) {
            println!("[OVERWORLD] Event: {event:?}");
            match event {
                Event::EntitySpawn { id } => {

                    let entity = self.resources.overworld_dim.entity(id).unwrap();
                    let pos = entity.pos().as_ivec3();
                    let spawn_packet = entity_spawn_packet(entity);

                    // FIXME: Do not spawn player for itself.
                    for player in self.players.iter_aware_players(pos) {
                        self.resources.tcp_server.send(player.client_id, &spawn_packet)?;
                    }

                }
                Event::BlockChange { pos, new_block: new_id, new_metadata, .. } => {

                    let block_change_packet = ClientPacket::BlockChange(BlockChangePacket {
                        x: pos.x,
                        y: pos.y as i8,
                        z: pos.z,
                        block: new_id,
                        metadata: new_metadata,
                    });

                    for player in self.players.iter_aware_players(pos) {
                        self.resources.tcp_server.send(player.client_id, &block_change_packet)?;
                    }

                }
            }
        }

        Ok(())

    }

}


/// Internal structure for storing players and keeping internal coherency.
struct Players {
    /// List of connected players.
    players: Vec<Box<Player>>,
    /// Mapping of client id to the runtime player.
    players_client_map: HashMap<usize, usize>,
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
    fn remove_player(&mut self, client_id: usize) -> Box<Player> {

        let index = self.players_client_map.remove(&client_id).expect("unknown client id");
        let player = self.players.swap_remove(index);

        // We need to update the player that was swapped with the removed one, because
        // its index within the players list changed
        if let Some(player) = self.players.get(index) {
            // Remap the client id to its new index, and debug check that we are correct.
            let old_index = self.players_client_map.insert(player.client_id, index);
            debug_assert_eq!(old_index, Some(self.players.len()));
        }

        player

    }

    /// Iterate over players that are aware of a given world's position.
    fn iter_aware_players(&self, pos: IVec3) -> impl Iterator<Item = &Player> {
        calc_chunk_pos(pos).into_iter()
            .flat_map(|(cx, cz)| {
                self.players.iter()
                    .map(|player| &**player)
                    .filter(move |player| {
                        if let Some(playing) = &player.playing {
                            playing.sent_chunks.contains(&(cx, cz))
                        } else {
                            false
                        }
                    })
            })
    }

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

/// A handle for a player connected to the server and playing in a world.
#[derive(Debug, Default)]
struct PlayingPlayer {
    /// The entity id linked to this player, set when player is connected.
    entity_id: u32,
    /// The player's username.
    username: String,
    /// Indicates if the player's pos and look has been sent for initialization.
    initialized: bool,
    /// List of chunks that should be loaded by the client.
    sent_chunks: HashSet<(i32, i32)>,
    /// Breaking block state of the player.
    breaking_block: Option<BreakingBlock>,
}

impl Player {

    /// Called to drop the player when connection was lost.
    fn handle_lost(self, res: &mut Resources) {
        
        let Some(playing) = self.playing else { return };

        res.overworld_dim.kill_entity(playing.entity_id);
        

    }

    /// Handle a server side packet received by this client.
    fn handle_packet(&mut self, res: &mut Resources, packet: ServerPacket) -> io::Result<()> {
        match packet {
            ServerPacket::Handshake(packet) =>
                self.handle_handshake(res, packet.username),
            ServerPacket::Login(packet) =>
                self.handle_login(res, packet.protocol_version, packet.username),
            ServerPacket::Chat(packet) =>
                self.handle_chat(res, packet.message),
            ServerPacket::Position(packet) => 
                self.handle_move(res,
                    Some(PlayerPosition { pos: packet.pos, stance: packet.stance }),
                    None,
                    packet.on_ground),
            ServerPacket::Look(packet) => 
                self.handle_move(res,
                    None, 
                    Some(PlayerLook { look: packet.look }), 
                    packet.on_ground),
            ServerPacket::PositionLook(packet) => 
                self.handle_move(res,
                    Some(PlayerPosition { pos: packet.pos, stance: packet.stance }), 
                    Some(PlayerLook { look: packet.look }),
                    packet.on_ground),
            ServerPacket::BreakBlock(packet) =>
                self.handle_break_block(res, packet),
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
            username: username.clone(),
            ..Default::default()
        });

        res.tcp_server.send(self.client_id, &ClientPacket::Login(ClientLoginPacket {
            entity_id,
            random_seed: 0,
            dimension: 0,
        }))?;

        res.tcp_server.send(self.client_id, &ClientPacket::SpawnPosition(SpawnPositionPacket {
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

    /// This function handles char packets.
    fn handle_chat(&mut self, res: &mut Resources, message: String) -> io::Result<()> {
        
        let Some(playing) = &mut self.playing else { return Ok(()); };

        // Directly broadcast the message!
        res.broadcast_packets.push(ClientPacket::Chat(ChatPacket {
            message: format!("<{}> {message}", playing.username)
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
            
            res.tcp_server.send(self.client_id, &ClientPacket::PositionLook(PositionLookPacket {
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

        let mut map_chunk_packet = ClientPacket::ChunkData(ChunkDataPacket {
            x: 0, y: 0, z: 0, 
            x_size: CHUNK_WIDTH as u8, y_size: CHUNK_HEIGHT as u8, z_size: CHUNK_WIDTH as u8,
            compressed_data: Vec::new(),
        });

        for cx in -2..2 {
            for cz in -2..2 {

                let mut send_chunk_entities = false;

                if let Some(chunk) = res.overworld_dim.chunk(cx, cz) {
                    if playing.sent_chunks.insert((cx, cz)) {

                        res.tcp_server.send(self.client_id, &ClientPacket::ChunkState(ChunkStatePacket {
                            cx, cz, init: true
                        }))?;

                        if let ClientPacket::ChunkData(packet) = &mut map_chunk_packet {

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
                        send_chunk_entities = true;

                    }
                }

                if send_chunk_entities {
                    for entity in res.overworld_dim.iter_chunk_entities(cx, cz) {
                        let spawn_packet = entity_spawn_packet(entity);
                        res.tcp_server.send(self.client_id, &spawn_packet)?;
                    }
                }

            }
        }

        Ok(())

    }

    /// This function handles various positioning packets.
    fn handle_break_block(&mut self, 
        res: &mut Resources, 
        packet: BreakBlockPacket
    ) -> io::Result<()> {

        let Some(playing) = &mut self.playing else { return Ok(()); };

        let pos = IVec3::new(packet.x, packet.y as i32, packet.z);

        match packet.status {
            // Start breaking.
            0 => {

                let (block, _metadata) = res.overworld_dim.block_and_metadata(pos).expect("invalid chunk");
                playing.breaking_block = Some(BreakingBlock {
                    time: res.overworld_dim.time(),
                    pos,
                    block,
                });

            }
            // Stop breaking.
            2 => {

                if let Some(breaking_block) = playing.breaking_block.take() {
                    if breaking_block.pos == pos {
                        let (block, _metadata) = res.overworld_dim.block_and_metadata(pos).expect("invalid chunk");
                        if breaking_block.block == block {
                            res.overworld_dim.break_block(pos);
                        }
                    }
                }

            }
            _ => {}
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

#[derive(Debug, Clone)]
struct BreakingBlock {
    time: u64,
    pos: IVec3,
    block: u8,
}


fn entity_spawn_packet(entity: &dyn EntityGeneric) -> ClientPacket {
    if let Some(entity) = entity.downcast_ref::<ItemEntity>() {
        ClientPacket::ItemSpawn(ItemSpawnPacket::from_entity(entity))
    } else if let Some(entity) = entity.downcast_ref::<PlayerEntity>() {
        ClientPacket::PlayerSpawn(PlayerSpawnPacket::from_entity(entity))
    } else {
        todo!("entity_spawn_packet: {}", entity.type_name());
    }
}
