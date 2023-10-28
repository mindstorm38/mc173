//! The network server managing connected players and dispatching incoming packets.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use std::net::SocketAddr;
use std::ops::{Mul, Div};
use std::io;

use anyhow::Result as AnyResult;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use glam::{DVec3, Vec2, IVec3};

use mc173::chunk::{calc_entity_chunk_pos, calc_chunk_pos_unchecked, CHUNK_WIDTH, CHUNK_HEIGHT};
use mc173::entity::{Entity, PlayerEntity, ItemEntity};
use mc173::world::{World, Dimension, Event};
use mc173::item::crafting::CraftingTracker;
use mc173::item::inventory::Inventory;
use mc173::item::{self, ItemStack};
use mc173::util::Face;
use mc173::block;

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
            slot_changes: Vec::new(),
            crafting_tracker: CraftingTracker::default(),
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
                Event::EntityVelocity { id, vel } =>
                    self.handle_entity_velocity(id, vel),
                Event::EntityPickup { id, target_id } =>
                    self.handle_entity_pickup(id, target_id),
                Event::EntityInventoryItem { id, index, item } =>
                    self.handle_entity_inventory_item(id, index, item),
                Event::BlockChange { pos, new_id: new_block, new_metadata, .. } => 
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

    /// Handle an entity look world event.
    fn handle_entity_velocity(&mut self, id: u32, vel: DVec3) {
        self.trackers.get_mut(&id).unwrap().set_vel(vel);
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
            stack: item.to_non_empty(),
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
    /// A temporary list of item changes in the current window, each item is associated 
    /// to its slot number.
    slot_changes: Vec<(i16, ItemStack)>,
    /// This crafting tracker is used to update the current craft being constructed by 
    /// the player in the current window (player inventory or crafting table interface).
    crafting_tracker: CraftingTracker,
    /// If the player is breaking a block, this record the breaking state.
    breaking_block: Option<BreakingBlock>,
}

/// State of a player breaking a block.
struct BreakingBlock {
    /// The minimum time this break can finish.
    min_time: u64,
    /// The position of the block.
    pos: IVec3,
    /// The block id.
    id: u8,
}

impl ServerPlayer {

    /// Send a packet to this player.
    fn send(&self, packet: OutPacket) {
        self.net.send(self.client, packet);
    }

    /// Send a chat message to this player.
    fn send_chat(&self, message: String) {
        self.send(OutPacket::Chat(proto::ChatPacket { message }));
    }

    /// Handle an incoming packet from this player.
    fn handle(&mut self, world: &mut World, packet: InPacket) {
        
        match packet {
            InPacket::KeepAlive => {}
            InPacket::Flying(_) => {}, // Ignore because it doesn't update anything.
            InPacket::Disconnect(_) =>
                self.handle_disconnect(),
            InPacket::Chat(packet) =>
                self.handle_chat(world, packet.message),
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
            _ => println!("[{:?}] Packet: {packet:?}", self.client)
        }

    }

    /// Just disconnect itself, this will produce a lost event from the network.
    fn handle_disconnect(&mut self) {
        self.net.disconnect(self.client);
    }

    /// Handle a chat message packet.
    fn handle_chat(&mut self, world: &mut World, message: String) {
        if message.starts_with('/') {
            let parts = message.split_whitespace().collect::<Vec<_>>();
            if let Err(message) = self.handle_chat_command(world, &parts) {
                self.send_chat(message);
            }
        }
    }

    /// Handle a chat command, parsed from a chat message packet starting with '/'.
    fn handle_chat_command(&mut self, world: &mut World, parts: &[&str]) -> Result<(), String> {

        let Some(Entity::Player(base)) = world.entity_mut(self.entity_id) else {
            return Err(format!("§cCould not retrieve player entity!"));
        };

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
                    return Err(format!("§cError: unknown item name or id: {id_raw}"));
                }

                let item = item::from_id(id);
                if item.name.is_empty() {
                    return Err(format!("§cError: unknown item id: {id_raw}"));
                }

                let mut stack = ItemStack::new_sized(id, 0, item.max_stack_size);

                if !metadata_raw.is_empty() {
                    stack.damage = metadata_raw.parse::<u16>()
                        .map_err(|_| format!("§cError: invalid item damage: {metadata_raw}"))?;
                }

                if let Some(size_raw) = parts.get(2) {
                    stack.size = size_raw.parse::<u16>()
                        .map_err(|_| format!("§cError: invalid stack size: {size_raw}"))?;
                }

                base.kind.kind.main_inv.add_stack(stack);
                self.send_chat(format!("§aGave {} ({}:{}) x{} to {}", item.name, stack.id, stack.damage, stack.size, self.username));
                Ok(())

            }
            ["/give", ..] => Err(format!("§eUsage: /give <item>[:<damage>] [<size>]")),
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

        let entity = world.entity_mut(self.entity_id).expect("incoherent player entity");
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

    }

    /// Handle a break block packet.
    fn handle_break_block(&mut self, world: &mut World, packet: proto::BreakBlockPacket) {
        
        let Some(Entity::Player(base)) = world.entity_mut(self.entity_id) else { return };
        let pos = IVec3::new(packet.x, packet.y as i32, packet.z);

        let in_water = base.in_water;
        let on_ground = base.on_ground;
        let main_inv = &mut base.kind.kind.main_inv;
        let hand_slot = base.kind.kind.hand_slot as usize;
        let mut stack = main_inv.stack(hand_slot);

        if packet.status == 0 {
            // Start breaking a block, ignore if the position is invalid.
            if let Some((id, _)) = world.block(pos) {

                block::using::use_at(world, pos);
                
                let break_duration = item::breaking::get_break_duration(stack.id, id, in_water, on_ground);
                if break_duration == 0.0 {
                    block::breaking::break_at(world, pos);
                } else {
                    self.breaking_block = Some(BreakingBlock {
                        min_time: world.time() + (break_duration * 0.7) as u64,
                        pos,
                        id,
                    });
                }

            }
        } else if packet.status == 2 {
            // Block breaking should be finished.
            if let Some(state) = self.breaking_block.take() {
                if state.pos == pos && world.time() >= state.min_time {
                    if matches!(world.block(pos), Some((id, _)) if id == state.id) {
                        block::breaking::break_at(world, pos);
                    }
                } else {
                    println!("[WARNING] Incoherent break: {pos} @ {}, got {} @ {}", world.time(), state.pos, state.min_time);
                }
            }
        } else if packet.status == 4 {
            // Drop the selected item.

            if !stack.is_empty() {
                
                stack.size -= 1;
                main_inv.set_stack(hand_slot, stack.to_non_empty().unwrap_or(ItemStack::EMPTY));
                
                self.send(OutPacket::WindowSetItem(proto::WindowSetItemPacket {
                    window_id: 0,
                    slot: 36 + hand_slot as i16,
                    stack: main_inv.stack(hand_slot).to_non_empty(),
                }));

                self.drop_item(world, stack.with_size(1), false);

            }

        }

    }

    /// Handle a place block packet.
    fn handle_place_block(&mut self, world: &mut World, packet: proto::PlaceBlockPacket) {
        
        // This packet only works if the player's entity is a player.
        let Some(Entity::Player(base)) = world.entity_mut(self.entity_id) else { return };

        let face = match packet.direction {
            0 => Face::NegY,
            1 => Face::PosY,
            2 => Face::NegZ,
            3 => Face::PosZ,
            4 => Face::NegX,
            5 => Face::PosX,
            0xFF => return, // TODO: special case with direction
            _ => return,
        };

        let pos = IVec3 {
            x: packet.x,
            y: packet.y as i32,
            z: packet.z,
        };

        let mut new_hand_stack = None;

        // Check if the player is reasonably near the block.
        if base.pos.distance_squared(pos.as_dvec3() + 0.5) < 64.0 {
            
            let hand_stack = base.kind.kind.main_inv.stack(base.kind.kind.hand_slot as usize);
            let look = base.look;

            if !block::using::use_at(world, pos) {
                new_hand_stack = item::using::use_at(world, pos, face, look, hand_stack);
            }

        }

        if let Some(hand_stack) = new_hand_stack {
            let Entity::Player(base) = world.entity_mut(self.entity_id).unwrap() else { panic!() };
            base.kind.kind.main_inv.set_stack(base.kind.kind.hand_slot as usize, hand_stack);
        }

        // world.block_and_metadata(pos);
        // self.send(OutPacket::BlockChange(proto::BlockChangePacket {
        //     x: pos.x,
        //     y: pos.y as i8,
        //     z: pos.z,
        //     block: todo!(),
        //     metadata: todo!(),
        // }))

    }

    /// Handle a hand slot packet.
    fn handle_hand_slot(&mut self, world: &mut World, slot: i16) {

        // This packet only works if the player's entity is a player.
        let Some(Entity::Player(base)) = world.entity_mut(self.entity_id) else { return };
        base.kind.kind.hand_slot = slot as u8;

    }

    /// Handle a window click packet.
    fn handle_window_click(&mut self, world: &mut World, packet: proto::WindowClickPacket) {

        // This packet only works if the player's entity is a player.
        let Some(Entity::Player(base)) = world.entity_mut(self.entity_id) else { return };
        
        // Holding the target slot's item stack.
        let mut cursor_stack = base.kind.kind.cursor_stack;
        let slot_stack;

        if packet.slot == -999 {
            slot_stack = ItemStack::EMPTY;
            if !cursor_stack.is_empty() {

                let mut drop_stack = cursor_stack;
                if packet.right_click {
                    drop_stack = drop_stack.with_size(1);
                }

                cursor_stack.size -= drop_stack.size;
                self.drop_item(world, drop_stack, false);

            }
        } else if packet.shift_click {
            todo!()
        } else {

            let mut slot_handle;

            if packet.window_id == 0 {
                slot_handle = SlotHandle::new_player(self, packet.slot, base);
            } else {
                todo!()
            }

            slot_stack = slot_handle.stack();

            if slot_stack.is_empty() {
                if !cursor_stack.is_empty() && slot_handle.can_drop(cursor_stack) {
                    
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
                if packet.right_click && slot_handle.can_drop(cursor_stack) {
                    cursor_stack.size = (cursor_stack.size + 1) / 2;
                }

                let mut new_slot_stack = slot_stack;
                new_slot_stack.size -= cursor_stack.size;
                if new_slot_stack.size == 0 {
                    slot_handle.set_stack(ItemStack::EMPTY);
                } else {
                    slot_handle.set_stack(new_slot_stack);
                }

            } else if slot_handle.can_drop(cursor_stack) {

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

            } else {

                // This last case is when the slot and the cursor are not empty, but we
                // can't drop the cursor into the slot, in such case we try to pick the 

                if (slot_stack.id, slot_stack.damage) == (cursor_stack.id, cursor_stack.damage) {
                    let cursor_item = item::from_id(cursor_stack.id);
                    if slot_stack.size + cursor_stack.size <= cursor_item.max_stack_size {
                        cursor_stack.size += slot_stack.size;
                        // NOTE: We can only drop EMPTY stack if drop is forbidden.
                        slot_handle.set_stack(ItemStack::EMPTY);
                    }
                }

            }

        }
            
        // Answer with a transaction packet that is accepted if the packet's stack is
        // the same as the server's slot stack.
        self.send(OutPacket::WindowTransaction(proto::WindowTransactionPacket {
            window_id: packet.window_id,
            transaction_id: packet.transaction_id,
            accepted: slot_stack.to_non_empty() == packet.stack,
        }));

        // This temporary vector is filled by slot handles and contains stacks to update
        // in the current window.
        for (slot, stack) in self.slot_changes.drain(..) {
            self.net.send(self.client, OutPacket::WindowSetItem(proto::WindowSetItemPacket { 
                window_id: packet.window_id,
                slot,
                stack: stack.to_non_empty(),
            }));
        }

        // Send the new cursor item.
        if cursor_stack.size == 0 {
            cursor_stack = ItemStack::EMPTY;
        }

        self.send(OutPacket::WindowSetItem(proto::WindowSetItemPacket { 
            window_id: 0xFF,
            slot: -1,
            stack: cursor_stack.to_non_empty(),
        }));

        // At the end where the world is no longer borrowed, re-borrow our player entity
        // and set the new cursor stack.
        let Entity::Player(base) = world.entity_mut(self.entity_id).unwrap() else { panic!() };
        base.kind.kind.cursor_stack = cursor_stack;

    }

    /// Drop an item from the player's entity, items are drop in front of the player, but
    /// the `on_ground` argument can be set to true in order to drop item on the ground.
    fn drop_item(&mut self, world: &mut World, stack: ItemStack, on_ground: bool) {

        let entity = world.entity_mut(self.entity_id).expect("incoherent player entity");
        let base = entity.base_mut();

        let mut item_entity = ItemEntity::default();
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

    fn new(entity: &Entity) -> Self {

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
            entity_id: entity_base.id,
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
                    entity_id: self.entity_id,
                    dx,
                    dy,
                    dz,
                    yaw: self.look.0,
                    pitch: self.look.1,
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
                    yaw: self.look.0,
                    pitch: self.look.1,
                }))
            }

        } else {
            self.forced_countdown_ticks = 0;
            move_packet = Some(OutPacket::EntityPositionAndLook(proto::EntityPositionAndLookPacket {
                entity_id: self.entity_id,
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
                    if player.tracked_entities.contains(&self.entity_id) {
                        player.send(OutPacket::EntityVelocity(proto::EntityVelocityPacket {
                            entity_id: self.entity_id,
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

        let delta = player.pos - IVec3::new(self.pos.0, self.pos.1, self.pos.2).as_dvec3() / 32.0;
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

        let x = self.sent_pos.0;
        let y = self.sent_pos.1;
        let z = self.sent_pos.2;
        let yaw = self.sent_look.0;
        let pitch = self.sent_look.1;
        
        match entity {
            Entity::Player(base) => {
                player.send(OutPacket::PlayerSpawn(proto::PlayerSpawnPacket {
                    entity_id: base.id,
                    username: base.kind.kind.username.clone(),
                    x, 
                    y, 
                    z, 
                    yaw,
                    pitch,
                    current_item: 0, // TODO:
                }));
            }
            Entity::Item(base) => {
                let vel = base.vel.mul(128.0).as_ivec3();
                player.send(OutPacket::ItemSpawn(proto::ItemSpawnPacket { 
                    entity_id: base.id, 
                    stack: base.kind.stack, 
                    x, 
                    y, 
                    z, 
                    vx: vel.x as i8,
                    vy: vel.y as i8,
                    vz: vel.z as i8,
                }));
            }
            Entity::Pig(base) => {
                player.send(OutPacket::MobSpawn(proto::MobSpawnPacket {
                    entity_id: base.id,
                    kind: 90,
                    x, 
                    y, 
                    z, 
                    yaw,
                    pitch,
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

/// A pointer to a slot in an inventory, its type affects the behavior of interactions 
/// with it. Lifetimes are `'w` for references to world, and `'p` for references to the
/// `ServerPlayer` structure that is using the slot.
struct SlotHandle<'p, 'w> {
    player: &'p mut ServerPlayer,
    slot: i16,
    kind: SlotKind<'w>,
}

/// Different kind of slots, these kind of slots are generic and are made to adapt to
/// a variety of containers and interfaces.
enum SlotKind<'w> {
    /// This slot is a regular storage slot in the given inventory and index into it.
    Storage {
        inv: &'w mut Inventory,
        index: usize,
    },
    /// This slot is a player armor slot.
    Armor {
        inv: &'w mut Inventory,
        index: usize,
    },
    /// This slot is part of a crafting grid.
    CraftingGrid {
        inv: &'w mut Inventory,
        inv_width: u8,
        inv_height: u8,
        index: usize,
        result_slot: i16,
    },
    /// This slot is used for the result of a crafting recipe.
    CraftingResult {
        inv: &'w mut Inventory,
        inv_width: u8,
        inv_height: u8,
        grid_first_slot: i16,
    },
}

impl<'p, 'w> SlotHandle<'p, 'w> {

    /// Create a new slot handle for a player inventory slot.
    fn new_player(player: &'p mut ServerPlayer, slot: i16, base: &'w mut PlayerEntity) -> Self {
        Self::new(player, slot, match slot {
            0 => SlotKind::CraftingResult {
                inv: &mut base.kind.kind.craft_inv,
                inv_width: 2,
                inv_height: 2,
                grid_first_slot: 1,
            },
            1..=4 => SlotKind::CraftingGrid { 
                inv: &mut base.kind.kind.craft_inv, 
                inv_width: 2,
                inv_height: 2,
                index: slot as usize - 1, 
                result_slot: 0,
            },
            5..=8 => SlotKind::Armor { 
                inv: &mut base.kind.kind.armor_inv, 
                index: slot as usize - 5,
            },
            9..=35 => SlotKind::Storage { 
                inv: &mut base.kind.kind.main_inv, 
                index: slot as usize,
            },
            36..=44 => SlotKind::Storage { 
                inv: &mut base.kind.kind.main_inv, 
                index: slot as usize - 36,
            },
            _ => panic!()
        })
    }

    fn new(player: &'p mut ServerPlayer, slot: i16, kind: SlotKind<'w>) -> Self {
        Self { player, slot, kind }
    }

    /// Get the maximum stack size for that slot.
    fn max_stack_size(&self) -> u16 {
        match self.kind {
            SlotKind::Armor { .. } => 1,
            _ => 64,
        }
    }

    /// Check if the given item stack can be dropped in the slot.
    fn can_drop(&self, stack: ItemStack) -> bool {
        match self.kind {
            SlotKind::Storage { .. } => true,
            SlotKind::Armor { index, .. } if index == 0 => matches!(stack.id, 
                item::LEATHER_HELMET | 
                item::GOLD_HELMET | 
                item::CHAIN_HELMET | 
                item::IRON_HELMET | 
                item::DIAMOND_HELMET) || stack.id == block::PUMPKIN as u16,
            SlotKind::Armor { index, .. } if index == 1 => matches!(stack.id, 
                item::LEATHER_CHESTPLATE | 
                item::GOLD_CHESTPLATE | 
                item::CHAIN_CHESTPLATE | 
                item::IRON_CHESTPLATE | 
                item::DIAMOND_CHESTPLATE),
            SlotKind::Armor { index, .. } if index == 2 => matches!(stack.id, 
                item::LEATHER_LEGGINGS | 
                item::GOLD_LEGGINGS | 
                item::CHAIN_LEGGINGS | 
                item::IRON_LEGGINGS | 
                item::DIAMOND_LEGGINGS),
            SlotKind::Armor { index, .. } if index == 3 => matches!(stack.id, 
                item::LEATHER_BOOTS | 
                item::GOLD_BOOTS | 
                item::CHAIN_BOOTS | 
                item::IRON_BOOTS | 
                item::DIAMOND_BOOTS),
            SlotKind::Armor { .. } => false,
            SlotKind::CraftingGrid { .. } => true,
            SlotKind::CraftingResult { .. } => false,
        }
    }

    /// Get the stack in this slot.
    fn stack(&self) -> ItemStack {
        match self.kind {
            SlotKind::Storage { ref inv, index } |
            SlotKind::Armor { ref inv, index } |
            SlotKind::CraftingGrid { ref inv, index, .. } => {
                inv.stack(index)
            }
            SlotKind::CraftingResult { .. } => {
                self.player.crafting_tracker.recipe().unwrap_or(ItemStack::EMPTY)
            }
        }
    }

    /// Set the stack in this slot, called if `is_valid` previously returned `true`, if
    /// the latter return `false`, this function can only be called with `EMPTY` stack.
    /// 
    /// This function also push the slot changes that happened into `slot_changes` of the
    /// server player temporary vector.
    fn set_stack(&mut self, stack: ItemStack) {
        match self.kind {
            SlotKind::Storage { ref mut inv, index } |
            SlotKind::Armor { ref mut inv, index } => {
                
                inv.set_stack(index, stack);
                self.player.slot_changes.push((self.slot, stack));

            }
            SlotKind::CraftingGrid { 
                ref mut inv, inv_width, inv_height, 
                index, 
                result_slot,
            } => {

                inv.set_stack(index, stack);
                self.player.crafting_tracker.update(inv, inv_width, inv_height);

                let result_stack = self.player.crafting_tracker.recipe()
                    .unwrap_or(ItemStack::EMPTY);

                self.player.slot_changes.push((self.slot, stack));
                self.player.slot_changes.push((result_slot, result_stack));

            }
            SlotKind::CraftingResult { 
                ref mut inv, inv_width, inv_height,
                grid_first_slot,
            } => {

                // NOTE: The 'can_drop' method always return false for this slot.
                // This means that this result slot has been picked up.
                debug_assert_eq!(stack, ItemStack::EMPTY);

                self.player.crafting_tracker.consume(inv);
                self.player.crafting_tracker.update(inv, inv_width, inv_height);

                let result_stack = self.player.crafting_tracker.recipe()
                    .unwrap_or(ItemStack::EMPTY);
                
                for (i, grid_stack) in inv.stacks().iter().copied().enumerate() {
                    self.player.slot_changes.push((grid_first_slot + i as i16, grid_stack));
                } 

                self.player.slot_changes.push((self.slot, result_stack));

            }
        }
    }

}
