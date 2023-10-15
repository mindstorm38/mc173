//! The network server managing connected players and dispatching incoming packets.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use std::net::SocketAddr;
use std::ops::{Mul, Div};
use std::io;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use glam::{DVec3, Vec2, IVec3, IVec2};

use anyhow::Result as AnyResult;

use crate::chunk::{CHUNK_WIDTH, CHUNK_HEIGHT, calc_chunk_pos};
use crate::overworld::new_overworld;
use crate::world::{World, Event};

use crate::entity::{EntityGeneric, PlayerEntity, ItemEntity, PigEntity};

use crate::util::tcp::{TcpServer, TcpEvent, TcpEventKind};
use crate::proto::{ServerPacket, ClientPacket,
    ClientHandshakePacket, DisconnectPacket, ClientLoginPacket, SpawnPositionPacket, 
    UpdateTimePacket, ChatPacket, PositionLookPacket, ChunkDataPacket, 
    ChunkStatePacket, BreakBlockPacket, BlockChangePacket, ItemSpawnPacket, 
    PlayerSpawnPacket, MobSpawnPacket, EntityKillPacket, EntityMoveAndLookPacket, 
    EntityLookPacket, EntityMovePacket, EntityPositionAndLookPacket};


/// Target tick duration. Currently 20 TPS, so 50 ms/tick.
const TICK_DURATION: Duration = Duration::from_millis(50);
/// Timeout for TCP polling.
const TCP_TIMEOUT: Duration = Duration::from_millis(1);


/// This structure manages a whole server and its clients, dispatching incoming packets
/// to correct handlers.
pub struct Server {
    /// Global server resources.
    resources: Resources,
    /// The player manager.
    players: Players,
    /// Trackers for data of entity.
    entities: Entities,
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
            entities: Entities { 
                update_ticks: 0, 
                trackers: HashMap::new(),
                killed_trackers: Vec::new(),
            }
        })
    }

    pub fn run(&mut self) -> AnyResult<()> {
        loop {
            let start = Instant::now();
            self.tick()?;
            let elapsed = start.elapsed();
            if let Some(missing) = TICK_DURATION.checked_sub(elapsed) {
                std::thread::sleep(missing);
            }
        }
    }

    /// Run a single tick in the server.
    pub fn tick(&mut self) -> AnyResult<()> {

        let mut events: Vec<TcpEvent<ServerPacket>> = Vec::new();
        self.resources.tcp_server.poll(&mut events, Some(TCP_TIMEOUT))?;

        // Process each event with concerned client.
        for event in events.drain(..) {
            match event.kind {
                TcpEventKind::Accepted => {}
                TcpEventKind::Lost(err) => {
                    println!("[{}] Lost ({err:?})", event.client_id);
                    self.players.remove_player(event.client_id).handle_lost(&mut self.resources);
                    self.entities.remove_player(event.client_id);
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
                if player.playing.is_some() {
                    self.resources.tcp_server.send(player.client_id, &time_packet)?;
                }
            }
        }

        // NOTE: In the future it could be much better to just take the events from the
        // dimension and swap it with another events queue. This will avoid repeatedly 
        // copying.
        self.resources.overworld_events.extend(self.resources.overworld_dim.drain_events());
        for event in self.resources.overworld_events.drain(..) {
            // println!("[OVERWORLD] Event: {event:?}");
            match event {
                Event::EntitySpawn { id, pos, look } => {
                    let entity = self.resources.overworld_dim.entity(id).expect("incoherent entity");
                    let tracker = self.entities.new_tracker(entity);
                    tracker.set_pos(pos);
                    tracker.set_look(look);
                }
                Event::EntityKill { id } => {
                    self.entities.kill_tracker(id);
                }
                Event::EntityPosition { id, pos } => {
                    self.entities.tracker_mut(id).set_pos(pos);
                }
                Event::EntityLook { id, look } => {
                    self.entities.tracker_mut(id).set_look(look);
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

        // Tick entity trackers.
        self.entities.tick(&self.players, &self.resources.overworld_dim, &mut self.resources.tcp_server)?;

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
                    last_pos: DVec3::new(0.0, 67.0, 0.0),
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
            .flat_map(|(cx, cz)| self.iter_chunk_aware_players(cx, cz))
    }

    /// Iterate over players that are aware of a given chunk position.
    fn iter_chunk_aware_players(&self, cx: i32, cz: i32) -> impl Iterator<Item = &Player> {
        self.players.iter()
            .map(|player| &**player)
            .filter(move |player| {
                if let Some(playing) = &player.playing {
                    playing.sent_chunks.contains(&(cx, cz))
                } else {
                    false
                }
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

        let mut entity = PlayerEntity::new(self.last_pos);
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


/// Internal structure to keep tracks of all entities in order to send correct move,
/// velocity or look packets only when needed.
#[derive(Debug)]
struct Entities {
    /// Global ticks counter.
    update_ticks: u32,
    /// Entities by type.
    trackers: HashMap<u32, EntityTracker>,
    /// List of trackers to kill on the next tick.
    killed_trackers: Vec<u32>,  // TODO:
}

impl Entities {

    /// Create a new tracker for the given entity.
    fn new_tracker(&mut self, entity: &dyn EntityGeneric) -> &mut EntityTracker {
        self.trackers.entry(entity.id()).or_insert_with(|| {
            EntityTracker::new(entity)
        })
    }

    fn kill_tracker(&mut self, entity_id: u32) {
        self.killed_trackers.push(entity_id);
    }

    /// Get a mutable reference to a tracker.
    fn tracker_mut(&mut self, id: u32) -> &mut EntityTracker {
        self.trackers.get_mut(&id).expect("invalid entity")
    }

    /// Ensure that the given player has been removed internally.
    fn remove_player(&mut self, client_id: usize) {
        for tracker in self.trackers.values_mut() {
            tracker.client_ids.remove(&client_id);
        }
    }

    fn tick(&mut self, players: &Players, world: &World, server: &mut TcpServer) -> io::Result<()> {

        let update_players = self.update_ticks % 60 == 0;

        for tracker in self.trackers.values_mut() {

            if update_players || !tracker.first_update {
                for player in &players.players {
                    tracker.update_player(&player, world, server)?;
                }
            }

            tracker.forced_countdown_ticks += 1;
            if !tracker.first_update || self.update_ticks % tracker.interval == 0 {
                tracker.tick(server)?;
            }

        }
        
        self.update_ticks = self.update_ticks.wrapping_add(1);
        Ok(())

    }

}

/// This structure tracks every entity spawned in the world and save their previous 
/// position/look (and motion for some entities). It handle allows sending the right
/// packets to the right players when these properties are changed.
#[derive(Debug)]
struct EntityTracker {
    /// The entity id.
    entity_id: u32,
    /// Maximum tracking distance for this type of entity.
    distance: u32,
    /// Update interval for this type of entity.
    interval: u32,
    /// Last known position of the entity.
    pos: IVec3,
    /// Last known look of the entity.
    look: IVec2,
    /// Last encoded position sent to clients.
    sent_pos: IVec3,
    /// Last encoded look sent to clients.
    sent_look: IVec2,
    /// This counter is forced 
    forced_countdown_ticks: u16,
    /// Client ids that tracks this entity.
    client_ids: HashSet<usize>,
    /// Indicate if this tracker has been ticked at least once.
    first_update: bool,
}

impl EntityTracker {

    fn new(entity: &dyn EntityGeneric) -> Self {

        macro_rules! entity_match {
            (match ($entity:expr) { $($ty:ty => $value:expr),* $(,)? }) => {
                if false { unreachable!() } 
                $(else if $entity.is::<$ty>() { $value })* 
                else { panic!("unmatched entity type"); }
            };
        }

        let (distance, interval) = entity_match! {
            match (entity) {
                PlayerEntity => (512, 2),
                PigEntity => (160, 3),
                ItemEntity => (64, 20),
            }
        };

        Self {
            entity_id: entity.id(),
            distance,
            interval,
            forced_countdown_ticks: 0,
            pos: IVec3::ZERO,
            look: IVec2::ZERO,
            sent_pos: IVec3::ZERO,
            sent_look: IVec2::ZERO,
            client_ids: HashSet::new(),
            first_update: false,
        }

    }

    fn set_pos(&mut self, pos: DVec3) {
        self.pos = pos.mul(32.0).floor().as_ivec3();
        if !self.first_update {
            self.sent_pos = self.pos;
        }
    }

    fn set_look(&mut self, look: Vec2) {
        self.look = look.div(std::f32::consts::TAU).mul(256.0).as_ivec2();
        if !self.first_update {
            self.sent_look = self.look;
        }
    }

    /// Tick this tracker.
    fn tick(&mut self, server: &mut TcpServer) -> io::Result<()> {

        let delta_pos = self.pos - self.sent_pos;
        let delta_look = self.look - self.sent_look;

        let mut send_pos = true;
        let send_look = delta_look.x.abs() >= 8 || delta_look.y.abs() >= 8;

        // Check if the delta can be sent with a move packet.
        let dx = i8::try_from(delta_pos.x).ok();
        let dy = i8::try_from(delta_pos.y).ok();
        let dz = i8::try_from(delta_pos.z).ok();

        let mut move_packet = None;
        let forced_position = self.forced_countdown_ticks > 400;

        if let (false, Some(dx), Some(dy), Some(dz)) = (forced_position, dx, dy, dz) {

            // We don't send position if delta is too small.
            send_pos = dx.abs() >= 8 || dy.abs() >= 8 || dz.abs() >= 8;

            if send_pos && send_look {
                move_packet = Some(ClientPacket::EntityMoveAndLook(EntityMoveAndLookPacket {
                    entity_id: self.entity_id,
                    dx,
                    dy,
                    dz,
                    yaw: self.look.x as i8,
                    pitch: self.look.y as i8,
                }))
            } else if send_pos {
                move_packet = Some(ClientPacket::EntityMove(EntityMovePacket {
                    entity_id: self.entity_id,
                    dx,
                    dy,
                    dz,
                }))
            } else if send_look {
                move_packet = Some(ClientPacket::EntityLook(EntityLookPacket {
                    entity_id: self.entity_id,
                    yaw: self.look.x as i8,
                    pitch: self.look.y as i8,
                }))
            }

        } else {
            self.forced_countdown_ticks = 0;
            move_packet = Some(ClientPacket::EntityPositionAndLook(EntityPositionAndLookPacket {
                entity_id: self.entity_id,
                x: self.pos.x,
                y: self.pos.y,
                z: self.pos.z,
                yaw: self.look.x as i8,
                pitch: self.look.y as i8,
            }));
        }

        if send_pos {
            self.sent_pos = self.pos;
        }

        if send_look {
            self.sent_look = self.look;
        }

        if let Some(packet) = move_packet {
            for &client_id in &self.client_ids {
                server.send(client_id, &packet)?;
            }
        }

        self.first_update = true;

        Ok(())

    }

    /// Update a player regarding this tracker, if the player is in range, this function
    /// make sure that the player has the entity spawn on its side, if the player is not
    /// longer in range, the entity is killed on its side.
    fn update_player(&mut self, player: &Player, world: &World, server: &mut TcpServer) -> io::Result<()> {
        
        // Do nothing if player is not yet playing.
        let Some(playing) = &player.playing else { return Ok(()) };

        // Ignore this tracker if this is the player's entity.
        if playing.entity_id == self.entity_id {
            return Ok(());
        }

        let delta = player.last_pos - self.pos.as_dvec3() / 32.0;
        if delta.x.abs() <= self.distance as f64 && delta.z.abs() <= self.distance as f64 {
            if self.client_ids.insert(player.client_id) {
                self.spawn_player_entity(player, world, server)
            } else {
                Ok(())
            }
        } else if self.client_ids.remove(&player.client_id) {
            self.kill_player_entity(player, server)
        } else {
            Ok(())
        }

    }

    /// Spawn the entity on the player side.
    fn spawn_player_entity(&mut self, player: &Player, world: &World, server: &mut TcpServer) -> io::Result<()> {

        // NOTE: Silently ignore dead if the entity is dead, it will be killed later.
        let Some(entity) = world.entity(self.entity_id) else { return Ok(()) };

        if let Some(entity) = entity.downcast_ref::<PlayerEntity>() {
            server.send(player.client_id, &ClientPacket::PlayerSpawn(PlayerSpawnPacket {
                entity_id: entity.id,
                username: entity.base.living.username.clone(),
                x: self.sent_pos.x, 
                y: self.sent_pos.y, 
                z: self.sent_pos.z, 
                yaw: self.sent_look.x as i8,
                pitch: self.sent_look.y as i8,
                current_item: 0, // TODO:
            }))
        } else if let Some(entity) = entity.downcast_ref::<ItemEntity>() {
            let vel = entity.vel.mul(128.0).as_ivec3();
            server.send(player.client_id, &ClientPacket::ItemSpawn(ItemSpawnPacket { 
                entity_id: entity.id, 
                item: entity.base.item, 
                x: self.sent_pos.x, 
                y: self.sent_pos.y, 
                z: self.sent_pos.z, 
                vx: vel.x as i8,
                vy: vel.y as i8,
                vz: vel.z as i8,
            }))
        } else if let Some(entity) = entity.downcast_ref::<PigEntity>() {
            server.send(player.client_id, &ClientPacket::MobSpawn(MobSpawnPacket {
                entity_id: entity.id,
                kind: 90,
                x: self.sent_pos.x, 
                y: self.sent_pos.y, 
                z: self.sent_pos.z, 
                yaw: self.sent_look.x as i8,
                pitch: self.sent_look.y as i8,
                metadata: Vec::new(), // TODO:
            }))
        } else {
            unimplemented!("unknown entity");
        }

    }

    /// Kill the entity on the player side.
    fn kill_player_entity(&mut self, player: &Player, server: &mut TcpServer) -> io::Result<()> {
        server.send(player.client_id, &ClientPacket::EntityKill(EntityKillPacket { 
            entity_id: self.entity_id
        }))
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
