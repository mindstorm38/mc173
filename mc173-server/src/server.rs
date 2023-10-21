//! The network server managing connected players and dispatching incoming packets.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use std::net::SocketAddr;
use std::ops::{Mul, Div};
use std::io;

use anyhow::Result as AnyResult;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use glam::{DVec3, Vec2, IVec3, IVec2};

use mc173::chunk::{calc_entity_chunk_pos, calc_chunk_pos_unchecked, CHUNK_WIDTH, CHUNK_HEIGHT};
use mc173::world::{World, Dimension, Event};
use mc173::entity::{Entity, PlayerEntity};
use mc173::item::ItemStack;

use crate::proto::{self, Network, NetworkEvent, NetworkClient, InPacket, OutPacket};
use crate::overworld::new_overworld;


/// Target tick duration. Currently 20 TPS, so 50 ms/tick.
const TICK_DURATION: Duration = Duration::from_millis(50);


/// This structure manages a whole server and its clients, dispatching incoming packets
/// to correct handlers.
pub struct Server {
    /// Packet server handle.
    net: Network,
    /// Clients of this server, these structures track the network state of each client.
    clients: HashMap<NetworkClient, ClientState>,
    /// Worlds list.
    worlds: Vec<ServerWorld>,
    /// Offline players
    offline_players: HashMap<String, OfflinePlayer>,
}

impl Server {

    /// Bind this server's TCP listener to the given address.
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            net: Network::bind(addr)?,
            clients: HashMap::new(),
            worlds: vec![
                ServerWorld::new("overworld", new_overworld()),
            ],
            offline_players: HashMap::new(),
        })
    }

    /// Rick the game at an approximately constant tick rate.
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

        // Poll all network events.
        while let Some(event) = self.net.poll()? {
            match event {
                NetworkEvent::Accept { client } => 
                    self.handle_accept(client),
                NetworkEvent::Lost { client, error } => 
                    self.handle_lost(client, error),
                NetworkEvent::Packet { client, packet } => 
                    self.handle_packet(client, packet),
            }
        }

        for world in &mut self.worlds {
            world.tick();
        }

        Ok(())

    }

    /// Handle new client accepted by the network.
    fn handle_accept(&mut self, client: NetworkClient) {
        println!("[{client:?}] Accepted");
        self.clients.insert(client, ClientState::Handshaking);
    }

    /// Handle a lost client.
    fn handle_lost(&mut self, client: NetworkClient, error: Option<io::Error>) {

        println!("[{client:?}] Lost: {error:?}");
        let state = self.clients.remove(&client).unwrap();
        
        if let ClientState::Playing { world_index, player_index } = state {
            // If the client was playing, remove it from its world.
            let world = &mut self.worlds[world_index];
            if let Some(swapped_player) = world.handle_player_leave(player_index, true) {
                // If a player has been swapped in place of the removed one, update the 
                // swapped one to point to its new index (and same world).
                let state = self.clients.get_mut(&swapped_player.client)
                    .expect("swapped player should be existing");
                *state = ClientState::Playing { world_index, player_index };
            }
        }

    }

    fn handle_packet(&mut self, client: NetworkClient, packet: InPacket) {
        
        // println!("[{client:?}] Packet: {packet:?}");

        match *self.clients.get(&client).unwrap() {
            ClientState::Handshaking => {
                self.handle_handshaking(client, packet);
            }
            ClientState::Playing { world_index, player_index } => {
                let world = &mut self.worlds[world_index];
                let player = &mut world.players[player_index];
                player.handle(&mut world.world, packet);
            }
        }

    }

    /// Handle a packet for a client that is in handshaking state.
    fn handle_handshaking(&mut self, client: NetworkClient, packet: InPacket) {
        match packet {
            InPacket::KeepAlive => {}
            InPacket::Handshake(_) => 
                self.handle_handshake(client),
            InPacket::Login(packet) =>
                self.handle_login(client, packet),
            _ => self.send_disconnect(client, format!("Invalid packet: {packet:?}"))
        }
    }

    /// Handle a handshake from a client that is still handshaking, there is no 
    /// restriction.
    fn handle_handshake(&mut self, client: NetworkClient) {
        self.net.send(client, OutPacket::Handshake(proto::OutHandshakePacket {
            server: "-".to_string(),
        }));
    }

    /// Handle a login after handshake.
    fn handle_login(&mut self, client: NetworkClient, packet: proto::InLoginPacket) {

        if packet.protocol_version != 14 {
            self.send_disconnect(client, format!("Protocol version mismatch!"));
            return;
        }

        // Get the offline player, if not existing we create a new one with the 
        let offline_player = self.offline_players.entry(packet.username.clone())
            .or_insert_with(|| {
                let spawn_world = &self.worlds[0];
                OfflinePlayer {
                    world: spawn_world.name.clone(),
                    pos: spawn_world.world.spawn_position(),
                    look: Vec2::ZERO,
                }
            });

        let (world_index, world) = self.worlds.iter_mut()
            .enumerate()
            .filter(|(_, world)| world.name == offline_player.world)
            .next()
            .expect("invalid offline player world name");

        let mut entity = PlayerEntity::default();
        entity.kind.kind.username = packet.username.clone();
        entity.pos = offline_player.pos;
        entity.look = offline_player.look;
        let entity_id = world.world.spawn_entity(Entity::Player(entity));

        // Confirm the login by sending same packet in response.
        self.net.send(client, OutPacket::Login(proto::OutLoginPacket {
            entity_id,
            random_seed: 0,
            dimension: match world.world.dimension() {
                Dimension::Overworld => 0,
                Dimension::Nether => -1,
            },
        }));

        // The standard server sends the spawn position just after login response.
        self.net.send(client, OutPacket::SpawnPosition(proto::SpawnPositionPacket {
            pos: world.world.spawn_position().as_ivec3(),
        }));

        // Send the initial position for the client.
        self.net.send(client, OutPacket::PositionLook(proto::PositionLookPacket {
            pos: offline_player.pos,
            stance: offline_player.pos.y + 1.62,
            look: offline_player.look,
            on_ground: false,
        }));

        // Time must be sent once at login to conclude the login phase.
        self.net.send(client, OutPacket::UpdateTime(proto::UpdateTimePacket {
            time: world.world.time(),
        }));

        // Finally insert the player tracker.
        let player_index = world.handle_player_join(ServerPlayer {
            net: self.net.clone(),
            client,
            entity_id,
            username: packet.username,
            pos: offline_player.pos,
            look: offline_player.look,
            tracked_chunks: HashSet::new(),
            tracked_entities: HashSet::new(),
            breaking_block: None,
        });

        // Replace the previous state with a playing state containing the world and 
        // player indices, used to get to the player instance.
        let previous_state = self.clients.insert(client, ClientState::Playing {
            world_index,
            player_index,
        });

        // Just a sanity check...
        debug_assert_eq!(previous_state, Some(ClientState::Handshaking));

        // TODO: Broadcast chat joining chat message.

    }

    /// Send disconnect (a.k.a. kick) to a client.
    fn send_disconnect(&mut self, client: NetworkClient, reason: String) {
        self.net.send(client, OutPacket::Disconnect(proto::DisconnectPacket {
            reason,
        }))
    }

}

/// An offline player defines the saved data of a player that is not connected.
#[derive(Debug)]
struct OfflinePlayer {
    /// World name.
    world: String,
    /// Last saved position of the player.
    pos: DVec3,
    /// Last saved look of the player.
    look: Vec2,
}

/// Track state of a network client in the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClientState {
    /// This client is not yet connected to the world.
    Handshaking,
    /// This client is actually playing into a world.
    Playing {
        /// Index of the world this player is in.
        world_index: usize,
        /// Index of the player within the server world.
        player_index: usize,
    }
}

/// A single world in the server, this structure keep tracks of players and entities
/// tracked by players.
struct ServerWorld {
    /// World name.
    name: String,
    /// The inner world data structure.
    world: World,
    /// True when the world has been ticked once.
    init: bool,
    /// Entity tracker, each is associated to the entity id.
    trackers: HashMap<u32, EntityTracker>,
    /// Players currently in the world.
    players: Vec<ServerPlayer>,
}

impl ServerWorld {

    /// Internal function to create a server world.
    fn new(name: impl Into<String>, mut inner: World) -> Self {

        // Make sure that the world initially have an empty events queue.
        inner.swap_events(Some(Vec::new()));

        Self {
            name: name.into(),
            world: inner,
            init: false,
            trackers: HashMap::new(),
            players: Vec::new(),
        }

    }

    /// Tick this world.
    fn tick(&mut self) {

        if !self.init {
            self.handle_init();
            self.init = true;
        }

        self.world.tick();

        // Send time to every playing clients every second.
        let time = self.world.time();
        if time % 20 == 0 {
            for player in &self.players {
                player.send(OutPacket::UpdateTime(proto::UpdateTimePacket {
                    time,
                }));
            }
        }

        // Swap events out in order to proceed them.
        let mut events = self.world.swap_events(None).expect("events should be enabled");
        for event in events.drain(..) {
            match event {
                Event::EntitySpawn { id } =>
                    self.handle_entity_spawn(id),
                Event::EntityKill { id } => 
                    self.handle_entity_kill(id),
                Event::EntityPosition { id, pos } => 
                    self.handle_entity_position(id, pos),
                Event::EntityLook { id, look } =>
                    self.handle_entity_look(id, look),
                Event::EntityPickup { id, target_id } =>
                    self.handle_entity_pickup(id, target_id),
                Event::EntityInventoryItem { id, index, item } =>
                    self.handle_entity_inventory_item(id, index, item),
                Event::BlockChange { pos, new_block, new_metadata, .. } => 
                    self.handle_block_change(pos, new_block, new_metadata),
                Event::SpawnPosition { pos } =>
                    self.handle_spawn_position(pos),
            }
            // println!("[WORLD] Event: {event:?}");
        }

        // Reinsert events after processing.
        self.world.swap_events(Some(events));

        // After world events are processed, tick entity trackers.
        for tracker in self.trackers.values_mut() {

            if time % 60 == 0 {
                tracker.update_tracking_players(&mut self.players, &self.world);
            }

            tracker.forced_countdown_ticks += 1;
            if tracker.interval != 0 && time % tracker.interval as u64 == 0 {
                tracker.update_players(&self.players);
            }

        }

    }
    
    /// Initialize the world by ensuring that every entity is currently tracked. This
    /// method can be called multiple time and should be idempotent.
    fn handle_init(&mut self) {

        // Ensure that every entity has a tracker.
        for entity in self.world.iter_entities() {
            self.trackers.entry(entity.base().id).or_insert_with(|| {
                let tracker = EntityTracker::new(entity);
                tracker.update_tracking_players(&mut self.players, &self.world);
                tracker
            });
        }

    }

    /// Handle a player joining this world.
    fn handle_player_join(&mut self, mut player: ServerPlayer) -> usize {

        // Initial tracked entities.
        for tracker in self.trackers.values() {
            tracker.update_tracking_player(&mut player, &self.world);
        }

        player.update_chunks(&mut self.world);
        
        let player_index = self.players.len();
        self.players.push(player);
        player_index

    }

    /// Handle a player leaving this world, this should remove its entity. The `lost`
    /// argument indicates if the player is leaving because of a lost connection or not.
    /// If the connection was not lost, chunks and entities previously tracked by the
    /// player are send to be untracked. 
    /// 
    /// **Note that** this function swap remove the player, so the last player in this
    /// world's list is moved to the given player index. So if it exists, you should 
    /// update all indices pointing to the swapped player. This method returns, if 
    /// existing, the player that was swapped.
    fn handle_player_leave(&mut self, player_index: usize, lost: bool) -> Option<&ServerPlayer> {

        // Remove the player tracker.
        let mut player = self.players.swap_remove(player_index);
        
        // Kill the entity associated to the player.
        self.world.kill_entity(player.entity_id);

        // If player has not lost connection but it's just leaving the world, we just
        // send it untrack packets.
        if !lost {
            
            // Take and replace it with an empty set (no overhead).
            let tracked_entities = std::mem::take(&mut player.tracked_entities);

            // Untrack all its entities.
            for entity_id in tracked_entities {
                let tracker = self.trackers.get(&entity_id).expect("incoherent tracked entity");
                tracker.kill_player_entity(&mut player);
            }

        }

        self.players.get(player_index)

    }

    /// Handle an entity spawn world event.
    fn handle_entity_spawn(&mut self, id: u32) {
        let entity = self.world.entity(id).expect("incoherent event entity");
        self.trackers.entry(id).or_insert_with(|| {
            let tracker = EntityTracker::new(entity);
            tracker.update_tracking_players(&mut self.players, &self.world);
            tracker
        });
    }

    /// Handle an entity kill world event.
    fn handle_entity_kill(&mut self, id: u32) {
        let tracker = self.trackers.remove(&id).expect("incoherent event entity");
        tracker.untrack_players(&mut self.players);
    }

    /// Handle an entity position world event.
    fn handle_entity_position(&mut self, id: u32, pos: DVec3) {
        self.trackers.get_mut(&id).unwrap().set_pos(pos);
    }

    /// Handle an entity look world event.
    fn handle_entity_look(&mut self, id: u32, look: Vec2) {
        self.trackers.get_mut(&id).unwrap().set_look(look);
    }

    /// Handle an entity pickup world event.
    fn handle_entity_pickup(&mut self, id: u32, target_id: u32) {
        for player in &self.players {
            if player.tracked_entities.contains(&target_id) {
                player.send(OutPacket::EntityPickup(proto::EntityPickupPacket {
                    entity_id: id,
                    picked_entity_id: target_id,
                }));
            }
        }
    }

    /// Handle an entity inventory item world event. We support only this for player 
    /// entities, therefore the index must be in range `0..36`, and the first 9 slots
    /// are the hotbar, the rest is the inventory from top row to bottom row.
    fn handle_entity_inventory_item(&mut self, id: u32, index: usize, item: ItemStack) {

        let Some(player) = self.players.iter().find(move |p| p.entity_id == id) else { return };

        let slot = match index {
            0..=8 => 36 + index,
            _ => index,
        };

        player.send(OutPacket::WindowSetItem(proto::WindowSetItemPacket {
            window_id: 0,
            slot: slot as i16,
            item: Some(item),
        }));

    }

    /// Handle a block change world event.
    fn handle_block_change(&mut self, pos: IVec3, block: u8, metadata: u8) {
        let (cx, cz) = calc_chunk_pos_unchecked(pos);
        for player in &self.players {
            if player.tracked_chunks.contains(&(cx, cz)) {
                player.send(OutPacket::BlockChange(proto::BlockChangePacket {
                    x: pos.x,
                    y: pos.y as i8,
                    z: pos.z,
                    block,
                    metadata,
                }));
            }
        }
    }

    /// Handle a dynamic update of the spawn position.
    fn handle_spawn_position(&mut self, pos: DVec3) {
        let pos = pos.floor().as_ivec3();
        for player in &self.players {
            player.send(OutPacket::SpawnPosition(proto::SpawnPositionPacket {
                pos,
            }))
        }
    }

}

/// A server player is an actual 
struct ServerPlayer {
    /// A packet server handle.
    net: Network,
    /// The network client the player is managed by.
    client: NetworkClient,
    /// The entity id linked to this player.
    entity_id: u32,
    /// Its username.
    #[allow(unused)] // FIXME: Use this for chat message for example.
    username: String,
    /// Last position sent by the client.
    pos: DVec3,
    /// Last look sent by the client.
    look: Vec2,
    /// Set of chunks that are already sent to the player.
    tracked_chunks: HashSet<(i32, i32)>,
    /// Set of tracked entities by this player, all entity ids in this set are considered
    /// known and rendered by the client, when the entity will disappear, a kill packet
    /// should be sent.
    tracked_entities: HashSet<u32>,
    /// If the player is breaking a block, this record the breaking state.
    breaking_block: Option<BreakingBlock>,
}

/// State of a player breaking a block.
struct BreakingBlock {
    /// The world time when breaking started.
    #[allow(unused)] // FIXME: Check breaking time in the future.
    time: u64,
    /// The position of the block.
    pos: IVec3,
    /// The block id.
    block: u8,
}

impl ServerPlayer {

    /// Send a packet to this player.
    fn send(&self, packet: OutPacket) {
        self.net.send(self.client, packet);
    }

    /// Handle an incoming packet from this player.
    fn handle(&mut self, world: &mut World, packet: InPacket) {
        
        match packet {
            InPacket::KeepAlive => {}
            InPacket::Disconnect(_) =>
                self.handle_disconnect(),
            InPacket::Position(packet) => 
                self.handle_position(world, packet),
            InPacket::Look(packet) => 
                self.handle_look(world, packet),
            InPacket::PositionLook(packet) => 
                self.handle_position_look(world, packet),
            InPacket::BreakBlock(packet) =>
                self.handle_break_block(world, packet),
            _ => {}
        }

    }

    /// Just disconnect itself, this will produce a lost event from the network.
    fn handle_disconnect(&mut self) {
        self.net.disconnect(self.client);
    }

    /// Handle a position packet.
    fn handle_position(&mut self, world: &mut World, packet: proto::PositionPacket) {
        self.handle_position_look_inner(world, Some(packet.pos), None);
    }

    /// Handle a look packet.
    fn handle_look(&mut self, world: &mut World, packet: proto::LookPacket) {
        self.handle_position_look_inner(world, None, Some(packet.look));
    }

    /// Handle a position and look packet.
    fn handle_position_look(&mut self, world: &mut World, packet: proto::PositionLookPacket) {
        self.handle_position_look_inner(world, Some(packet.pos), Some(packet.look));
    }

    fn handle_position_look_inner(&mut self, world: &mut World, pos: Option<DVec3>, look: Option<Vec2>) {

        let entity = world.entity_mut(self.entity_id).expect("incoherent player entity");
        let entity_base = entity.base_mut();

        if let Some(pos) = pos {
            self.pos = pos;
            entity_base.pos = self.pos;
            entity_base.pos_dirty = true;
        }

        if let Some(look) = look {
            self.look = Vec2::new(look.x.to_radians(), look.y.to_radians());
            entity_base.look = self.look;
            entity_base.look_dirty = true;
        }

    }

    /// Handle a break block packet.
    fn handle_break_block(&mut self, world: &mut World, packet: proto::BreakBlockPacket) {

        let pos = IVec3::new(packet.x, packet.y as i32, packet.z);

        if packet.status == 0 {
            // Start breaking a block, ignore if the position is invalid.
            if let Some((block, _)) = world.block_and_metadata(pos) {
                self.breaking_block = Some(BreakingBlock {
                    time: world.time(),
                    pos,
                    block,
                });
            }
        } else if packet.status == 2 {
            // Block breaking should be finished.
            if let Some(state) = self.breaking_block.take() {
                if state.pos == pos && matches!(world.block_and_metadata(pos), Some((block, _)) if block == state.block) {
                    world.break_block(pos);
                }
            }
        }

    }

    /// Update the chunks sent to this player.
    fn update_chunks(&mut self, world: &mut World) {

        let (ocx, ocz) = calc_entity_chunk_pos(self.pos);
        let view_range = 3;

        for cx in (ocx - view_range)..(ocx + view_range) {
            for cz in (ocz - view_range)..(ocz + view_range) {

                if let Some(chunk) = world.chunk(cx, cz) {
                    if self.tracked_chunks.insert((cx, cz)) {

                        self.send(OutPacket::ChunkState(proto::ChunkStatePacket {
                            cx, cz, init: true
                        }));

                        let mut compressed_data = Vec::new();

                        let mut encoder = ZlibEncoder::new(&mut compressed_data, Compression::fast());
                        chunk.write_data_to(&mut encoder).unwrap();
                        encoder.finish().unwrap();

                        self.send(OutPacket::ChunkData(proto::ChunkDataPacket {
                            x: cx * CHUNK_WIDTH as i32,
                            y: 0, 
                            z: cz * CHUNK_WIDTH as i32, 
                            x_size: CHUNK_WIDTH as u8, 
                            y_size: CHUNK_HEIGHT as u8, 
                            z_size: CHUNK_WIDTH as u8,
                            compressed_data,
                        }));

                    }
                }

            }
        }

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
    distance: u16,
    /// Update interval for this type of entity.
    interval: u16,
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
}

impl EntityTracker {

    fn new(entity: &Entity) -> Self {

        let (distance, interval) = match entity {
            Entity::Player(_) => (512, 2),
            Entity::Fish(_) => (64, 5),
            Entity::Arrow(_) => (64, 20),
            Entity::Fireball(_) => (64, 10),
            Entity::Snowball(_) => (64, 10),
            Entity::Item(_) => (64, 20),
            Entity::Minecart(_) => (160, 5),
            Entity::Boat(_) => (160, 5),
            Entity::Squid(_) => (160, 3),
            Entity::Chicken(_) |
            Entity::Cow(_) |
            Entity::Sheep(_) |
            Entity::Wolf(_) |
            Entity::Pig(_) => (160, 3),
            Entity::Tnt(_) => (160, 10),
            Entity::FallingBlock(_) => (160, 20),
            Entity::Painting(_) => (160, 0),
            _ => unimplemented!("unsupported entity")
        };

        let entity_base = entity.base();

        let mut tracker = Self {
            entity_id: entity_base.id,
            distance,
            interval,
            forced_countdown_ticks: 0,
            pos: IVec3::ZERO,
            look: IVec2::ZERO,
            sent_pos: IVec3::ZERO,
            sent_look: IVec2::ZERO,
        };
        
        tracker.set_pos(entity_base.pos);
        tracker.set_look(entity_base.look);
        tracker.sent_pos = tracker.pos;
        tracker.sent_look = tracker.look;
        tracker

    } 

    fn set_pos(&mut self, pos: DVec3) {
        self.pos = pos.mul(32.0).floor().as_ivec3();
    }

    fn set_look(&mut self, look: Vec2) {
        self.look = look.div(std::f32::consts::TAU).mul(256.0).as_ivec2();
    }

    /// Update this tracker to determine which move packet to send and to which players.
    fn update_players(&mut self, players: &[ServerPlayer]) {

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
                move_packet = Some(OutPacket::EntityMoveAndLook(proto::EntityMoveAndLookPacket {
                    entity_id: self.entity_id,
                    dx,
                    dy,
                    dz,
                    yaw: self.look.x as i8,
                    pitch: self.look.y as i8,
                }))
            } else if send_pos {
                move_packet = Some(OutPacket::EntityMove(proto::EntityMovePacket {
                    entity_id: self.entity_id,
                    dx,
                    dy,
                    dz,
                }))
            } else if send_look {
                move_packet = Some(OutPacket::EntityLook(proto::EntityLookPacket {
                    entity_id: self.entity_id,
                    yaw: self.look.x as i8,
                    pitch: self.look.y as i8,
                }))
            }

        } else {
            self.forced_countdown_ticks = 0;
            move_packet = Some(OutPacket::EntityPositionAndLook(proto::EntityPositionAndLookPacket {
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
            for player in players {
                if player.tracked_entities.contains(&self.entity_id) {
                    player.send(packet.clone());
                }
            }
        }

    }

    /// Update players to track or untrack this entity. See [`update_tracking_player`].
    fn update_tracking_players(&self, players: &mut [ServerPlayer], world: &World) {
        for player in players {
            self.update_tracking_player(player, world);
        }
    }

    /// Update a player to track or untrack this entity. The correct packet is sent if
    /// the entity needs to appear or disappear on the client side.
    fn update_tracking_player(&self, player: &mut ServerPlayer, world: &World) {
        
        // A player cannot track its own entity.
        if player.entity_id == self.entity_id {
            return;
        }

        let delta = player.pos - self.pos.as_dvec3() / 32.0;
        if delta.x.abs() <= self.distance as f64 && delta.z.abs() <= self.distance as f64 {
            if player.tracked_entities.insert(self.entity_id) {
                self.spawn_player_entity(player, world);
            }
        } else if player.tracked_entities.remove(&self.entity_id) {
            self.kill_player_entity(player);
        }

    }

    /// Force untrack this entity to this player if the player is already tracking it.
    fn untrack_player(&self, player: &mut ServerPlayer) {
        if player.tracked_entities.remove(&self.entity_id) {
            self.kill_player_entity(player);
        }
    }

    /// Force untrack this entity to all given players, it applies only to players that
    /// were already tracking the entity.
    fn untrack_players(&self, players: &mut [ServerPlayer]) {
        for player in players {
            self.untrack_player(player);
        }
    }

    /// Spawn the entity on the player side.
    fn spawn_player_entity(&self, player: &ServerPlayer, world: &World) {

        // NOTE: Silently ignore dead if the entity is dead, it will be killed later.
        let Some(entity) = world.entity(self.entity_id) else { return };
        
        match entity {
            Entity::Player(base) => {
                player.send(OutPacket::PlayerSpawn(proto::PlayerSpawnPacket {
                    entity_id: base.id,
                    username: base.kind.kind.username.clone(),
                    x: self.sent_pos.x, 
                    y: self.sent_pos.y, 
                    z: self.sent_pos.z, 
                    yaw: self.sent_look.x as i8,
                    pitch: self.sent_look.y as i8,
                    current_item: 0, // TODO:
                }));
            }
            Entity::Item(base) => {
                let vel = base.vel.mul(128.0).as_ivec3();
                player.send(OutPacket::ItemSpawn(proto::ItemSpawnPacket { 
                    entity_id: base.id, 
                    item: base.kind.item, 
                    x: self.sent_pos.x, 
                    y: self.sent_pos.y, 
                    z: self.sent_pos.z, 
                    vx: vel.x as i8,
                    vy: vel.y as i8,
                    vz: vel.z as i8,
                }));
            }
            Entity::Pig(base) => {
                player.send(OutPacket::MobSpawn(proto::MobSpawnPacket {
                    entity_id: base.id,
                    kind: 90,
                    x: self.sent_pos.x, 
                    y: self.sent_pos.y, 
                    z: self.sent_pos.z, 
                    yaw: self.sent_look.x as i8,
                    pitch: self.sent_look.y as i8,
                    metadata: Vec::new(), // TODO:
                }));
            }
            _ => unimplemented!("unsupported entity to spawn")
        }

    }

    /// Kill the entity on the player side.
    fn kill_player_entity(&self, player: &ServerPlayer) {
        player.send(OutPacket::EntityKill(proto::EntityKillPacket { 
            entity_id: self.entity_id
        }));
    }

}
