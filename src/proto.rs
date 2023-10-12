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
    /// Sent by the client to handshake.
    Handshake(ServerHandshakePacket),
    Chat(ChatPacket),
    /// A login request from the client.
    Login(ServerLoginPacket),
    PlayerFlying(PlayerFlyingPacket),
    PlayerPosition(PlayerPositionPacket),
    PlayerLook(PlayerLookPacket),
    PlayerPositionLook(PlayerPositionLookPacket),
    PlayerBreakBlock(PlayerBreakBlockPacket),
    EntityAnimation(EntityAnimationPacket),
}

/// A packet to send to a client (client-bound).
#[derive(Debug, Clone)]
pub enum ClientPacket {
    /// Used for TCP keep alive.
    KeepAlive,
    /// Answered by the server when the client wants to handshake.
    Handshake(ClientHandshakePacket),
    /// Answered by the server to a client's login request, if successful.
    Login(ClientLoginPacket),
    /// Sent to a player after successful login with its position.
    PlayerSpawnPosition(PlayerSpawnPositionPacket),
    UpdateTime(UpdateTimePacket),
    Chat(ChatPacket),
    PlayerFlying(PlayerFlyingPacket),
    PlayerPosition(PlayerPositionPacket),
    PlayerLook(PlayerLookPacket),
    PlayerPositionLook(PlayerPositionLookPacket),
    EntityAnimation(EntityAnimationPacket),
    PreChunk(PreChunkPacket),
    MapChunk(MapChunkPacket),
    BlockChange(BlockChangePacket),
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
pub struct EntityAnimationPacket {
    pub entity_id: u32,
    pub animate: u8,
}

#[derive(Debug, Clone)]
pub struct PreChunkPacket {
    pub cx: i32,
    pub cz: i32,
    pub init: bool,
}

#[derive(Debug, Clone)]
pub struct MapChunkPacket {
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
            10 => ServerPacket::PlayerFlying(PlayerFlyingPacket {
                on_ground: read.read_java_boolean()?,
            }),
            11 => {
                let x = read.read_java_double()?;
                let y = read.read_java_double()?;
                let stance = read.read_java_double()?;
                let z = read.read_java_double()?;
                let on_ground = read.read_java_boolean()?;
                ServerPacket::PlayerPosition(PlayerPositionPacket {
                    pos: DVec3::new(x, y, z),
                    stance,
                    on_ground,
                })
            }
            12 => {
                let yaw = read.read_java_float()?;
                let pitch = read.read_java_float()?;
                let on_ground = read.read_java_boolean()?;
                ServerPacket::PlayerLook(PlayerLookPacket {
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
                ServerPacket::PlayerPositionLook(PlayerPositionLookPacket {
                    pos: DVec3::new(x, y, z),
                    look: Vec2::new(yaw, pitch),
                    stance,
                    on_ground,
                })
            }
            14 => ServerPacket::PlayerBreakBlock(PlayerBreakBlockPacket {
                status: read.read_java_byte()? as u8,
                x: read.read_java_int()?,
                y: read.read_java_byte()?,
                z: read.read_java_int()?,
                face: read.read_java_byte()? as u8,
            }),
            15 => return Err(new_todo_packet_err("place block")),
            16 => return Err(new_todo_packet_err("block item switch")),
            18 => ServerPacket::EntityAnimation(EntityAnimationPacket {
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
            ClientPacket::Handshake(packet) => {
                write.write_u8(2)?;
                write.write_java_string(&packet.server)?;
            }
            ClientPacket::Login(packet) => {
                write.write_u8(1)?;
                write.write_java_int(packet.entity_id)?;
                write.write_java_string("")?; // No username it sent to the client.
                write.write_java_long(packet.random_seed)?;
                write.write_java_byte(packet.dimension)?;
            }
            ClientPacket::UpdateTime(packet) => {
                write.write_u8(4)?;
                write.write_java_long(packet.time as i64)?;
            }
            ClientPacket::Chat(packet) => {
                write.write_u8(3)?;
                write.write_java_string(&packet.message[..packet.message.len().min(199)])?;
            }
            ClientPacket::PlayerSpawnPosition(packet)=> {
                write.write_u8(6)?;
                write.write_java_int(packet.pos.x)?;
                write.write_java_int(packet.pos.y)?;
                write.write_java_int(packet.pos.z)?;
            }
            ClientPacket::PlayerFlying(packet) => {
                write.write_u8(10)?;
                write.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::PlayerPosition(packet) => {
                write.write_u8(11)?;
                write.write_java_double(packet.pos.x)?;
                write.write_java_double(packet.pos.y)?;
                write.write_java_double(packet.stance)?;
                write.write_java_double(packet.pos.z)?;
                write.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::PlayerLook(packet) => {
                write.write_u8(12)?;
                write.write_java_float(packet.look.x)?;
                write.write_java_float(packet.look.y)?;
                write.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::PlayerPositionLook(packet) => {
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
            ClientPacket::PreChunk(packet) => {
                write.write_u8(50)?;
                write.write_java_int(packet.cx)?;
                write.write_java_int(packet.cz)?;
                write.write_java_boolean(packet.init)?;
            }
            ClientPacket::MapChunk(packet) => {
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
