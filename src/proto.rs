//! Minecraft Beta 1.7.3 network protocol definition.

use std::io::{Read, self, Write};
use std::fmt::Arguments;

use glam::{DVec3, Vec2, IVec3};

use byteorder::{ReadBytesExt, WriteBytesExt};

use crate::util::tcp::{TcpServerPacket, TcpClientPacket};
use crate::util::io::{ReadPacketExt, WritePacketExt};


/// A packet received by the server (server-bound).
#[derive(Debug, Clone)]
pub enum ServerPacket {
    /// Used for TCP keep alive.
    KeepAlive,
    /// A login request from the client.
    Login(ServerLoginPacket),
    /// Sent by the client to handshake.
    Handshake(ServerHandshakePacket),
    /// A chat message.
    Chat(ChatPacket),
    /// The client's player interact with an entity.
    Interact(()),
    /// The client's player want to respawn after being dead.
    Respawn(()),
    /// The client's player is not moving/rotating.
    Flying(PlayerFlyingPacket),
    /// The client's player is moving but not rotating.
    Position(PlayerPositionPacket),
    /// The client's player is rotating but not moving.
    Look(PlayerLookPacket),
    /// The client's player is moving and rotating.
    PositionLook(PlayerPositionLookPacket),
    /// The client's player break a block.
    BreakBlock(PlayerBreakBlockPacket),
    /// The client's player place a block.
    PlaceBlock(()),
    /// The client's player change its hand item.
    HandItem(()),
    /// The client's player has an animation, vanilla client usually only send swing arm.
    Animation(AnimationPacket),
    /// The player is making an action, like (un)crouch or leave bed.
    Action(()),
    /// The client is closing a window.
    WindowClose(()),
    /// The client clicked a window.
    WindowClick(()),
    /// Answer to a server transaction rejection.
    WindowTransaction(()),
    /// Sent when a player click the "Done" button after placing a sign.
    UpdateSign(()),
}

/// A packet to send to a client (client-bound).
#[derive(Debug, Clone)]
pub enum ClientPacket {
    /// Used for TCP keep alive.
    KeepAlive,
    /// Answered by the server to a client's login request, if successful.
    Login(ClientLoginPacket),
    /// Answered by the server when the client wants to handshake.
    Handshake(ClientHandshakePacket),
    /// A chat message sent to the client.
    Chat(ChatPacket),
    /// Update the world's time of the client.
    UpdateTime(UpdateTimePacket),
    /// Sent after a player spawn packet to setup each of the 5 slots (held item and 
    /// armor slots) with the items.
    PlayerInventory(()),
    /// Set the spawn position for the compass to point to.
    SpawnPosition(PlayerSpawnPositionPacket),
    /// Update the client's player health.
    UpdateHealth(()),
    /// Sent to the client when the player has been successfully respawned.
    Respawn(()),
    /// Legal to send but not made in practice.
    Flying(PlayerFlyingPacket),
    /// Legal to send but not made in practice.
    Position(PlayerPositionPacket),
    /// Legal to send but not made in practice.
    Look(PlayerLookPacket),
    /// Set the client's player position and look.
    PositionLook(PlayerPositionLookPacket),
    /// Set a given player to sleep in a bed.
    PlayerSleep(()),
    /// An entity play an animation.
    EntityAnimation(AnimationPacket),
    /// A player entity to spawn.
    PlayerSpawn(()),
    /// An item entity to spawn.
    ItemSpawn(()),
    /// A player entity has picked up an item entity on ground.
    PlayerItemPickup(()),
    /// An object entity to spawn.
    ObjectSpawn(()),
    /// A mob entity to spawn.
    MobSpawn(()),
    /// A painting entity to spawn.
    PaintingSpawn(()),
    /// Update an entity velocity.
    EntityVelocity(()),
    /// Kill an entity.
    EntityKill(()),
    /// Base packet for subsequent entity packets, this packet alone is not sent by the
    /// vanilla server.
    Entity(()),
    /// Move an entity by a given offset.
    EntityMove(()),
    /// Set an entity' look.
    EntityLook(()),
    /// Move an entity by a given offset and set its look.
    EntityMoveAndLook(()),
    /// Teleport an entity to a position and set its look.
    EntityPositionAndLook(()),
    /// Not fully understood.
    EntityStatus(()),
    /// Make an entity ride another one.
    EntityRide(()),
    /// Modify an entity's metadata.
    EntityMetadata(()),
    /// Notify the client of a chunk initialization or deletion, this is required before
    /// sending blocks and chunk data.
    ChunkState(ChunkStatePacket),
    /// A bulk send of chunk data.
    ChunkData(ChunkDataPacket),
    /// Many block changed at the same time.
    BlockMultiChange(()),
    /// A single block changed.
    BlockChange(BlockChangePacket),
    /// An action to apply to a block, currently only note block and pistons.
    BlockAction(()),
    /// Sent when an explosion happen, from TNT or creeper.
    Explosion(()),
    /// Play sound on the client.
    SoundPlay(()),
    /// Various state notification, such as raining begin/end and invalid bed to sleep.
    Notification(()),
    /// Spawn a lightning bold.
    LightningBolt(()),
    /// Force the client to quit a window (when a chest is destroyed).
    WindowClose(()),
    /// Change a slot in a window.
    WindowSetItem(()),
    /// Set all items in a window.
    WindowItems(()),
    /// Set a progress bar in a window (for furnaces).
    WindowProgressBar(()),
    /// Information about a window transaction to the client.
    WindowTransaction(()),
    /// A sign is discovered or is created.
    UpdateSign(()),
    /// Complex item data.
    ItemData(()),
    /// Increment a statistic by a given amount.
    StatisticIncrement(()),
    /// Sent to a client to force disconnect it from the server.
    Disconnect(DisconnectPacket),
}

#[derive(Debug, Clone)]
pub struct ServerHandshakePacket {
    /// Username of the player trying to connect.
    pub username: String,
}

#[derive(Debug, Clone)]
pub struct ClientHandshakePacket {
    /// Server identifier that accepted the player handshake. This equals '-' in 
    /// offline mode.
    pub server: String,
}

#[derive(Debug, Clone)]
pub struct ServerLoginPacket {
    /// Current protocol version, should be 14 for this version.
    pub protocol_version: i32,
    /// The username of the player that connects.
    pub username: String,
}

#[derive(Debug, Clone)]
pub struct ClientLoginPacket {
    /// The entity id of the player being connected.
    pub entity_id: i32,
    /// A random seed sent to the player.
    pub random_seed: i64,
    /// The dimension the player is connected to.
    pub dimension: i8,
}

#[derive(Debug, Clone)]
pub struct UpdateTimePacket {
    /// The world time (in game ticks).
    pub time: u64,
}

#[derive(Debug, Clone)]
pub struct ChatPacket {
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct PlayerSpawnPositionPacket {
    /// The spawn position.
    pub pos: IVec3,
}

#[derive(Debug, Clone)]
pub struct PlayerFlyingPacket {
    pub on_ground: bool,
}

#[derive(Debug, Clone)]
pub struct PlayerPositionPacket {
    pub pos: DVec3,
    pub stance: f64,
    pub on_ground: bool,
}

#[derive(Debug, Clone)]
pub struct PlayerLookPacket {
    pub look: Vec2,
    pub on_ground: bool,
}

#[derive(Debug, Clone)]
pub struct PlayerPositionLookPacket {
    pub pos: DVec3,
    pub stance: f64,
    pub look: Vec2,
    pub on_ground: bool,
}

#[derive(Debug, Clone)]
pub struct PlayerBreakBlockPacket {
    pub x: i32,
    pub y: i8,
    pub z: i32,
    pub face: u8,
    pub status: u8,
}

#[derive(Debug, Clone)]
pub struct AnimationPacket {
    pub entity_id: u32,
    pub animate: u8,
}

#[derive(Debug, Clone)]
pub struct ChunkStatePacket {
    pub cx: i32,
    pub cz: i32,
    pub init: bool,
}

#[derive(Debug, Clone)]
pub struct ChunkDataPacket {
    pub x: i32,
    pub y: i16,
    pub z: i32,
    pub x_size: u8,
    pub y_size: u8,
    pub z_size: u8,
    pub compressed_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct BlockChangePacket {
    pub x: i32,
    pub y: i8,
    pub z: i32,
    pub block: u8,
    pub metadata: u8,
}

#[derive(Debug, Clone)]
pub struct DisconnectPacket {
    /// The reason for being kicked or disconnection.
    pub reason: String,
}


impl TcpServerPacket for ServerPacket {

    fn read(read: &mut impl Read) -> io::Result<Self> {
        Ok(match read.read_u8()? {
            0 => ServerPacket::KeepAlive,
            1 => {

                let packet = ServerPacket::Login(ServerLoginPacket {
                    protocol_version: read.read_java_int()?, 
                    username: read.read_java_string(16)?,
                });

                // Unused when client connects to server.
                let _map_seed = read.read_java_long()?;
                let _dimension = read.read_java_byte()?;

                packet

            }
            2 => ServerPacket::Handshake(ServerHandshakePacket {
                username: read.read_java_string(16)?
            }),
            3 => ServerPacket::Chat(ChatPacket { 
                message: read.read_java_string(119)?,
            }),
            7 => return Err(new_todo_packet_err("use entity")),
            9 => return Err(new_todo_packet_err("respawn")),
            10 => ServerPacket::Flying(PlayerFlyingPacket {
                on_ground: read.read_java_boolean()?,
            }),
            11 => {
                let x = read.read_java_double()?;
                let y = read.read_java_double()?;
                let stance = read.read_java_double()?;
                let z = read.read_java_double()?;
                let on_ground = read.read_java_boolean()?;
                ServerPacket::Position(PlayerPositionPacket {
                    pos: DVec3::new(x, y, z),
                    stance,
                    on_ground,
                })
            }
            12 => {
                let yaw = read.read_java_float()?;
                let pitch = read.read_java_float()?;
                let on_ground = read.read_java_boolean()?;
                ServerPacket::Look(PlayerLookPacket {
                    look: Vec2::new(yaw, pitch), 
                    on_ground,
                })
            }
            13 => {
                let x = read.read_java_double()?;
                let y = read.read_java_double()?;
                let stance = read.read_java_double()?;
                let z = read.read_java_double()?;
                let yaw = read.read_java_float()?;
                let pitch = read.read_java_float()?;
                let on_ground = read.read_java_boolean()?;
                ServerPacket::PositionLook(PlayerPositionLookPacket {
                    pos: DVec3::new(x, y, z),
                    look: Vec2::new(yaw, pitch),
                    stance,
                    on_ground,
                })
            }
            14 => ServerPacket::BreakBlock(PlayerBreakBlockPacket {
                status: read.read_java_byte()? as u8,
                x: read.read_java_int()?,
                y: read.read_java_byte()?,
                z: read.read_java_int()?,
                face: read.read_java_byte()? as u8,
            }),
            15 => return Err(new_todo_packet_err("place block")),
            16 => return Err(new_todo_packet_err("block item switch")),
            18 => ServerPacket::Animation(AnimationPacket {
                entity_id: read.read_java_int()? as u32,
                animate: read.read_java_byte()? as u8,
            }),
            19 => return Err(new_todo_packet_err("entity action")),
            27 => return Err(new_todo_packet_err("position??")),
            101 => return Err(new_todo_packet_err("close window")),
            102 => return Err(new_todo_packet_err("click window")),
            106 => return Err(new_todo_packet_err("transaction")),
            130 => return Err(new_todo_packet_err("update sign")),
            255 => return Err(new_todo_packet_err("kick/disconnect")),
            id => return Err(new_invalid_packet_err(format_args!("unknown id {id}"))),
        })
    }

}

impl TcpClientPacket for ClientPacket {

    fn write(&self, write: &mut impl Write) -> io::Result<()> {
        
        match self {
            ClientPacket::KeepAlive => write.write_u8(0)?,
            ClientPacket::Login(packet) => {
                write.write_u8(1)?;
                write.write_java_int(packet.entity_id)?;
                write.write_java_string("")?; // No username it sent to the client.
                write.write_java_long(packet.random_seed)?;
                write.write_java_byte(packet.dimension)?;
            }
            ClientPacket::Handshake(packet) => {
                write.write_u8(2)?;
                write.write_java_string(&packet.server)?;
            }
            ClientPacket::Chat(packet) => {
                write.write_u8(3)?;
                write.write_java_string(&packet.message[..packet.message.len().min(199)])?;
            }
            ClientPacket::UpdateTime(packet) => {
                write.write_u8(4)?;
                write.write_java_long(packet.time as i64)?;
            }
            ClientPacket::SpawnPosition(packet)=> {
                write.write_u8(6)?;
                write.write_java_int(packet.pos.x)?;
                write.write_java_int(packet.pos.y)?;
                write.write_java_int(packet.pos.z)?;
            }
            ClientPacket::Flying(packet) => {
                write.write_u8(10)?;
                write.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::Position(packet) => {
                write.write_u8(11)?;
                write.write_java_double(packet.pos.x)?;
                write.write_java_double(packet.pos.y)?;
                write.write_java_double(packet.stance)?;
                write.write_java_double(packet.pos.z)?;
                write.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::Look(packet) => {
                write.write_u8(12)?;
                write.write_java_float(packet.look.x)?;
                write.write_java_float(packet.look.y)?;
                write.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::PositionLook(packet) => {
                write.write_u8(13)?;
                write.write_java_double(packet.pos.x)?;
                write.write_java_double(packet.pos.y)?;
                write.write_java_double(packet.stance)?;
                write.write_java_double(packet.pos.z)?;
                write.write_java_float(packet.look.x)?;
                write.write_java_float(packet.look.y)?;
                write.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::EntityAnimation(packet) => {
                write.write_u8(18)?;
                write.write_java_int(packet.entity_id as i32)?;
                write.write_java_byte(packet.animate as i8)?;
            }
            ClientPacket::ChunkState(packet) => {
                write.write_u8(50)?;
                write.write_java_int(packet.cx)?;
                write.write_java_int(packet.cz)?;
                write.write_java_boolean(packet.init)?;
            }
            ClientPacket::ChunkData(packet) => {
                write.write_u8(51)?;
                write.write_java_int(packet.x)?;
                write.write_java_short(packet.y)?;
                write.write_java_int(packet.z)?;
                write.write_java_byte((packet.x_size - 1) as i8)?;
                write.write_java_byte((packet.y_size - 1) as i8)?;
                write.write_java_byte((packet.z_size - 1) as i8)?;
                write.write_java_int(packet.compressed_data.len() as i32)?;
                write.write_all(&packet.compressed_data)?;
            }
            ClientPacket::BlockChange(packet) => {
                write.write_u8(53)?;
                write.write_java_int(packet.x)?;
                write.write_java_byte(packet.y)?;
                write.write_java_int(packet.z)?;
                write.write_java_byte(packet.block as i8)?;
                write.write_java_byte(packet.metadata as i8)?;
            }
            ClientPacket::Disconnect(packet) => {
                write.write_u8(255)?;
                write.write_java_string(&packet.reason)?;
            }
        }

        Ok(())

    }

}


/// Return an invalid data io error with specific message.
fn new_invalid_packet_err(format: Arguments) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("invalid packet: {format}"))
}

fn new_todo_packet_err(name: &'static str) -> io::Error {
    new_invalid_packet_err(format_args!("todo({name})"))
}
