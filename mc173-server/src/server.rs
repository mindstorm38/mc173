//! The network server managing connected players and dispatching incoming packets.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use std::net::SocketAddr;
use std::ops::{Mul, Div};
use std::sync::Arc;
use std::io;

use anyhow::Result as AnyResult;

use glam::{DVec3, Vec2, IVec3};

use mc173::world::{World, Dimension, Event, Weather, BlockEvent, EntityEvent, 
    BlockEntityEvent, BlockEntityStorage, BlockEntityProgress};
    use mc173::world::interact::Interaction;
    use mc173::chunk::{self, Chunk};

use mc173::entity::{self, Entity, PlayerEntity, ItemEntity};
use mc173::block_entity::BlockEntity;
use mc173::item::{self, ItemStack};
use mc173::block;

use mc173::storage::{ChunkStorage, ChunkStorageReply};
use mc173::gen::OverworldGenerator;

use mc173::inventory::InventoryHandle;
use mc173::craft::CraftTracker;
use mc173::path::PathFinder;
use mc173::util::Face;

use crate::proto::{self, Network, NetworkEvent, NetworkClient, InPacket, OutPacket};


/// Target tick duration. Currently 20 TPS, so 50 ms/tick.
const TICK_DURATION: Duration = Duration::from_millis(50);

/// Server world seed is currently hardcoded.
// const SEED: i64 = 3841016456717830250;
const SEED: i64 = 9999;


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
                ServerWorld::new("overworld"),
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
            } else {
                println!("[WARN] Tick was too long ({elapsed:?})");
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
                player.handle(&mut world.world, &mut world.state, packet);
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
                    world: spawn_world.state.name.clone(),
                    pos: spawn_world.world.get_spawn_pos(),
                    look: Vec2::ZERO,
                }
            });

        let (world_index, world) = self.worlds.iter_mut()
            .enumerate()
            .filter(|(_, world)| world.state.name == offline_player.world)
            .next()
            .expect("invalid offline player world name");

        let mut entity = PlayerEntity::default();
        entity.kind.kind.username = packet.username.clone();
        entity.pos = offline_player.pos;
        entity.look = offline_player.look;
        entity.persistent = false;
        entity.can_pickup = true;
        let entity_id = world.world.spawn_entity(Entity::Player(entity));

        // Confirm the login by sending same packet in response.
        self.net.send(client, OutPacket::Login(proto::OutLoginPacket {
            entity_id,
            random_seed: SEED,
            dimension: match world.world.get_dimension() {
                Dimension::Overworld => 0,
                Dimension::Nether => -1,
            },
        }));

        // The standard server sends the spawn position just after login response.
        self.net.send(client, OutPacket::SpawnPosition(proto::SpawnPositionPacket {
            pos: world.world.get_spawn_pos().as_ivec3(),
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
            time: world.world.get_time(),
        }));

        if world.world.get_weather() != Weather::Clear {
            self.net.send(client, OutPacket::Notification(proto::NotificationPacket {
                reason: 1,
            }));
        }

        // Finally insert the player tracker.
        let server_player = ServerPlayer::new(&self.net, client, entity_id, packet.username, &offline_player);
        let player_index = world.handle_player_join(server_player);

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
    /// The inner world data structure.
    world: World,
    /// Players currently in the world.
    players: Vec<ServerPlayer>,
    /// The remaining world state, this is put is a separate struct in order to facilitate
    /// borrowing when handling player packets.
    state: ServerWorldState,
}

/// Represent the whole state of a world.
struct ServerWorldState {
    /// World name.
    name: String,
    /// The server-side time, that is not necessarily in-sync with the world time in case
    /// of tick freeze or stepping. This avoids running in socket timeout issues.
    time: u64,
    /// The chunk source used to load and save the world's chunk.
    storage: ChunkStorage,
    /// A set of chunks that have been modified and needs to be saved at some point. This
    /// includes modification to any chunk, entity or block entity data.
    dirty_chunks: HashSet<(i32, i32)>,
    /// Chunks trackers used to send proper block changes packets.
    chunk_trackers: HashMap<(i32, i32), ChunkTracker>,
    /// Entity tracker, each is associated to the entity id.
    entity_trackers: HashMap<u32, EntityTracker>,
    /// True when world ticking is frozen, events are still processed by the world no 
    /// longer runs.
    tick_mode: TickMode,
    /// Sliding average tick duration, in seconds.
    tick_duration: f32,
    /// Sliding average interval between two ticks.
    tick_interval: f32,
    /// Instant of the last tick.
    tick_last: Instant,
}

/// Indicate the current mode for ticking the world.
enum TickMode {
    /// The world is ticked on each server tick (20 TPS).
    Auto,
    /// The world if ticked on each server tick (20 TPS), but the counter decrease and
    /// it is no longer ticked when reaching 0.
    Manual(u32),
}

impl ServerWorld {

    /// Internal function to create a server world.
    fn new(name: impl Into<String>) -> Self {

        let mut inner = World::new(Dimension::Overworld);
        inner.set_spawn_pos(DVec3::new(0.0, 100.0, 0.0));

        // Make sure that the world initially have an empty events queue.
        inner.swap_events(Some(Vec::new()));

        Self {
            world: inner,
            players: Vec::new(),
            state: ServerWorldState {
                name: name.into(),
                time: 0,
                storage: ChunkStorage::new("test_world/region/", OverworldGenerator::new(SEED), 4),
                dirty_chunks: HashSet::new(),
                chunk_trackers: HashMap::new(),
                entity_trackers: HashMap::new(),
                tick_mode: TickMode::Auto,
                tick_duration: 0.0,
                tick_interval: 0.0,
                tick_last: Instant::now(),
            },
        }

    }

    /// Tick this world.
    fn tick(&mut self) {

        let start = Instant::now();
        self.state.tick_interval = (self.state.tick_interval * 0.98) + (start - self.state.tick_last).as_secs_f32() * 0.02;
        self.state.tick_last = start;

        // Get server-side time.
        let time = self.state.time;
        if time == 0 {
            self.init();
        }

        // Poll all chunks to load in the world.
        while let Some(reply) = self.state.storage.poll() {
            match reply {
                ChunkStorageReply::Load(Ok(snapshot)) => {
                    println!("[STORAGE] Inserting chunk {}/{}", snapshot.cx, snapshot.cz);
                    self.world.insert_chunk_snapshot(snapshot);
                }
                ChunkStorageReply::Load(Err(err)) => {
                    println!("[STORAGE] Error while loading chunk: {err}");
                }
                ChunkStorageReply::Save(Ok((cx, cz))) => {
                    println!("[STORAGE] Saved chunk {cx}/{cz}");
                }
                ChunkStorageReply::Save(Err(err)) => {
                    println!("[STORAGE] Error while saving chunk: {err}");
                }
            }
        }

        // Only run if no tick freeze.
        match self.state.tick_mode {
            TickMode::Auto => {
                self.world.tick()
            }
            TickMode::Manual(0) => {}
            TickMode::Manual(ref mut n) => {
                self.world.tick();
                *n -= 1;
            }
        }

        // Swap events out in order to proceed them.
        let mut events = self.world.swap_events(None).expect("events should be enabled");
        for event in events.drain(..) {
            // println!("[WORLD] Event: {event:?}");
            match event {
                Event::Block { pos, inner } => match inner {
                    BlockEvent::Set { id, metadata, prev_id, prev_metadata } =>
                        self.handle_block_set(pos, id, metadata, prev_id, prev_metadata),
                    BlockEvent::Sound { id, metadata } =>
                        self.handle_block_sound(pos, id, metadata),
                }
                Event::Entity { id, inner } => match inner {
                    EntityEvent::Spawn => 
                        self.handle_entity_spawn(id),
                    EntityEvent::Remove => 
                        self.handle_entity_remove(id),
                    EntityEvent::Position { pos } => 
                        self.handle_entity_position(id, pos),
                    EntityEvent::Look { look } => 
                        self.handle_entity_look(id, look),
                    EntityEvent::Velocity { vel } => 
                        self.handle_entity_velocity(id, vel),
                    EntityEvent::Pickup { target_id } => 
                        self.handle_entity_pickup(id, target_id),
                }
                Event::BlockEntity { pos, inner } => match inner {
                    BlockEntityEvent::Set =>
                        self.handle_block_entity_set(pos),
                    BlockEntityEvent::Remove =>
                        self.handle_block_entity_remove(pos),
                    BlockEntityEvent::Storage { storage, stack } =>
                        self.handle_block_entity_storage(pos, storage, stack),
                    BlockEntityEvent::Progress { progress, value } =>
                        self.handle_block_entity_progress(pos, progress, value),
                }
                Event::SpawnPosition { pos } =>
                    self.handle_spawn_position(pos),
                Event::Weather { new, .. } =>
                    self.handle_weather_change(new),
            }
        }

        // Reinsert events after processing.
        self.world.swap_events(Some(events));

        // Send time to every playing clients every second.
        if time % 20 == 0 {
            let world_time = self.world.get_time();
            for player in &self.players {
                player.send(OutPacket::UpdateTime(proto::UpdateTimePacket {
                    time: world_time,
                }));
            }
        }

        // After we collected every block change, update all players accordingly.
        for (&(cx, cz), tracker) in &mut self.state.chunk_trackers {
            tracker.update_players(cx, cz, &mut self.players, &self.world);
        }

        // After world events are processed, tick entity trackers.
        for tracker in self.state.entity_trackers.values_mut() {

            if time % 60 == 0 {
                tracker.update_tracking_players(&mut self.players, &self.world);
            }

            tracker.forced_countdown_ticks += 1;
            if tracker.interval != 0 && time % tracker.interval as u64 == 0 {
                tracker.update_players(&self.players);
            }

        }

        // Every 10 second, save each modified chunks.
        self.state.dirty_chunks.retain(|&(cx, cz)| {
            // Unwrap should be safe because tracker should exists if chunk is dirty.
            let tracker = self.state.chunk_trackers.get_mut(&(cx, cz)).unwrap();
            // Save the chunk after 2 seconds not being modified OR if th chunk has not 
            // been saved for 10 seconds.
            if time - tracker.dirty_time > 20 || time - tracker.save_time > 200 {
                tracker.save_time = time;
                if let Some(snapshot) = self.world.take_chunk_snapshot(cx, cz) {
                    self.state.storage.request_save(snapshot);
                }
                false
            } else {
                true
            }
        });

        // Update tick duration metric.
        let tick_duration = start.elapsed();
        self.state.tick_duration = (self.state.tick_duration * 0.98) + tick_duration.as_secs_f32() * 0.02;

        // Finally increase server-side tick time.
        self.state.time += 1;

    }
    
    /// Initialize the world by ensuring that every entity is currently tracked. This
    /// method can be called multiple time and should be idempotent.
    fn init(&mut self) {

        // Ensure that every entity has a tracker.
        for (id, entity) in self.world.iter_entities() {
            self.state.entity_trackers.entry(id).or_insert_with(|| {
                let tracker = EntityTracker::new(id, entity);
                tracker.update_tracking_players(&mut self.players, &self.world);
                tracker
            });
        }

        // FIXME: Temporary code.
        for cx in -5..=5 {
            for cz in -5..=5 {
                self.state.storage.request_load(cx, cz);
            }
        }

    }

    /// Internal function ot mark a chunk dirty, in order to be saved later.
    fn mark_chunk_dirty(&mut self, cx: i32, cz: i32) {
        self.state.dirty_chunks.insert((cx, cz));
        let tracker = self.state.chunk_trackers.entry((cx, cz)).or_default();
        tracker.dirty_time = self.state.time;
    }

    /// Handle a player joining this world.
    fn handle_player_join(&mut self, mut player: ServerPlayer) -> usize {

        // Initial tracked entities.
        for tracker in self.state.entity_trackers.values() {
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
        self.world.remove_entity(player.entity_id);

        // If player has not lost connection but it's just leaving the world, we just
        // send it untrack packets.
        if !lost {
            
            // Take and replace it with an empty set (no overhead).
            let tracked_entities = std::mem::take(&mut player.tracked_entities);

            // Untrack all its entities.
            for entity_id in tracked_entities {
                let tracker = self.state.entity_trackers.get(&entity_id).expect("incoherent tracked entity");
                tracker.kill_player_entity(&mut player);
            }

        }

        self.players.get(player_index)

    }

    /// Handle a block change world event.
    fn handle_block_set(&mut self, pos: IVec3, id: u8, metadata: u8, prev_id: u8, _prev_metadata: u8) {
        
        let (cx, cz) = chunk::calc_chunk_pos_unchecked(pos);
        self.mark_chunk_dirty(cx, cz);

        // Ensure that we have a chunk tracker and register the block change in it.
        let chunk_tracker = self.state.chunk_trackers.entry((cx, cz)).or_default();
        chunk_tracker.set_block(pos, id, metadata);

        // If the block was a crafting table, if any player has a crafting table
        // window referencing this block then we force close it.
        let break_crafting_table = id != prev_id && prev_id == block::CRAFTING_TABLE;
        if break_crafting_table {
            for player in &mut self.players {
                if let WindowKind::CraftingTable { pos: check_pos } = player.window.kind {
                    if check_pos == pos {
                        player.close_window(&mut self.world, None, true);
                    }
                }
            }
        }

    }

    fn handle_block_sound(&mut self, pos: IVec3, _block: u8, _metadata: u8) {
        let (cx, cz) = chunk::calc_chunk_pos_unchecked(pos);
        for player in &self.players {
            if player.tracked_chunks.contains(&(cx, cz)) {
                player.send(OutPacket::EffectPlay(proto::EffectPlayPacket {
                    effect_id: 1003,
                    x: pos.x,
                    y: pos.y as i8,
                    z: pos.z,
                    effect_data: 0,
                }));
            }
        }
    }

    /// Handle an entity spawn world event.
    fn handle_entity_spawn(&mut self, id: u32) {
        let entity = self.world.get_entity(id).expect("incoherent event entity");
        self.state.entity_trackers.entry(id).or_insert_with(|| {
            let tracker = EntityTracker::new(id, entity);
            tracker.update_tracking_players(&mut self.players, &self.world);
            tracker
        });
    }

    /// Handle an entity kill world event.
    fn handle_entity_remove(&mut self, id: u32) {
        let tracker = self.state.entity_trackers.remove(&id).expect("incoherent event entity");
        tracker.untrack_players(&mut self.players);
    }

    /// Handle an entity position world event.
    fn handle_entity_position(&mut self, id: u32, pos: DVec3) {
        self.state.entity_trackers.get_mut(&id).unwrap().set_pos(pos);
    }

    /// Handle an entity look world event.
    fn handle_entity_look(&mut self, id: u32, look: Vec2) {
        self.state.entity_trackers.get_mut(&id).unwrap().set_look(look);
    }

    /// Handle an entity look world event.
    fn handle_entity_velocity(&mut self, id: u32, vel: DVec3) {
        self.state.entity_trackers.get_mut(&id).unwrap().set_vel(vel);
    }

    /// Handle an entity pickup world event.
    fn handle_entity_pickup(&mut self, id: u32, target_id: u32) {

        let Some(target_entity) = self.world.get_entity_mut(target_id) else { return };
        let Some(player) = self.players.iter_mut().find(|p| p.entity_id == id) else {
            // This works only on entities handled by players.
            return
        };

        let mut inv = InventoryHandle::new(&mut player.main_inv[..]);

        let remove_target = match target_entity {
            Entity::Item(base) => {
                base.kind.stack.size -= inv.add(base.kind.stack);
                base.kind.stack.size == 0
            }
            Entity::Arrow(_) => {
                inv.add(ItemStack::new_single(item::ARROW, 0)) != 0
            }
            // Other entities cannot be picked up.
            _ => return,
        };

        // Update the associated slots in the player inventory.
        for index in inv.iter_changes() {
            player.send_main_inv_item(index);
        }

        if remove_target {
            self.world.remove_entity(target_id);
        }

        for player in &self.players {
            if player.tracked_entities.contains(&target_id) {
                player.send(OutPacket::EntityPickup(proto::EntityPickupPacket {
                    entity_id: id,
                    picked_entity_id: target_id,
                }));
            }
        }

    }

    /// HAndle a block entity set event.
    fn handle_block_entity_set(&mut self, pos: IVec3) {
        let (cx, cz) = chunk::calc_chunk_pos_unchecked(pos);
        self.mark_chunk_dirty(cx, cz);
    }

    /// Handle a block entity remove event.
    fn handle_block_entity_remove(&mut self, target_pos: IVec3) {

        let (cx, cz) = chunk::calc_chunk_pos_unchecked(target_pos);
        self.mark_chunk_dirty(cx, cz);
        
        // Close the inventory of all entities that had a window opened for this block.
        for player in &mut self.players {

            let contains = match player.window.kind {
                WindowKind::Furnace { pos } |
                WindowKind::Dispenser { pos } => 
                    pos == target_pos,
                WindowKind::Chest { ref pos } => 
                    pos.iter().any(|&pos| pos == target_pos),
                _ => false
            };

            if contains {
                player.close_window(&mut self.world, None, true);
            }

        }

    }

    /// Handle a storage event for a block entity.
    fn handle_block_entity_storage(&mut self, target_pos: IVec3, storage: BlockEntityStorage, stack: ItemStack) {

        let (cx, cz) = chunk::calc_chunk_pos_unchecked(target_pos);
        self.mark_chunk_dirty(cx, cz);

        for player in &mut self.players {
            match player.window.kind {
                WindowKind::Chest { ref pos } => {
                    if let Some(row) = pos.iter().position(|&pos| pos == target_pos) {

                        if let BlockEntityStorage::Standard(index) = storage {
                            player.send(OutPacket::WindowSetItem(proto::WindowSetItemPacket {
                                window_id: player.window.id,
                                slot: row as i16 * 27 + index as i16,
                                stack: stack.to_non_empty(),
                            }));
                        }
                        
                    }
                }
                WindowKind::Furnace { pos } => {
                    if pos == target_pos {

                        let slot = match storage {
                            BlockEntityStorage::FurnaceInput => 0,
                            BlockEntityStorage::FurnaceFuel => 1,
                            BlockEntityStorage::FurnaceOutput => 2,
                            _ => continue,
                        };

                        player.send(OutPacket::WindowSetItem(proto::WindowSetItemPacket {
                            window_id: player.window.id,
                            slot,
                            stack: stack.to_non_empty(),
                        }));

                    }
                }
                WindowKind::Dispenser { pos } => {
                    if pos == target_pos {
                        if let BlockEntityStorage::Standard(index) = storage {

                            player.send(OutPacket::WindowSetItem(proto::WindowSetItemPacket {
                                window_id: player.window.id,
                                slot: index as i16,
                                stack: stack.to_non_empty(),
                            }));

                        }
                    }
                }
                _ => {}  // Not handled.
            }
        }

    }

    fn handle_block_entity_progress(&mut self, target_pos: IVec3, progress: BlockEntityProgress, value: u16) {

        let (cx, cz) = chunk::calc_chunk_pos_unchecked(target_pos);
        self.mark_chunk_dirty(cx, cz);
        
        for player in &mut self.players {
            if let WindowKind::Furnace { pos } = player.window.kind {
                if pos == target_pos {

                    let bar_id = match progress {
                        BlockEntityProgress::FurnaceSmeltTime => 0,
                        BlockEntityProgress::FurnaceBurnRemainingTime => 1,
                        BlockEntityProgress::FurnaceBurnMaxTime => 2,
                    };

                    player.send(OutPacket::WindowProgressBar(proto::WindowProgressBarPacket {
                        window_id: player.window.id,
                        bar_id,
                        value: value as i16,
                    }));

                }
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

    /// Handle weather change in the world.
    fn handle_weather_change(&mut self, weather: Weather) {
        for player in &self.players {
            player.send(OutPacket::Notification(proto::NotificationPacket {
                reason: if weather == Weather::Clear { 2 } else { 1 },
            }));
        }
    }

}

/// A server player is an actual 
struct ServerPlayer {
    /// The network handle for the network server.
    net: Network,
    /// The network client used to send packets through the network to that player.
    client: NetworkClient,
    /// The entity id this player is controlling.
    entity_id: u32, 
    /// The username of that player.
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
    /// The main player inventory including the hotbar in the first 9 slots.
    main_inv: Box<[ItemStack; 36]>,
    /// The armor player inventory.
    armor_inv: Box<[ItemStack; 4]>,
    /// The item stacks for the 3x3 crafting grid. Also support the 2x2 as top left slots.
    craft_inv: Box<[ItemStack; 9]>,
    /// The item stack in the cursor of the client's using a window.
    cursor_stack: ItemStack,
    /// The slot current selected for the hand. Must be in range 0..9.
    hand_slot: u8,
    /// The total number of windows that have been opened by this player, this is also 
    /// used to generate a unique window id. This id should never be zero because it is
    /// reserved for the player inventory.
    window_count: u32,
    /// The current window opened on the client side. Note that the player inventory is
    /// not registered here while opened because we can't know when it is. However we 
    /// know that its window id is always 0.
    window: Window,
    /// This crafting tracker is used to update the current craft being constructed by 
    /// the player in the current window (player inventory or crafting table interface).
    craft_tracker: CraftTracker,
    /// If the player is breaking a block, this record the breaking state.
    breaking_block: Option<BreakingBlock>,
}

/// Describe an opened window and how to handle clicks into it.
#[derive(Debug, Default)]
struct Window {
    /// The unique id of the currently opened window.
    id: u8,
    /// Specialization kind of window.
    kind: WindowKind,
}

/// Describe a kind of opened window on the client side.
#[derive(Debug, Default)]
enum WindowKind {
    /// The player inventory is the default window that is always opened if no other 
    /// window is opened, it also always has the id 0, it contains the armor and craft
    /// matrix.
    #[default]
    Player,
    /// The client-side has a crafting table window opened on the given block pos.
    CraftingTable {
        pos: IVec3,
    },
    /// The client-side has a chest window opened referencing the listed block entities.
    Chest {
        pos: Vec<IVec3>,
    },
    /// The client-side has a furnace window onto the given block entity.
    Furnace {
        pos: IVec3,
    },
    /// The client-side has a dispenser window onto the given block entity.
    Dispenser {
        pos: IVec3,
    }
}

/// State of a player breaking a block.
struct BreakingBlock {
    /// The start time of this block breaking.
    start_time: u64,
    /// The position of the block.
    pos: IVec3,
    /// The block id.
    id: u8,
}

impl ServerPlayer {

    fn new(net: &Network, client: NetworkClient, entity_id: u32, username: String, offline: &OfflinePlayer) -> Self {
        Self {
            net: net.clone(),
            client,
            entity_id,
            username,
            pos: offline.pos,
            look: offline.look,
            tracked_chunks: HashSet::new(),
            tracked_entities: HashSet::new(),
            main_inv: Box::new([ItemStack::EMPTY; 36]),
            armor_inv: Box::new([ItemStack::EMPTY; 4]),
            craft_inv: Box::new([ItemStack::EMPTY; 9]),
            cursor_stack: ItemStack::EMPTY,
            hand_slot: 0,
            window_count: 0,
            window: Window::default(),
            craft_tracker: CraftTracker::default(),
            breaking_block: None,
        }
    }

    /// Send a packet to this player.
    fn send(&self, packet: OutPacket) {
        // println!("[NET] Sending packet {packet:?}");
        self.net.send(self.client, packet);
    }

    /// Send a chat message to this player.
    fn send_chat(&self, message: String) {
        self.send(OutPacket::Chat(proto::ChatPacket { message }));
    }

    /// Handle an incoming packet from this player.
    fn handle(&mut self, world: &mut World, state: &mut ServerWorldState, packet: InPacket) {
        
        match packet {
            InPacket::KeepAlive => {}
            InPacket::Flying(_) => {}, // Ignore because it doesn't update anything.
            InPacket::Disconnect(_) =>
                self.handle_disconnect(),
            InPacket::Chat(packet) =>
                self.handle_chat(world, state, packet.message),
            InPacket::Position(packet) => 
                self.handle_position(world, packet),
            InPacket::Look(packet) => 
                self.handle_look(world, packet),
            InPacket::PositionLook(packet) => 
                self.handle_position_look(world, packet),
            InPacket::BreakBlock(packet) =>
                self.handle_break_block(world, packet),
            InPacket::PlaceBlock(packet) =>
                self.handle_place_block(world, packet),
            InPacket::HandSlot(packet) =>
                self.handle_hand_slot(world, packet.slot),
            InPacket::WindowClick(packet) =>
                self.handle_window_click(world, packet),
            InPacket::WindowClose(packet) =>
                self.handle_window_close(world, packet),
            InPacket::Animation(packet) =>
                self.handle_animation(world, packet),
            _ => println!("[{:?}] Packet: {packet:?}", self.client)
        }

    }

    /// Just disconnect itself, this will produce a lost event from the network.
    fn handle_disconnect(&mut self) {
        self.net.disconnect(self.client);
    }

    /// Handle a chat message packet.
    fn handle_chat(&mut self, world: &mut World, state: &mut ServerWorldState, message: String) {
        if message.starts_with('/') {
            let parts = message.split_whitespace().collect::<Vec<_>>();
            if let Err(message) = self.handle_chat_command(world, state, &parts) {
                self.send_chat(message);
            }
        }
    }

    /// Handle a chat command, parsed from a chat message packet starting with '/'.
    fn handle_chat_command(&mut self, world: &mut World, state: &mut ServerWorldState, parts: &[&str]) -> Result<(), String> {

        match *parts {
            ["/give", item_raw, _] |
            ["/give", item_raw] => {

                let (
                    id_raw, 
                    metadata_raw
                ) = item_raw.split_once(':').unwrap_or((item_raw, ""));

                let id;
                if let Ok(direct_id) = id_raw.parse::<u16>() {
                    id = direct_id;
                } else if let Some(name_id) = item::from_name(id_raw.trim_start_matches("i/")) {
                    id = name_id;
                } else if let Some(block_id) = block::from_name(id_raw.trim_start_matches("b/")) {
                    id = block_id as u16;
                } else {
                    return Err(format!("§cError: unknown item name or id:§r {id_raw}"));
                }

                let item = item::from_id(id);
                if item.name.is_empty() {
                    return Err(format!("§cError: unknown item id:§r {id_raw}"));
                }

                let mut stack = ItemStack::new_sized(id, 0, item.max_stack_size);

                if !metadata_raw.is_empty() {
                    stack.damage = metadata_raw.parse::<u16>()
                        .map_err(|_| format!("§cError: invalid item damage:§r {metadata_raw}"))?;
                }

                if let Some(size_raw) = parts.get(2) {
                    stack.size = size_raw.parse::<u16>()
                        .map_err(|_| format!("§cError: invalid stack size:§r {size_raw}"))?;
                }

                let mut inv = InventoryHandle::new(&mut self.main_inv[..]);
                inv.add(stack);
                for index in inv.iter_changes() {
                    self.send_main_inv_item(index);
                }

                self.send_chat(format!("§aGave §r{}§a (§r{}:{}§a) x§r{}§a to §r{}", item.name, stack.id, stack.damage, stack.size, self.username));
                Ok(())

            }
            ["/give", ..] => Err(format!("§eUsage: /give <item>[:<damage>] [<size>]")),
            ["/spawn", entity_kind, ..] => {

                // Entity spawning params depends on the entity
                let mut entity = match entity_kind {
                    "item" => {
                        let mut item: ItemEntity = Default::default();
                        item.kind.stack = ItemStack::new_block(block::STONE, 0);
                        Entity::Item(item)
                    }
                    "boat" => Entity::Boat(Default::default()),
                    "minecart" => Entity::Minecart(Default::default()),
                    "pig" => Entity::Pig(Default::default()),
                    "chicken" => Entity::Chicken(Default::default()),
                    "cow" => Entity::Cow(Default::default()),
                    "sheep" => Entity::Sheep(Default::default()),
                    _ => return Err(format!("§cError: invalid or unsupported entity kind: {entity_kind}"))
                };

                let base = entity.base_mut();
                base.persistent = true;
                base.pos = self.pos;

                let entity_id = world.spawn_entity(entity);
                self.send_chat(format!("§aEntity spawned:§r {entity_id}"));

                Ok(())

            }
            ["/spawn", ..] => Err(format!("§eUsage: /spawn <entity_kind> [<params>...]")),
            ["/time", ..] => {
                self.send_chat(format!("§aTime:§r {}", world.get_time()));
                Ok(())
            }
            ["/weather", ..] => {
                self.send_chat(format!("§aWeather:§r {:?}", world.get_weather()));
                Ok(())
            }
            ["/pos", ..] => {

                let block_pos = self.pos.floor().as_ivec3();
                self.send_chat(format!("§aPosition information"));
                self.send_chat(format!("§a- Real:§r {}", self.pos));
                self.send_chat(format!("§a- Block:§r {}", block_pos));

                if let Some(height) = world.get_height(block_pos) {
                    self.send_chat(format!("§a- Height:§r {}", height));
                }

                if let Some(light) = world.get_light(block_pos, false) {
                    self.send_chat(format!("§a- Block light:§r {}", light.block));
                    self.send_chat(format!("§a- Sky light:§r {}", light.sky));
                    self.send_chat(format!("§a- Max light:§r {}", light.max));
                }

                if let Some(biome) = world.get_biome(block_pos) {
                    self.send_chat(format!("§a- Biome:§r {biome:?}"));
                }

                Ok(())

            }
            ["/effect", effect_raw] |
            ["/effect", effect_raw, _] => {

                let (effect_id, mut effect_data) = match effect_raw {
                    "click" => (1000, 0),
                    "click2" => (1001, 0),
                    "bow" => (1002, 0),
                    "door" => (1003, 0),
                    "fizz" => (1004, 0),
                    "record_13" => (1005, 2000),
                    "record_cat" => (1005, 2001),
                    "smoke" => (2000, 0),
                    "break" => (2001, 0),
                    _ => {
                        let id = effect_raw.parse::<u32>()
                            .map_err(|_| format!("§cError: invalid effect id:§r {effect_raw}"))?;
                        (id, 0)
                    }
                };

                if let Some(effect_data_raw) = parts.get(2) {
                    effect_data = effect_data_raw.parse::<u32>()
                        .map_err(|_| format!("§cError: invalid effect data:§r {effect_data_raw}"))?;
                }

                let pos = self.pos.floor().as_ivec3();
                self.send(OutPacket::EffectPlay(proto::EffectPlayPacket {
                    x: pos.x,
                    y: pos.y as i8,
                    z: pos.z,
                    effect_id,
                    effect_data,
                }));

                self.send_chat(format!("§aPlayed effect:§r {effect_id}/{effect_data}"));
                Ok(())
                
            }
            ["/effect", ..] => Err(format!("§eUsage: /effect <id> [<data>]")),
            ["/pathfinder", x_raw, y_raw, z_raw] => {

                // This command is used to debug the pathfinder from the player to the
                // given position.

                let from = self.pos.floor().as_ivec3();
                let to = IVec3 {
                    x: x_raw.parse::<i32>().map_err(|_| format!("§cError: invalid x:§r {x_raw}"))?,
                    y: y_raw.parse::<i32>().map_err(|_| format!("§cError: invalid y:§r {y_raw}"))?,
                    z: z_raw.parse::<i32>().map_err(|_| format!("§cError: invalid z:§r {z_raw}"))?,
                };

                if let Some(path) = PathFinder::new(world).find_path(from, to, IVec3::ONE, 20.0) {
                    
                    for pos in path {
                        world.set_block(pos, block::DEAD_BUSH, 0);
                    }

                    Ok(())

                } else {
                    Err(format!("§cError: path not found"))
                }

            }
            ["/pathfinder", ..] => Err(format!("§eUsage: /pathfinder <x> <y> <z>")),
            ["/tick", "freeze"] => {
                self.send_chat(format!("§aWorld ticking:§r freeze"));
                state.tick_mode = TickMode::Manual(0);
                Ok(())
            }
            ["/tick", "auto"] => {
                self.send_chat(format!("§aWorld ticking:§r auto"));
                state.tick_mode = TickMode::Auto;
                Ok(())
            }
            ["/tick", "step"] => {
                self.send_chat(format!("§aWorld ticking:§r step"));
                state.tick_mode = TickMode::Manual(1);
                Ok(())
            }
            ["/tick", "step", step_count] => {

                let step_count = step_count.parse::<u32>()
                    .map_err(|_| format!("§cError: invalid step count:§r {step_count}"))?;

                    self.send_chat(format!("§aWorld ticking:§r {step_count} steps"));
                state.tick_mode = TickMode::Manual(step_count);
                Ok(())

            }
            ["/tick", ..] => Err(format!("§eUsage: /tick [freeze|auto|step <n>]")),
            _ => Err(format!("§eUnknown command!"))
        }
    }

    /// Handle a position packet.
    fn handle_position(&mut self, world: &mut World, packet: proto::PositionPacket) {
        self.handle_position_look_inner(world, Some(packet.pos), None, packet.on_ground);
    }

    /// Handle a look packet.
    fn handle_look(&mut self, world: &mut World, packet: proto::LookPacket) {
        self.handle_position_look_inner(world, None, Some(packet.look), packet.on_ground);
    }

    /// Handle a position and look packet.
    fn handle_position_look(&mut self, world: &mut World, packet: proto::PositionLookPacket) {
        self.handle_position_look_inner(world, Some(packet.pos), Some(packet.look), packet.on_ground);
    }

    fn handle_position_look_inner(&mut self, world: &mut World, pos: Option<DVec3>, look: Option<Vec2>, on_ground: bool) {

        let entity = world.get_entity_mut(self.entity_id).expect("incoherent player entity");
        let entity_base = entity.base_mut();
        entity_base.on_ground = on_ground;

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

        if pos.is_some() {
            self.update_chunks(world);
        }

    }

    /// Handle a break block packet.
    fn handle_break_block(&mut self, world: &mut World, packet: proto::BreakBlockPacket) {
        
        let Some(Entity::Player(base)) = world.get_entity_mut(self.entity_id) else { return };
        let pos = IVec3::new(packet.x, packet.y as i32, packet.z);

        let in_water = base.in_water;
        let on_ground = base.on_ground;
        let mut stack = self.main_inv[self.hand_slot as usize];

        if packet.status == 0 {

            // We ignore any interaction result for the left click (break block) to
            // avoid opening an inventory when breaking a container.
            // NOTE: Interact before 'get_block': relevant for redstone_ore lit.
            world.interact_block(pos);

            // Start breaking a block, ignore if the position is invalid.
            if let Some((id, _)) = world.get_block(pos) {
                
                let break_duration = world.get_break_duration(stack.id, id, in_water, on_ground);
                if break_duration.is_infinite() {
                    // Do nothing, the block is unbreakable.
                } else if break_duration == 0.0 {
                    world.break_block(pos);
                } else {
                    self.breaking_block = Some(BreakingBlock {
                        start_time: world.get_time(), // + (break_duration * 0.7) as u64,
                        pos,
                        id,
                    });
                }

            }

        } else if packet.status == 2 {
            // Block breaking should be finished.
            if let Some(state) = self.breaking_block.take() {
                if state.pos == pos && world.is_block(pos, state.id) {
                    let break_duration = world.get_break_duration(stack.id, state.id, in_water, on_ground);
                    let min_time = state.start_time + (break_duration * 0.7) as u64;
                    if world.get_time() >= min_time {
                        world.break_block(pos);
                    } else {
                        println!("[WARN] Incoherent break (too early), expected {min_time}, got {}", world.get_time());
                    }
                } else {
                    println!("[WARN] Incoherent break (position), expected {}, got {}", pos, state.pos);
                }
            }
        } else if packet.status == 4 {
            // Drop the selected item.

            if !stack.is_empty() {
                
                stack.size -= 1;
                self.main_inv[self.hand_slot as usize] = stack.to_non_empty().unwrap_or_default();
                
                self.send(OutPacket::WindowSetItem(proto::WindowSetItemPacket {
                    window_id: 0,
                    slot: 36 + self.hand_slot as i16,
                    stack: stack.to_non_empty(),
                }));

                self.drop_stack(world, stack.with_size(1), false);

            }

        }

    }

    /// Handle a place block packet.
    fn handle_place_block(&mut self, world: &mut World, packet: proto::PlaceBlockPacket) {
        
        let face = match packet.direction {
            0 => Some(Face::NegY),
            1 => Some(Face::PosY),
            2 => Some(Face::NegZ),
            3 => Some(Face::PosZ),
            4 => Some(Face::NegX),
            5 => Some(Face::PosX),
            0xFF => None,
            _ => return,
        };

        let pos = IVec3 {
            x: packet.x,
            y: packet.y as i32,
            z: packet.z,
        };

        let mut new_hand_stack = None;

        // Check if the player is reasonably near the block.
        if face.is_none() || self.pos.distance_squared(pos.as_dvec3() + 0.5) < 64.0 {
            let hand_stack = self.main_inv[self.hand_slot as usize];
            // The real action depends on 
            if let Some(face) = face {
                match world.interact_block(pos) {
                    Interaction::None => {
                        // No interaction, use the item at that block.
                        new_hand_stack = item::using::use_at(world, pos, face, self.entity_id, hand_stack);
                    }
                    Interaction::CraftingTable { pos } => {
                        self.open_window(world, WindowKind::CraftingTable { pos });
                    }
                    Interaction::Chest { pos } => {
                        self.open_window(world, WindowKind::Chest { pos });
                    }
                    Interaction::Furnace { pos } => {
                        self.open_window(world, WindowKind::Furnace { pos });
                    }
                    Interaction::Dispenser { pos } => {
                        self.open_window(world, WindowKind::Dispenser { pos });
                    }
                    Interaction::Handled => {}
                }
            } else {
                new_hand_stack = item::using::use_raw(world, self.entity_id, hand_stack);
            }
        }

        if let Some(hand_stack) = new_hand_stack {
            self.main_inv[self.hand_slot as usize] = hand_stack;
            self.send_main_inv_item(self.hand_slot as usize);
        }

    }

    /// Handle a hand slot packet.
    fn handle_hand_slot(&mut self, _world: &mut World, slot: i16) {
        if slot >= 0 && slot < 9 {
            self.hand_slot = slot as u8;
        } else {
            println!("[WARN] Invalid hand slot: {slot}");
        }
    }

    /// Handle a window click packet.
    fn handle_window_click(&mut self, world: &mut World, packet: proto::WindowClickPacket) {

        // Holding the target slot's item stack.
        let mut cursor_stack = self.cursor_stack;
        let slot_stack;

        if packet.slot == -999 {
            slot_stack = ItemStack::EMPTY;
            if !cursor_stack.is_empty() {

                let mut drop_stack = cursor_stack;
                if packet.right_click {
                    drop_stack = drop_stack.with_size(1);
                }

                cursor_stack.size -= drop_stack.size;
                self.drop_stack(world, drop_stack, false);

            }
        } else if packet.shift_click {
            todo!()
        } else {

            let slot_handle = self.make_window_slot_handle(world, packet.window_id, packet.slot);
            let Some(mut slot_handle) = slot_handle else {
                println!("[WARN] Cannot find a handle for slot {} in window {}", packet.slot, packet.window_id);
                return;
            };

            slot_stack = slot_handle.get_stack();
            let slot_access = slot_handle.get_access();

            if slot_stack.is_empty() {
                if !cursor_stack.is_empty() && slot_access.can_drop(cursor_stack) {
                    
                    let drop_size = if packet.right_click { 1 } else { cursor_stack.size };
                    let drop_size = drop_size.min(slot_handle.max_stack_size());
                    
                    slot_handle.set_stack(cursor_stack.with_size(drop_size));
                    cursor_stack.size -= drop_size;

                }
            } else if cursor_stack.is_empty() {

                // Here the slot is not empty, but the cursor is.
                
                // NOTE: Splitting is equivalent of taking and then drop (half), we check 
                // if the slot would accept that drop by checking validity.
                cursor_stack = slot_stack;
                if packet.right_click && slot_access.can_drop(cursor_stack) {
                    cursor_stack.size = (cursor_stack.size + 1) / 2;
                }

                let mut new_slot_stack = slot_stack;
                new_slot_stack.size -= cursor_stack.size;
                if new_slot_stack.size == 0 {
                    slot_handle.set_stack(ItemStack::EMPTY);
                } else {
                    slot_handle.set_stack(new_slot_stack);
                }

            } else if slot_access.can_drop(cursor_stack) {

                // Here the slot and the cursor are not empty, we check if we can
                // drop some item if compatible, or swap if not.

                let cursor_item = item::from_id(cursor_stack.id);

                if (slot_stack.id, slot_stack.damage) != (cursor_stack.id, cursor_stack.damage) {
                    // Not the same item, we just swap with hand.
                    if cursor_stack.size <= slot_handle.max_stack_size() {
                        slot_handle.set_stack(cursor_stack);
                        cursor_stack = slot_stack;
                    }
                } else {
                    // Same item, just drop some into the existing stack.
                    let max_stack_size = cursor_item.max_stack_size.min(slot_handle.max_stack_size());
                    // Only drop if the stack is not full.
                    if slot_stack.size < max_stack_size {
                        
                        let drop_size = if packet.right_click { 1 } else { cursor_stack.size };
                        let drop_size = drop_size.min(max_stack_size - slot_stack.size);
                        cursor_stack.size -= drop_size;

                        let mut new_slot_stack = slot_stack;
                        new_slot_stack.size += drop_size;
                        slot_handle.set_stack(new_slot_stack);

                    }
                }

            } else if let SlotAccess::Pickup(min_size) = slot_access {

                // This last case is when the slot and the cursor are not empty, but we
                // can't drop the cursor into the slot, in such case we try to pick item.

                if (slot_stack.id, slot_stack.damage) == (cursor_stack.id, cursor_stack.damage) {
                    let cursor_item = item::from_id(cursor_stack.id);
                    if cursor_stack.size < cursor_item.max_stack_size {
                        let available_size = cursor_item.max_stack_size - cursor_stack.size;
                        if available_size >= min_size {
                            let pick_size = slot_stack.size.min(available_size);
                            cursor_stack.size += pick_size;
                            let new_slot_stack = slot_stack.with_size(slot_stack.size - pick_size);
                            slot_handle.set_stack(new_slot_stack.to_non_empty().unwrap_or_default());
                        }
                    }
                }

            }

            // Handle notification if the slot has changed.
            match slot_handle.notify {
                SlotNotify::Craft { 
                    mapping, 
                    modified: true,
                } => {

                    self.craft_tracker.update(&self.craft_inv);
                    
                    self.net.send(self.client, OutPacket::WindowSetItem(proto::WindowSetItemPacket {
                        window_id: packet.window_id,
                        slot: 0,
                        stack: self.craft_tracker.recipe(),
                    }));

                    if let Some(mapping) = mapping {
                        for (index, &slot) in mapping.iter().enumerate() {
                            if slot >= 0 {
                                self.net.send(self.client, OutPacket::WindowSetItem(proto::WindowSetItemPacket {
                                    window_id: packet.window_id,
                                    slot,
                                    stack: self.craft_inv[index].to_non_empty(),
                                }));
                            }
                        }
                    }

                }
                SlotNotify::BlockEntityStorageEvent { 
                    pos,
                    storage, 
                    stack: Some(stack),
                } => {
                    world.push_event(Event::BlockEntity { 
                        pos, 
                        inner: BlockEntityEvent::Storage { 
                            storage, 
                            stack,
                        },
                    });
                }
                _ => {}
            }

        }
            
        // Answer with a transaction packet that is accepted if the packet's stack is
        // the same as the server's slot stack.
        self.send(OutPacket::WindowTransaction(proto::WindowTransactionPacket {
            window_id: packet.window_id,
            transaction_id: packet.transaction_id,
            accepted: slot_stack.to_non_empty() == packet.stack,
        }));

        // Send the new cursor item.
        if cursor_stack.size == 0 {
            cursor_stack = ItemStack::EMPTY;
        }

        self.send(OutPacket::WindowSetItem(proto::WindowSetItemPacket { 
            window_id: 0xFF,
            slot: -1,
            stack: cursor_stack.to_non_empty(),
        }));

        self.cursor_stack = cursor_stack;

    }

    /// Handle a window close packet, it just forget the current window.
    fn handle_window_close(&mut self, world: &mut World, packet: proto::WindowClosePacket) {
        self.close_window(world, Some(packet.window_id), false);
    }

    fn handle_animation(&mut self, _world: &mut World, _packet: proto::AnimationPacket) {
        // TODO: Send animation to other players.
    }

    /// Open the given window kind on client-side by sending appropriate packet. A new
    /// window id is automatically associated to that window.
    fn open_window(&mut self, world: &mut World, kind: WindowKind) {
        
        // Close any already opened window.
        self.close_window(world, None, true);

        // NOTE: We should never get a window id of 0 because it is the player inventory.
        let window_id = (self.window_count % 100 + 1) as u8;
        self.window_count += 1;
        
        match kind {
            WindowKind::Player => panic!("cannot force open the player window"),
            WindowKind::CraftingTable { .. } => {
                self.send(OutPacket::WindowOpen(proto::WindowOpenPacket {
                    window_id,
                    inventory_type: 1,
                    title: "Crafting".to_string(),
                    slots_count: 9,
                }));
            }
            WindowKind::Chest { ref pos } => {

                self.send(OutPacket::WindowOpen(proto::WindowOpenPacket {
                    window_id,
                    inventory_type: 0,
                    title: if pos.len() <= 1 { "Chest" } else { "Large Chest" }.to_string(),
                    slots_count: (pos.len() * 27) as u8,  // TODO: Checked cast
                }));

                let mut stacks = Vec::new();

                for &pos in pos {
                    if let Some(BlockEntity::Chest(chest)) = world.get_block_entity(pos) {
                        stacks.extend(chest.inv.iter().map(|stack| stack.to_non_empty()));
                    } else {
                        stacks.extend(std::iter::repeat(None).take(27));
                    }
                }

                self.send(OutPacket::WindowItems(proto::WindowItemsPacket {
                    window_id,
                    stacks,
                }));

            }
            WindowKind::Furnace { pos } => {

                self.send(OutPacket::WindowOpen(proto::WindowOpenPacket {
                    window_id,
                    inventory_type: 2,
                    title: format!("Furnace"),
                    slots_count: 3,
                }));
                
                if let Some(BlockEntity::Furnace(furnace)) = world.get_block_entity(pos) {

                    self.send(OutPacket::WindowProgressBar(proto::WindowProgressBarPacket {
                        window_id,
                        bar_id: 0,
                        value: furnace.smelt_ticks as i16,
                    }));

                    self.send(OutPacket::WindowProgressBar(proto::WindowProgressBarPacket {
                        window_id,
                        bar_id: 1,
                        value: furnace.burn_remaining_ticks as i16,
                    }));

                    self.send(OutPacket::WindowProgressBar(proto::WindowProgressBarPacket {
                        window_id,
                        bar_id: 2,
                        value: furnace.burn_max_ticks as i16,
                    }));
    
                    self.send(OutPacket::WindowItems(proto::WindowItemsPacket {
                        window_id,
                        stacks: vec![
                            furnace.input_stack.to_non_empty(),
                            furnace.fuel_stack.to_non_empty(),
                            furnace.output_stack.to_non_empty()
                        ],
                    }));

                }

            }
            WindowKind::Dispenser { pos } => {

                self.send(OutPacket::WindowOpen(proto::WindowOpenPacket {
                    window_id,
                    inventory_type: 3,
                    title: format!("Dispenser"),
                    slots_count: 9,
                }));

                if let Some(BlockEntity::Dispenser(dispenser)) = world.get_block_entity(pos) {
                    self.send(OutPacket::WindowItems(proto::WindowItemsPacket {
                        window_id,
                        stacks: dispenser.inv.iter().map(|stack| stack.to_non_empty()).collect()
                    }));
                }

            }
        };

        self.window.id = window_id;
        self.window.kind = kind;

    }

    /// Close the current window opened by the player. If the window id argument is 
    /// provided, then this will only work if the current server-side window is matching.
    /// The send boolean indicates if a window close packet must also be sent.
    fn close_window(&mut self, world: &mut World, window_id: Option<u8>, send: bool) {
    
        if let Some(window_id) = window_id {
            if self.window.id != window_id {
                return;
            }
        }

        // For any closed inventory, we drop the cursor stack and crafting matrix.
        let mut drop_stacks = Vec::new();
        drop_stacks.extend(self.cursor_stack.take_non_empty());
        for stack in self.craft_inv.iter_mut() {
            drop_stacks.extend(stack.take_non_empty());
        }

        for drop_stack in drop_stacks {
            self.drop_stack(world, drop_stack, false);
        }

        // Closing the player inventory so we clear the crafting matrix.
        if self.window.id == 0 {
            for slot in 1..=4 {
                self.send(OutPacket::WindowSetItem(proto::WindowSetItemPacket { 
                    window_id: 0,
                    slot,
                    stack: None,
                }));
            }
        }

        // Reset to the default window.
        self.window.id = 0;
        self.window.kind = WindowKind::Player;

        if send {
            self.send(OutPacket::WindowClose(proto::WindowClosePacket {
                window_id: self.window.id,
            }));
        }

    }

    /// Internal function to create a window slot handle specifically for a player main
    /// inventory slot, the offset of the first player inventory slot is also given.
    fn make_player_window_slot_handle(&mut self, slot: i16, offset: i16) -> Option<SlotHandle<'_>> {

        let index = match slot - offset {
            0..=26 => slot - offset + 9,
            27..=35 => slot - offset - 27,
            _ => return None,
        } as usize;

        Some(SlotHandle {
            kind: SlotKind::Standard { 
                stack: &mut self.main_inv[index],
                access: SlotAccess::PickupDrop, 
                max_size: 64,
            },
            notify: SlotNotify::None
        })

    }

    /// Internal function to create a window slot handle. This handle is temporary and
    /// own two mutable reference to the player itself and the world, it can only work
    /// on the given slot.
    fn make_window_slot_handle<'a>(&'a mut self, world: &'a mut World, window_id: u8, slot: i16) -> Option<SlotHandle<'a>> {

        // Check coherency of server/client windows.
        if self.window.id != window_id {
            println!("[WARN] Incoherent window id, expected {}, got {} from client", self.window.id, window_id);
            return None;
        }

        // This avoid temporary cast issues afterward, even if we keep the signed type.
        if slot < 0 {
            println!("[WARN] Negative slot {slot} received for window {window_id}");
            return None;
        }

        Some(match self.window.kind {
            WindowKind::Player => {
                match slot {
                    0 => SlotHandle {
                        kind: SlotKind::CraftingResult { 
                            craft_inv: &mut self.craft_inv, 
                            craft_tracker: &mut self.craft_tracker,
                        },
                        notify: SlotNotify::Craft { 
                            mapping: Some(&[1, 2, -1, 3, 4, -1, -1, -1, -1]),
                            modified: false,
                        },
                    },
                    1..=4 => SlotHandle { 
                        kind: SlotKind::Standard { 
                            stack: &mut self.craft_inv[match slot {
                                1 => 0,
                                2 => 1,
                                3 => 3,
                                4 => 4,
                                _ => unreachable!()
                            }], 
                            access: SlotAccess::PickupDrop,
                            max_size: 64,
                        },
                        notify: SlotNotify::Craft {
                            mapping: None,
                            modified: false,
                        },
                    },
                    5..=8 => SlotHandle { 
                        kind: SlotKind::Standard { 
                            stack: &mut self.armor_inv[slot as usize - 5], 
                            access: match slot {
                                5 => SlotAccess::ArmorHelmet,
                                6 => SlotAccess::ArmorChestplate,
                                7 => SlotAccess::ArmorLeggings,
                                8 => SlotAccess::ArmorBoots,
                                _ => unreachable!(),
                            }, max_size: 1,
                        }, 
                        notify: SlotNotify::None,
                    },
                    _ => self.make_player_window_slot_handle(slot, 9)?
                }
            }
            WindowKind::CraftingTable { .. } => {
                match slot {
                    0 => SlotHandle {
                        kind: SlotKind::CraftingResult { 
                            craft_inv: &mut self.craft_inv, 
                            craft_tracker: &mut self.craft_tracker,
                        },
                        notify: SlotNotify::Craft {
                            mapping: Some(&[1, 2, 3, 4, 5, 6, 7, 8, 9]),
                            modified: false,
                        },
                    },
                    1..=9 => SlotHandle { 
                        kind: SlotKind::Standard { 
                            stack: &mut self.craft_inv[slot as usize - 1], 
                            access: SlotAccess::PickupDrop,
                            max_size: 64,
                        },
                        notify: SlotNotify::Craft {
                            mapping: None,
                            modified: false,
                        },
                    },
                    _ => self.make_player_window_slot_handle(slot, 10)?
                }
            }
            WindowKind::Chest { ref pos } => {

                let block_entity_index = slot as usize / 27;
                if let Some(&pos) = pos.get(block_entity_index) {
                    
                    // Get the chest tile entity corresponding to the clicked slot,
                    // if not found we just ignore.
                    let Some(BlockEntity::Chest(chest)) = world.get_block_entity_mut(pos) else {
                        return None
                    };

                    let index = slot as usize % 27;

                    SlotHandle {
                        kind: SlotKind::Standard { 
                            stack: &mut chest.inv[index],
                            access: SlotAccess::PickupDrop,
                            max_size: 64,
                        },
                        notify: SlotNotify::BlockEntityStorageEvent {
                            pos,
                            storage: BlockEntityStorage::Standard(index as u8),
                            stack: None,
                        },
                    }

                } else {
                    self.make_player_window_slot_handle(slot, pos.len() as i16 * 27)?
                }

            }
            WindowKind::Furnace { pos } => {

                if slot <= 2 {

                    let Some(BlockEntity::Furnace(furnace)) = world.get_block_entity_mut(pos) else {
                        return None
                    };

                    let (stack, access, storage) = match slot {
                        0 => (&mut furnace.input_stack, SlotAccess::PickupDrop, BlockEntityStorage::FurnaceInput),
                        1 => (&mut furnace.fuel_stack, SlotAccess::PickupDrop, BlockEntityStorage::FurnaceFuel),
                        2 => (&mut furnace.output_stack, SlotAccess::Pickup(1), BlockEntityStorage::FurnaceOutput),
                        _ => unreachable!()
                    };

                    SlotHandle {
                        kind: SlotKind::Standard { 
                            stack,
                            access, 
                            max_size: 64,
                        },
                        notify: SlotNotify::BlockEntityStorageEvent { 
                            pos, 
                            storage, 
                            stack: None,
                        },
                    }

                } else {
                    self.make_player_window_slot_handle(slot, 3)?
                }

            }
            WindowKind::Dispenser { pos } => {

                if slot < 9 {

                    let Some(BlockEntity::Dispenser(dispenser)) = world.get_block_entity_mut(pos) else {
                        return None
                    };

                    SlotHandle {
                        kind: SlotKind::Standard { 
                            stack: &mut dispenser.inv[slot as usize], 
                            access: SlotAccess::PickupDrop,
                            max_size: 64,
                        },
                        notify: SlotNotify::BlockEntityStorageEvent { 
                            pos, 
                            storage: BlockEntityStorage::Standard(slot as u8), 
                            stack: None,
                        },
                    }

                } else {
                    self.make_player_window_slot_handle(slot, 9)?
                }

            }
        })

    }

    /// Send the main inventory item at given index to the client.
    fn send_main_inv_item(&self, index: usize) {

        let slot = match index {
            0..=8 => 36 + index,
            _ => index,
        };

        let stack = self.main_inv[index];

        self.send(OutPacket::WindowSetItem(proto::WindowSetItemPacket {
            window_id: 0,
            slot: slot as i16,
            stack: stack.to_non_empty(),
        }));

    }

    /// Drop an item from the player's entity, items are drop in front of the player, but
    /// the `on_ground` argument can be set to true in order to drop item on the ground.
    fn drop_stack(&mut self, world: &mut World, stack: ItemStack, on_ground: bool) {

        let entity = world.get_entity_mut(self.entity_id).expect("incoherent player entity");
        let base = entity.base_mut();

        let mut item_entity = ItemEntity::default();
        item_entity.persistent = true;
        item_entity.pos = base.pos;
        item_entity.pos.y += 1.3;  // TODO: Adjust depending on eye height.

        if on_ground {

            let rand_drop_speed = base.rand.next_float() * 0.5;
            let rand_yaw = base.rand.next_float() * std::f32::consts::TAU;

            item_entity.vel.x = (rand_yaw.sin() * rand_drop_speed) as f64;
            item_entity.vel.z = (rand_yaw.cos() * rand_drop_speed) as f64;
            item_entity.vel.y = 0.2;

        } else {

            let drop_speed = 0.3;
            let rand_yaw = base.rand.next_float() * std::f32::consts::TAU;
            let rand_drop_speed = base.rand.next_float() * 0.02;
            let rand_vel_y = (base.rand.next_float() - base.rand.next_float()) * 0.1;

            item_entity.vel.x = (-base.look.x.sin() * base.look.y.cos() * drop_speed) as f64;
            item_entity.vel.z = (base.look.x.cos() * base.look.y.cos() * drop_speed) as f64;
            item_entity.vel.y = (-base.look.y.sin() * drop_speed + 0.1) as f64;
            item_entity.vel.x += (rand_yaw.cos() * rand_drop_speed) as f64;
            item_entity.vel.z += (rand_yaw.sin() * rand_drop_speed) as f64;
            item_entity.vel.y += rand_vel_y as f64;

        }

        item_entity.kind.frozen_ticks = 40;
        item_entity.kind.stack = stack;
        
        world.spawn_entity(Entity::Item(item_entity));

    }

    /// Update the chunks sent to this player.
    fn update_chunks(&mut self, world: &mut World) {

        let (ocx, ocz) = chunk::calc_entity_chunk_pos(self.pos);
        let view_range = 3;

        for cx in (ocx - view_range)..(ocx + view_range) {
            for cz in (ocz - view_range)..(ocz + view_range) {

                if let Some(chunk) = world.get_chunk(cx, cz) {
                    if self.tracked_chunks.insert((cx, cz)) {

                        self.send(OutPacket::ChunkState(proto::ChunkStatePacket {
                            cx, cz, init: true
                        }));

                        let from = IVec3 {
                            x: cx * 16,
                            y: 0,
                            z: cz * 16,
                        };

                        let size = IVec3 { 
                            x: 16, 
                            y: 128, 
                            z: 16,
                        };

                        self.send(OutPacket::ChunkData(new_chunk_data_packet(chunk, from, size)));

                    }
                }

            }
        }

    }

}

/// This structure tracks a chunk and record every block set in the chunk, this is used
/// to track blocks being set.
#[derive(Debug, Default)]
struct ChunkTracker {
    /// A list of block set in this chunk, if the number of set blocks go above a given
    /// threshold, the vector can be cleared and `set_blocks_full` set to true in order
    /// to resend only the modified range.
    set_blocks: Vec<ChunkSetBlock>,
    /// Set to true when the whole chunk area can be resent instead of all blocks one by
    /// one.
    set_blocks_full: bool,
    /// The minimum position where blocks have been set in the chunk (inclusive).
    set_blocks_min: ChunkLocalPos,
    /// The maximum position where blocks have been set in the chunk (inclusive).
    set_blocks_max: ChunkLocalPos,
    /// The last time the chunk was marked dirty, this is kept even after save.
    dirty_time: u64,
    /// Last save time of this chunk.
    save_time: u64,
}

/// A position structure to store chunk-local coordinates to save space.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
struct ChunkLocalPos {
    x: u8,
    y: u8,
    z: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ChunkSetBlock {
    pos: ChunkLocalPos,
    block: u8,
    metadata: u8,
}

impl ChunkTracker {

    /// Internally register the given set block, depending on the internal state the 
    /// change may be discarded and the whole modified area may be resent instead.
    fn set_block(&mut self, pos: IVec3, block: u8, metadata: u8) {

        // This is the Notchian implementation threshold.
        const FULL_THRESHOLD: usize = 10;

        let pos = ChunkLocalPos {
            x: (pos.x as u32 & 0b1111) as u8,
            y: (pos.y as u32 & 0b1111111) as u8,
            z: (pos.z as u32 & 0b1111) as u8,
        };

        if !self.set_blocks_full {
            // If the number of set blocks go above a threshold, then we abort and set
            // the full state.
            if self.set_blocks.len() >= FULL_THRESHOLD {
                self.set_blocks_full = true;
                self.set_blocks.clear(); // Can be cleared because useless now.
            } else {
                self.set_blocks.push(ChunkSetBlock { pos, block, metadata });
                // If the list was previously empty, we set min/max to initial pos.
                if self.set_blocks.len() == 1 {
                    self.set_blocks_min = pos;
                    self.set_blocks_max = pos;
                    return;
                }
            }
        }

        self.set_blocks_min.x = self.set_blocks_min.x.min(pos.x);
        self.set_blocks_min.y = self.set_blocks_min.y.min(pos.y);
        self.set_blocks_min.z = self.set_blocks_min.z.min(pos.z);
        
        self.set_blocks_max.x = self.set_blocks_max.x.max(pos.x);
        self.set_blocks_max.y = self.set_blocks_max.y.max(pos.y);
        self.set_blocks_max.z = self.set_blocks_max.z.max(pos.z);

    }

    /// Update the given players by sending them the correct packets to update the player
    /// client side. If the chunk is full of set blocks then the whole area is resent, 
    /// else only individual changes are sent to the players loading the chunk.
    /// 
    /// Once this function has updated all players, all modifications are forgot.
    fn update_players(&mut self, cx: i32, cz: i32, players: &mut [ServerPlayer], world: &World) {

        if self.set_blocks_full {

            let chunk = world.get_chunk(cx, cz).expect("chunk has been removed");
            
            let from = IVec3 { 
                x: cx * 16 + self.set_blocks_min.x as i32, 
                y: self.set_blocks_min.y as i32, 
                z: cz * 16 + self.set_blocks_min.z as i32,
            };

            let size = IVec3 { 
                x: (self.set_blocks_max.x - self.set_blocks_min.x + 1) as i32, 
                y: (self.set_blocks_max.y - self.set_blocks_min.y + 1) as i32, 
                z: (self.set_blocks_max.z - self.set_blocks_min.z + 1) as i32, 
            };

            let packet = OutPacket::ChunkData(new_chunk_data_packet(chunk, from, size));

            // println!("sending chunk data for {cx}/{cz}");
            for player in players {
                if player.tracked_chunks.contains(&(cx, cz)) {
                    player.send(packet.clone());
                }
            }

        } else if self.set_blocks.len() == 1 {

            let set_block = self.set_blocks[0];

            // println!("sending single block change for {cx}/{cz}");
            for player in players {
                if player.tracked_chunks.contains(&(cx, cz)) {
                    player.send(OutPacket::BlockSet(proto::BlockSetPacket {
                        x: cx * 16 + set_block.pos.x as i32,
                        y: set_block.pos.y as i8,
                        z: cz * 16 + set_block.pos.z as i32,
                        block: set_block.block,
                        metadata: set_block.metadata,
                    }));
                }
            }

        } else if !self.set_blocks.is_empty() {

            let set_blocks = self.set_blocks.iter()
                .map(|set_block| proto::ChunkBlockSet {
                    x: set_block.pos.x,
                    y: set_block.pos.y,
                    z: set_block.pos.z,
                    block: set_block.block,
                    metadata: set_block.metadata,
                })
                .collect();

            let packet = OutPacket::ChunkBlockSet(proto::ChunkBlockSetPacket {
                cx,
                cz,
                blocks: Arc::new(set_blocks),
            });

            // println!("sending multi block change for {cx}/{cz} ({})", self.set_blocks.len());

            for player in players {
                if player.tracked_chunks.contains(&(cx, cz)) {
                    player.send(packet.clone());
                }
            }

        }

        self.set_blocks_full = false;
        self.set_blocks.clear();

    }

}

/// This structure tracks every entity spawned in the world and save their previous 
/// position/look (and motion for some entities). It handle allows sending the right
/// packets to the right players when these properties are changed.
#[derive(Debug)]
struct EntityTracker {
    /// The entity id.
    id: u32,
    /// Maximum tracking distance for this type of entity.
    distance: u16,
    /// Update interval for this type of entity.
    interval: u16,
    /// This countdown is reset when the absolute position is sent, if the absolute 
    /// position has not been sent for 400 ticks (20 seconds), it's sent.
    forced_countdown_ticks: u16,
    /// Last known position of the entity.
    pos: (i32, i32, i32),
    /// Last known look of the entity.
    look: (i8, i8),
    /// Last encoded position sent to clients.
    sent_pos: (i32, i32, i32),
    /// Last encoded look sent to clients.
    sent_look: (i8, i8),
    /// If this tracker should track entity velocity, this contains the tracker.
    vel: Option<EntityVelocityTracker>,
}

/// Some entity velocity tracking if enabled for that entity.
#[derive(Debug)]
struct EntityVelocityTracker {
    /// Last known velocity of the entity.
    vel: (i16, i16, i16),
    /// Last encoded velocity sent to clients.
    sent_vel: (i16, i16, i16),
}

impl EntityTracker {

    fn new(id: u32, entity: &Entity) -> Self {

        let (distance, interval, velocity) = match entity {
            Entity::Player(_) => (512, 2, false),
            Entity::Fish(_) => (64, 5, true),
            Entity::Arrow(_) => (64, 20, false),
            Entity::Fireball(_) => (64, 10, false),
            Entity::Snowball(_) => (64, 10, true),
            Entity::Egg(_) => (64, 10, true),
            Entity::Item(_) => (64, 5, true), // Notchian use 20 ticks
            Entity::Minecart(_) => (160, 5, true),
            Entity::Boat(_) => (160, 5, true),
            Entity::Squid(_) => (160, 3, true),
            Entity::Tnt(_) => (160, 10, true),
            Entity::FallingBlock(_) => (160, 20, true),
            Entity::Painting(_) => (160, 0, false),
            // All remaining animals and mobs.
            _ => (160, 3, true)
        };

        let entity_base = entity.base();
        let mut tracker = Self {
            id,
            distance,
            interval,
            forced_countdown_ticks: 0,
            pos: (0, 0, 0),
            look: (0, 0),
            sent_pos: (0, 0, 0),
            sent_look: (0, 0),
            vel: velocity.then_some(EntityVelocityTracker { 
                vel: (0, 0, 0),
                sent_vel: (0, 0, 0),
            }),
        };
        
        tracker.set_pos(entity_base.pos);
        tracker.set_look(entity_base.look);
        tracker.sent_pos = tracker.pos;
        tracker.sent_look = tracker.look;
        tracker

    } 

    /// Update the last known position of this tracked entity.
    fn set_pos(&mut self, pos: DVec3) {
        let scaled = pos.mul(32.0).floor().as_ivec3();
        self.pos = (scaled.x, scaled.y, scaled.z);
    }

    /// Update the last known look of this tracked entity.
    fn set_look(&mut self, look: Vec2) {
        // Rebase 0..2PI to 0..256. 
        let scaled = look.mul(256.0).div(std::f32::consts::TAU);
        // We can cast to i8, this will take the low 8 bits and wrap around.
        self.look = (scaled.x as i8, scaled.y as i8);
    }

    /// Update the last known 
    fn set_vel(&mut self, vel: DVec3) {
        if let Some(tracker) = &mut self.vel {
            // The Notchian client clamps the input velocity, this ensure that the scaled 
            // vector is in i16 range or integers.
            let scaled = vel.clamp(DVec3::splat(-3.9), DVec3::splat(3.9)).mul(8000.0).as_ivec3();
            tracker.vel = (scaled.x as i16, scaled.y as i16, scaled.z as i16);
        }
    }

    /// Update this tracker to determine which move packet to send and to which players.
    fn update_players(&mut self, players: &[ServerPlayer]) {

        let mut send_pos = true;
        let send_look = self.look.0.abs_diff(self.sent_look.0) >= 8 || self.look.1.abs_diff(self.sent_look.1) >= 8;

        // Check if the delta can be sent with a move packet.
        let dx = i8::try_from(self.pos.0 - self.sent_pos.0).ok();
        let dy = i8::try_from(self.pos.1 - self.sent_pos.1).ok();
        let dz = i8::try_from(self.pos.2 - self.sent_pos.2).ok();

        let mut move_packet = None;
        let forced_position = self.forced_countdown_ticks > 400;

        if let (false, Some(dx), Some(dy), Some(dz)) = (forced_position, dx, dy, dz) {

            // We don't send position if delta is too small.
            send_pos = dx.abs() >= 8 || dy.abs() >= 8 || dz.abs() >= 8;

            if send_pos && send_look {
                move_packet = Some(OutPacket::EntityMoveAndLook(proto::EntityMoveAndLookPacket {
                    entity_id: self.id,
                    dx,
                    dy,
                    dz,
                    yaw: self.look.0,
                    pitch: self.look.1,
                }))
            } else if send_pos {
                move_packet = Some(OutPacket::EntityMove(proto::EntityMovePacket {
                    entity_id: self.id,
                    dx,
                    dy,
                    dz,
                }))
            } else if send_look {
                move_packet = Some(OutPacket::EntityLook(proto::EntityLookPacket {
                    entity_id: self.id,
                    yaw: self.look.0,
                    pitch: self.look.1,
                }))
            }

        } else {
            self.forced_countdown_ticks = 0;
            move_packet = Some(OutPacket::EntityPositionAndLook(proto::EntityPositionAndLookPacket {
                entity_id: self.id,
                x: self.pos.0,
                y: self.pos.1,
                z: self.pos.2,
                yaw: self.look.0,
                pitch: self.look.1,
            }));
        }

        if send_pos {
            self.sent_pos = self.pos;
        }

        if send_look {
            self.sent_look = self.look;
        }

        // If velocity tracking is enabled...
        if let Some(tracker) = &mut self.vel {
            // We differ from the Notchian server because we don't check for the distance.
            let dvx = tracker.vel.0 as i32 - tracker.sent_vel.0 as i32;
            let dvy = tracker.vel.1 as i32 - tracker.sent_vel.1 as i32;
            let dvz = tracker.vel.2 as i32 - tracker.sent_vel.2 as i32;
            // If any axis velocity change by 0.0125 (100 when encoded *8000).
            if dvx.abs() > 100 || dvy.abs() > 100 || dvz.abs() > 100 {
                for player in players {
                    if player.tracked_entities.contains(&self.id) {
                        player.send(OutPacket::EntityVelocity(proto::EntityVelocityPacket {
                            entity_id: self.id,
                            vx: tracker.vel.0,
                            vy: tracker.vel.1,
                            vz: tracker.vel.2,
                        }));
                    }
                }
                tracker.sent_vel = tracker.vel;
            }
        }

        if let Some(packet) = move_packet {
            for player in players {
                if player.tracked_entities.contains(&self.id) {
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
        if player.entity_id == self.id {
            return;
        }

        let delta = player.pos - IVec3::new(self.pos.0, self.pos.1, self.pos.2).as_dvec3() / 32.0;
        if delta.x.abs() <= self.distance as f64 && delta.z.abs() <= self.distance as f64 {
            if player.tracked_entities.insert(self.id) {
                self.spawn_player_entity(player, world);
            }
        } else if player.tracked_entities.remove(&self.id) {
            self.kill_player_entity(player);
        }

    }

    /// Force untrack this entity to this player if the player is already tracking it.
    fn untrack_player(&self, player: &mut ServerPlayer) {
        if player.tracked_entities.remove(&self.id) {
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
        let Some(entity) = world.get_entity(self.id) else { return };
        
        match entity {
            Entity::Player(base) => self.spawn_player_entity_player(player, base),
            Entity::Item(base) => self.spawn_player_entity_item(player, base),
            Entity::Minecart(base) => {
                match base.kind {
                    entity::Minecart::Normal => self.spawn_player_entity_object(player, 10, false),
                    entity::Minecart::Chest { .. } => self.spawn_player_entity_object(player, 11, false),
                    entity::Minecart::Furnace { .. } => self.spawn_player_entity_object(player, 12, false),
                }
            }
            Entity::Boat(_) => self.spawn_player_entity_object(player, 1, false),
            Entity::Painting(_) => todo!(),  // TODO:
            Entity::Fish(_) => self.spawn_player_entity_object(player, 90, false),
            Entity::LightningBolt(_) => (),
            Entity::FallingBlock(base) => {
                // NOTE: We use sand for any block id that is unsupported.
                match base.kind.block_id {
                    block::GRAVEL => self.spawn_player_entity_object(player, 71, false),
                    _ => self.spawn_player_entity_object(player, 70, false),
                }
            }
            Entity::Tnt(_) => self.spawn_player_entity_object(player, 50, false),
            Entity::Arrow(_) => self.spawn_player_entity_object(player, 60, true),
            Entity::Egg(_) => self.spawn_player_entity_object(player, 62, false),
            Entity::Fireball(_) => self.spawn_player_entity_object(player, 63, true),
            Entity::Snowball(_) => self.spawn_player_entity_object(player, 61, false),
            Entity::Ghast(_) => self.spawn_player_entity_mob(player, 56),
            Entity::Slime(_) => self.spawn_player_entity_mob(player, 55),
            Entity::Pig(_) => self.spawn_player_entity_mob(player, 90),
            Entity::Chicken(_) => self.spawn_player_entity_mob(player, 93),
            Entity::Cow(_) => self.spawn_player_entity_mob(player, 92),
            Entity::Sheep(_) => self.spawn_player_entity_mob(player, 91),
            Entity::Squid(_) => self.spawn_player_entity_mob(player, 94),
            Entity::Wolf(_) => self.spawn_player_entity_mob(player, 95),
            Entity::Creeper(_) => self.spawn_player_entity_mob(player, 50),
            Entity::Giant(_) => self.spawn_player_entity_mob(player, 53),
            Entity::PigZombie(_) => self.spawn_player_entity_mob(player, 57),
            Entity::Skeleton(_) => self.spawn_player_entity_mob(player, 51),
            Entity::Spider(_) => self.spawn_player_entity_mob(player, 52),
            Entity::Zombie(_) => self.spawn_player_entity_mob(player, 54),
        }

    }

    fn spawn_player_entity_player(&self, player: &ServerPlayer, base: &PlayerEntity) {
        player.send(OutPacket::PlayerSpawn(proto::PlayerSpawnPacket {
            entity_id: self.id,
            username: base.kind.kind.username.clone(),
            x: self.sent_pos.0, 
            y: self.sent_pos.1, 
            z: self.sent_pos.2, 
            yaw: self.sent_look.0,
            pitch: self.sent_look.1,
            current_item: 0, // TODO:
        }));
    }

    fn spawn_player_entity_item(&self, player: &ServerPlayer, base: &ItemEntity) {
        let vel = base.vel.mul(128.0).as_ivec3();
        player.send(OutPacket::ItemSpawn(proto::ItemSpawnPacket { 
            entity_id: self.id, 
            stack: base.kind.stack, 
            x: self.sent_pos.0, 
            y: self.sent_pos.1, 
            z: self.sent_pos.2, 
            vx: vel.x as i8,
            vy: vel.y as i8,
            vz: vel.z as i8,
        }));
    }

    fn spawn_player_entity_object(&self, player: &ServerPlayer, kind: u8, vel: bool) {
        player.send(OutPacket::ObjectSpawn(proto::ObjectSpawnPacket {
            entity_id: self.id,
            kind,
            x: self.sent_pos.0, 
            y: self.sent_pos.1, 
            z: self.sent_pos.2, 
            velocity: vel.then(|| {
                self.vel.as_ref().expect("expected velocity to be tracked").sent_vel
            })
        }));
    }

    fn spawn_player_entity_mob(&self, player: &ServerPlayer, kind: u8) {
        player.send(OutPacket::MobSpawn(proto::MobSpawnPacket {
            entity_id: self.id,
            kind,
            x: self.sent_pos.0, 
            y: self.sent_pos.1, 
            z: self.sent_pos.2, 
            yaw: self.sent_look.0,
            pitch: self.sent_look.1,
            metadata: vec![], // TODO:
        }));
    }

    /// Kill the entity on the player side.
    fn kill_player_entity(&self, player: &ServerPlayer) {
        player.send(OutPacket::EntityKill(proto::EntityKillPacket { 
            entity_id: self.id
        }));
    }

}

/// A pointer to a slot in an inventory.
struct SlotHandle<'a> {
    /// True if the client is able to drop item into this stack, if not then it can only
    /// pickup the item stack.
    kind: SlotKind<'a>,
    notify: SlotNotify,
}

/// Represent a major slot kind.
enum SlotKind<'a> {
    /// A standard slot referencing a single item stack.
    Standard {
        /// The stack referenced by this slot handle.
        stack: &'a mut ItemStack,
        /// The access kind to this slot.
        access: SlotAccess,
        /// The maximum stack size this slot can accept.
        max_size: u16,
    },
    /// The slot represent a crafting result.
    CraftingResult {
        /// The crafting grid item stacks.
        craft_inv: &'a mut [ItemStack; 9],
        /// The crafting tracker for the player.
        craft_tracker: &'a mut CraftTracker,
    },
}

/// Represent the kind of drop rule to apply to this slot.
#[derive(Clone, Copy)]
enum SlotAccess {
    /// The cursor is able to pickup and drop items into this slot. 
    PickupDrop,
    /// The cursor isn't able to drop items into this slot, it can only pickup. The field
    /// gives the minimum number of items that can be picked up at the same time. 
    /// Typically used for crafting because only a full recipe result can be picked up.
    Pickup(u16),
    /// This slot only accepts helmet armor items.
    ArmorHelmet,
    /// This slot only accepts chestplate armor items.
    ArmorChestplate,
    /// This slot only accepts leggings armor items.
    ArmorLeggings,
    /// This slot only accepts boots armor items.
    ArmorBoots,
}

/// Type of notification that will be triggered when the slot gets modified.
enum SlotNotify {
    /// The modification of the slot has no effect.
    None,
    /// The modification of the slot requires the crafting matrix to be resent.
    /// This should only be used for craft matrix windows, where the craft result is in
    /// slot 0.
    Craft {
        /// For each craft inventory stack a client slot number. If not present, this 
        /// means that the crafting matrix should not be updated. If the slot should not
        /// be sent to the client, then the value must be negative.
        mapping: Option<&'static [i16; 9]>,
        /// True if the craft result should be updated from matrix and resent.
        modified: bool,
    },
    /// A block entity storage event need to be pushed to the world.
    BlockEntityStorageEvent {
        /// The position of the block entity.
        pos: IVec3,
        /// The index of the inventory stack that is modified.
        storage: BlockEntityStorage,
        /// If the stack is actually modified, this is the new item stack at the index.
        stack: Option<ItemStack>,
    }
}

impl<'a> SlotHandle<'a> {

    /// Get the maximum stack size for that slot.
    fn max_stack_size(&self) -> u16 {
        match self.kind {
            SlotKind::Standard { max_size, .. } => max_size,
            SlotKind::CraftingResult { .. } => 64,
        }
    }

    /// Get the access rule to this slot.
    fn get_access(&self) -> SlotAccess {
        match self.kind {
            SlotKind::Standard { access, .. } => access,
            SlotKind::CraftingResult { ref craft_tracker, .. } => 
                SlotAccess::Pickup(craft_tracker.recipe().map(|stack| stack.size).unwrap_or(0)),
        }
    }

    /// Get the stack in this slot.
    fn get_stack(&mut self) -> ItemStack {
        match &self.kind {
            SlotKind::Standard { stack, .. } => **stack,
            SlotKind::CraftingResult { craft_tracker, .. } =>
                craft_tracker.recipe().unwrap_or_default()
        }
    }

    /// Set the stack in this slot, called if `is_valid` previously returned `true`, if
    /// the latter return `false`, this function can only be called with `EMPTY` stack.
    /// 
    /// This function also push the slot changes that happened into `slot_changes` of the
    /// server player temporary vector.
    fn set_stack(&mut self, new_stack: ItemStack) {

        match &mut self.kind {
            SlotKind::Standard { stack, .. } => {
                **stack = new_stack;
            }
            SlotKind::CraftingResult { 
                craft_inv, 
                craft_tracker,
            } => {
                craft_tracker.consume(*craft_inv);
            }
        }

        match &mut self.notify {
            SlotNotify::None => {}
            SlotNotify::Craft { modified, .. } => *modified = true,
            SlotNotify::BlockEntityStorageEvent { stack, .. } => *stack = Some(new_stack),
        }

    }

}

impl SlotAccess {

    fn can_drop(self, stack: ItemStack) -> bool {
        match self {
            SlotAccess::PickupDrop => true,
            SlotAccess::Pickup(_) => false,
            SlotAccess::ArmorHelmet => matches!(stack.id, 
                item::LEATHER_HELMET | 
                item::GOLD_HELMET | 
                item::CHAIN_HELMET | 
                item::IRON_HELMET | 
                item::DIAMOND_HELMET) || stack.id == block::PUMPKIN as u16,
            SlotAccess::ArmorChestplate => matches!(stack.id, 
                item::LEATHER_CHESTPLATE | 
                item::GOLD_CHESTPLATE | 
                item::CHAIN_CHESTPLATE | 
                item::IRON_CHESTPLATE | 
                item::DIAMOND_CHESTPLATE),
            SlotAccess::ArmorLeggings => matches!(stack.id, 
                item::LEATHER_LEGGINGS | 
                item::GOLD_LEGGINGS | 
                item::CHAIN_LEGGINGS | 
                item::IRON_LEGGINGS | 
                item::DIAMOND_LEGGINGS),
            SlotAccess::ArmorBoots => matches!(stack.id, 
                item::LEATHER_BOOTS | 
                item::GOLD_BOOTS | 
                item::CHAIN_BOOTS | 
                item::IRON_BOOTS | 
                item::DIAMOND_BOOTS),
        }
    }

}

/// Create a new chunk data packet for the given chunk. This only works for a single 
/// chunk and the given coordinate should be part of that chunk. The two arguments "from"
/// and "to" are inclusive but might be modified to include more blocks if ths reduces
/// computation.
fn new_chunk_data_packet(chunk: &Chunk, mut from: IVec3, mut size: IVec3) -> proto::ChunkDataPacket {

    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    
    debug_assert!(size.x != 0 && size.y != 0 && size.z != 0);
    
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    chunk.write_data(&mut encoder, &mut from, &mut size).unwrap();

    debug_assert!(size.x != 0 && size.y != 0 && size.z != 0);
    
    proto::ChunkDataPacket {
        x: from.x,
        y: from.y as i16, 
        z: from.z, 
        x_size: size.x as u8, 
        y_size: size.y as u8, 
        z_size: size.z as u8,
        compressed_data: Arc::new(encoder.finish().unwrap()),
    }
    
}
