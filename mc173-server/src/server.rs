//! The network server managing connected players and dispatching incoming packets.

use std::time::{Duration, Instant};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::io;

use anyhow::Result as AnyResult;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use glam::{DVec3, Vec2, IVec3};

use mc173::chunk::{calc_entity_chunk_pos, CHUNK_WIDTH, CHUNK_HEIGHT};
use mc173::world::{World, Dimension, Event};
use mc173::overworld::new_overworld;
use mc173::entity::PlayerEntity;

use crate::proto::{self, Network, NetworkEvent, NetworkClient, InPacket, OutPacket};


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
    pub fn handle_accept(&mut self, client: NetworkClient) {
        println!("[{client:?}] Accepted");
        self.clients.insert(client, ClientState::Handshaking);
    }

    /// Handle a lost client.
    pub fn handle_lost(&mut self, client: NetworkClient, error: Option<io::Error>) {
        println!("[{client:?}] Lost: {error:?}");
        let state = self.clients.remove(&client).unwrap();
        if let ClientState::Playing { world_index, player_index } = state {
            // If the client was playing, remove it from its world.
            let world = &mut self.worlds[world_index];
            let player = world.players.remove(player_index);
            player.handle_lost(&mut world.world);
        }
    }

    pub fn handle_packet(&mut self, client: NetworkClient, packet: InPacket) {
        
        println!("[{client:?}] Packet: {packet:?}");

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
    pub fn handle_handshaking(&mut self, client: NetworkClient, packet: InPacket) {
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
    pub fn handle_handshake(&mut self, client: NetworkClient) {
        self.net.send(client, OutPacket::Handshake(proto::OutHandshakePacket {
            server: "-".to_string(),
        }));
    }

    /// Handle a login after handshake.
    pub fn handle_login(&mut self, client: NetworkClient, packet: proto::InLoginPacket) {

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

        let mut entity = PlayerEntity::new(offline_player.pos);
        entity.base.living.username = packet.username.clone();
        entity.pos = offline_player.pos;
        entity.look = offline_player.look;
        let entity_id = world.world.spawn_entity(entity);

        let player_index = world.players.len();
        world.players.push(ServerPlayer {
            net: self.net.clone(),
            client,
            entity_id,
            username: packet.username,
            pos: offline_player.pos,
            look: offline_player.look,
            sent_chunks: HashSet::new(),
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

        // TODO: Broadcast chat joining chat message.

    }

    /// Send disconnect (a.k.a. kick) to a client.
    pub fn send_disconnect(&mut self, client: NetworkClient, reason: String) {
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
            players: Vec::new(),
        }

    }

    /// Tick this world.
    fn tick(&mut self) {

        self.world.tick();

        // Send time to every playing clients every second.
        let time = self.world.time();
        if time % 20 == 0 {
            for player in &self.players {
                player.net.send(player.client, OutPacket::UpdateTime(proto::UpdateTimePacket {
                    time,
                }));
            }
        }

        // Swap events out in order to proceed them.
        let mut events = self.world.swap_events(None).expect("events should be enabled");
        for event in events.drain(..) {
            // match event {
            //     Event::EntitySpawn { id, pos, look } => todo!(),
            //     Event::EntityKill { id } => todo!(),
            //     Event::EntityPosition { id, pos } => todo!(),
            //     Event::EntityLook { id, look } => todo!(),
            //     Event::BlockChange { pos, prev_block, prev_metadata, new_block, new_metadata } => todo!(),
            //     Event::SpawnPosition { pos } => todo!(),
            // }
            println!("[WORLD] Event: {event:?}");
        }

        // Reinsert events after processing.
        self.world.swap_events(Some(events));

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
    sent_chunks: HashSet<(i32, i32)>,
    /// If the player is breaking a block, this record the breaking state.
    breaking_block: Option<BreakingBlock>,
}

/// State of a player breaking a block.
struct BreakingBlock {
    /// The world time when breaking started.
    time: u64,
    /// The position of the block.
    pos: IVec3,
    /// The block id.
    block: u8,
}

impl ServerPlayer {

    /// Handle loss of this player.
    fn handle_lost(self, world: &mut World) {
        world.kill_entity(self.entity_id);
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
        self.pos = packet.pos;
        self.update_chunks(world);
    }

    /// Handle a look packet.
    fn handle_look(&mut self, world: &mut World, packet: proto::LookPacket) {
        self.look = packet.look;
    }

    /// Handle a position and look packet.
    fn handle_position_look(&mut self, world: &mut World, packet: proto::PositionLookPacket) {
        self.pos = packet.pos;
        self.look = packet.look;
        self.update_chunks(world);
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
                    if self.sent_chunks.insert((cx, cz)) {

                        self.net.send(self.client, OutPacket::ChunkState(proto::ChunkStatePacket {
                            cx, cz, init: true
                        }));

                        let mut compressed_data = Vec::new();

                        let mut encoder = ZlibEncoder::new(&mut compressed_data, Compression::fast());
                        chunk.write_data_to(&mut encoder).unwrap();
                        encoder.finish().unwrap();

                        self.net.send(self.client, OutPacket::ChunkData(proto::ChunkDataPacket {
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
