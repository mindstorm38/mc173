//! Network protocol definition and abstraction for interacting with clients.

use std::io::{self, Read, Cursor, Write};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::ops::Range;
use std::time::Duration;

use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use glam::{IVec3, Vec2, DVec3};

use mio::{Poll, Events, Interest, Token};
use mio::net::{TcpListener, TcpStream};
use mio::event::Event;


/// Internal polling token used for the listening socket.
const LISTENER_TOKEN: Token = Token(0);
/// Size of internal buffers for incoming client's data.
const BUF_SIZE: usize = 1024;


/// The server accepts incoming client connections and parses incoming packets that can
/// be retrieved by polling synchronously.
/// 
/// This server only provides low-level protocol codec, it doesn't answer itself to 
/// packets or manage connection with clients.
pub struct PacketServer {
    /// The inner, actual server.
    inner: InnerServer,
    /// The events queue when polling.
    events: Events,
}

/// Inner structure, split from the main one to avoid borrow issue with events queue.
struct InnerServer {
    /// The inner TCP listener.
    listener: TcpListener,
    /// The poll used for event listening TCP events.
    poll: Poll,
    /// The id allocator with use to generate unique polling token.
    token_allocator: TokenAllocator,
    /// Connected clients, mapped to their polling token.
    clients: HashMap<Token, Client>,
}

impl PacketServer {

    /// Bind this server's TCP listener to the given address.
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        
        let poll = Poll::new()?;
        let mut listener = TcpListener::bind(addr)?;
        poll.registry().register(&mut listener, LISTENER_TOKEN, Interest::READABLE)?;

        Ok(Self {
            inner: InnerServer {
                listener,
                poll,
                token_allocator: TokenAllocator::new(1000..10000),
                clients: HashMap::new(),
            },
            events: Events::with_capacity(128),
        })

    }

    /// Poll for incoming packets, the internal packets queue is updated.
    pub fn poll(&mut self, packets: &mut Packets, timeout: Option<Duration>) -> io::Result<()> {

        self.inner.poll.poll(&mut self.events, timeout)?;

        for event in self.events.iter() {
            match event.token() {
                LISTENER_TOKEN => self.inner.handle_listener()?,
                _ => self.inner.handle_client(event, packets)?,
            }
        }

        Ok(())

    }

    /// Send a packet to a client.
    pub fn send(&mut self, client_id: ClientId, packet: &ClientPacket) -> io::Result<()> {

        let client = self.inner.clients.get_mut(&Token(client_id.0)).unwrap();
        assert_eq!(client.writable, FunctionState::Enabled);

        client.write_packet(&packet)

    }

}

impl InnerServer {

    /// Internal function to handle a readable polling event from the TCP listener stream.
    fn handle_listener(&mut self) -> io::Result<()> {

        loop {

            let (
                mut stream, 
                addr
            ) = match self.listener.accept() {
                Ok(t) => t,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            };

            let token = self.token_allocator.alloc().expect("failed to allocate polling token");
            self.poll.registry().register(&mut stream, token, Interest::READABLE | Interest::WRITABLE)?;

            self.clients.insert(token, Client {
                token,
                stream,
                addr,
                writable: FunctionState::Init,
                readable: FunctionState::Init,
                buf: Box::new([0; BUF_SIZE]),
                buf_cursor: 0,
            });

        }

        Ok(())

    }

    /// Internal function to handle a polling event from a client.
    fn handle_client(&mut self, event: &Event, packets: &mut Packets) -> io::Result<()> {

        let token = event.token();
        let client = self.clients.get_mut(&token).expect("invalid client token");

        if event.is_writable() {

            if client.writable == FunctionState::Init {
                client.writable = FunctionState::Enabled;
            }

        }

        if event.is_readable() {

            if client.readable == FunctionState::Init {
                client.readable = FunctionState::Enabled;
            }

            if client.readable == FunctionState::Enabled {
                client.handle_read(packets)?;
            }

        }

        if event.is_write_closed() {
            client.writable = FunctionState::Closed;
        }

        if event.is_read_closed() {
            client.readable = FunctionState::Closed;
        }

        Ok(())

    }

}


struct Client {
    /// The client's token.
    token: Token,
    /// The client's stream.
    stream: TcpStream,
    /// The client's remote socket address.
    addr: SocketAddr,
    /// Writable state.
    writable: FunctionState,
    /// Readable state.
    readable: FunctionState,
    /// Internal buffer to temporarily stores incoming client's data.
    buf: Box<[u8; BUF_SIZE]>,
    /// Cursor in the receiving buffer.
    buf_cursor: usize,
}

impl Client {

    /// Internal function to handle a readable event on this client's socket.
    fn handle_read(&mut self, packets: &mut Packets) -> io::Result<()> {

        loop {
            match self.stream.read(&mut self.buf[self.buf_cursor..]) {
                Ok(0) => break,
                Ok(len) => self.buf_cursor += len,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e)
            }
        }

        self.handle_packet(packets)

    }

    /// Internal function to try reading packet(s) in the internal buffer.
    fn handle_packet(&mut self, packets: &mut Packets) -> io::Result<()> {

        loop {

            // TODO: Handle packet not found in a fully filled buffer.
            let buf = &self.buf[..self.buf_cursor];

            if buf.len() == 0 {
                // No packet received.
                return Ok(());
            }

            let mut cursor = Cursor::new(buf);
            let packet_id = cursor.read_u8()?;

            let packet;

            match packet_id {
                0 => {
                    packet = ServerPacket::KeepAlive;
                }
                1 => {

                    packet = ServerPacket::Login(ServerLoginPacket {
                        protocol_version: cursor.read_java_int()?, 
                        username: cursor.read_java_string(16)?,
                    });

                    // Unused when client connects to server.
                    let _map_seed = cursor.read_java_long()?;
                    let _dimension = cursor.read_java_byte()?;

                }
                2 => {
                    packet = ServerPacket::Handshake(ServerHandshakePacket {
                        username: cursor.read_java_string(16)?
                    });
                }
                3 => {
                    packet = ServerPacket::Chat(ChatPacket { 
                        message: cursor.read_java_string(119)?,
                    });
                }
                7 => todo!("use entity"),
                9 => todo!("respawn"),
                10 => {
                    packet = ServerPacket::PlayerFlying(PlayerFlyingPacket {
                        on_ground: cursor.read_java_boolean()?,
                    });
                }
                11 => {
                    
                    let x = cursor.read_java_double()?;
                    let y = cursor.read_java_double()?;
                    let stance = cursor.read_java_double()?;
                    let z = cursor.read_java_double()?;
                    let on_ground = cursor.read_java_boolean()?;
                    packet = ServerPacket::PlayerPosition(PlayerPositionPacket {
                        pos: DVec3::new(x, y, z),
                        stance,
                        on_ground,
                    });

                }
                12 => {
                    
                    let yaw = cursor.read_java_float()?;
                    let pitch = cursor.read_java_float()?;
                    let on_ground = cursor.read_java_boolean()?;
                    packet = ServerPacket::PlayerLook(PlayerLookPacket {
                        look: Vec2::new(yaw, pitch), 
                        on_ground,
                    });

                }
                13 => {
                    
                    let x = cursor.read_java_double()?;
                    let y = cursor.read_java_double()?;
                    let stance = cursor.read_java_double()?;
                    let z = cursor.read_java_double()?;
                    let yaw = cursor.read_java_float()?;
                    let pitch = cursor.read_java_float()?;
                    let on_ground = cursor.read_java_boolean()?;
                    packet = ServerPacket::PlayerPositionLook(PlayerPositionLookPacket {
                        pos: DVec3::new(x, y, z),
                        look: Vec2::new(yaw, pitch),
                        stance,
                        on_ground,
                    });

                }
                14 => {
                    packet = ServerPacket::PlayerBreakBlock(PlayerBreakBlockPacket {
                        status: cursor.read_java_byte()? as u8,
                        x: cursor.read_java_int()?,
                        y: cursor.read_java_byte()?,
                        z: cursor.read_java_int()?,
                        face: cursor.read_java_byte()? as u8,
                    })
                }
                15 => todo!("place block"),
                16 => todo!("block item switch??"),
                18 => {
                    packet = ServerPacket::EntityAnimation(EntityAnimationPacket {
                        entity_id: cursor.read_java_int()? as u32,
                        animate: cursor.read_java_byte()? as u8,
                    })
                }
                19 => todo!("entity action"),
                27 => todo!("position??"),
                101 => todo!("close window"),
                102 => todo!("click window"),
                106 => todo!("transaction"),
                130 => todo!("update sign"),
                255 => todo!("kick/disconnect"),
                _ => panic!("invalid packet id {packet_id}")
            }

            println!("[SERVER <- {}] {:?}", self.token.0, packet);
            packets.inner.push((ClientId(self.token.0), packet));

            let read_length = cursor.position() as usize;
            drop(cursor);

            // Remove the buffer part that we successfully read.
            self.buf.copy_within(read_length..self.buf_cursor, 0);
            self.buf_cursor -= read_length;

        }

    }

    /// Internal function to write a given packet to this client's stream.
    fn write_packet(&mut self, packet: &ClientPacket) -> io::Result<()> {

        println!("[SERVER -> {}] {:?}", self.token.0, packet);
        
        let stream = &mut self.stream;

        match packet {
            ClientPacket::KeepAlive => stream.write_u8(0)?,
            ClientPacket::Handshake(packet) => {
                stream.write_u8(2)?;
                stream.write_java_string(&packet.server)?;
            }
            ClientPacket::Login(packet) => {
                stream.write_u8(1)?;
                stream.write_java_int(packet.entity_id)?;
                stream.write_java_string("")?; // No username it sent to the client.
                stream.write_java_long(packet.random_seed)?;
                stream.write_java_byte(packet.dimension)?;
            }
            ClientPacket::UpdateTime(packet) => {
                stream.write_u8(4)?;
                stream.write_java_long(packet.time as i64)?;
            }
            ClientPacket::Chat(packet) => {
                stream.write_u8(3)?;
                stream.write_java_string(&packet.message[..packet.message.len().min(199)])?;
            }
            ClientPacket::PlayerSpawnPosition(packet)=> {
                stream.write_u8(6)?;
                stream.write_java_int(packet.pos.x)?;
                stream.write_java_int(packet.pos.y)?;
                stream.write_java_int(packet.pos.z)?;
            }
            ClientPacket::PlayerFlying(packet) => {
                stream.write_u8(10)?;
                stream.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::PlayerPosition(packet) => {
                stream.write_u8(11)?;
                stream.write_java_double(packet.pos.x)?;
                stream.write_java_double(packet.pos.y)?;
                stream.write_java_double(packet.stance)?;
                stream.write_java_double(packet.pos.z)?;
                stream.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::PlayerLook(packet) => {
                stream.write_u8(12)?;
                stream.write_java_float(packet.look.x)?;
                stream.write_java_float(packet.look.y)?;
                stream.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::PlayerPositionLook(packet) => {
                stream.write_u8(13)?;
                stream.write_java_double(packet.pos.x)?;
                stream.write_java_double(packet.pos.y)?;
                stream.write_java_double(packet.stance)?;
                stream.write_java_double(packet.pos.z)?;
                stream.write_java_float(packet.look.x)?;
                stream.write_java_float(packet.look.y)?;
                stream.write_java_boolean(packet.on_ground)?;
            }
            ClientPacket::EntityAnimation(packet) => {
                stream.write_u8(18)?;
                stream.write_java_int(packet.entity_id as i32)?;
                stream.write_java_byte(packet.animate as i8)?;
            }
            ClientPacket::PreChunk(packet) => {
                stream.write_u8(50)?;
                stream.write_java_int(packet.cx)?;
                stream.write_java_int(packet.cz)?;
                stream.write_java_boolean(packet.init)?;
            }
            ClientPacket::MapChunk(packet) => {
                stream.write_u8(51)?;
                stream.write_java_int(packet.x)?;
                stream.write_java_short(packet.y)?;
                stream.write_java_int(packet.z)?;
                stream.write_java_byte((packet.x_size - 1) as i8)?;
                stream.write_java_byte((packet.y_size - 1) as i8)?;
                stream.write_java_byte((packet.z_size - 1) as i8)?;
                stream.write_java_int(packet.compressed_data.len() as i32)?;
                stream.write_all(&packet.compressed_data)?;
            }
            ClientPacket::Disconnect(packet) => {
                stream.write_u8(255)?;
                stream.write_java_string(&packet.reason)?;
            }
        }

        Ok(())

    }

}


/// Different state for readable/writable attributes of a proxy side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FunctionState {
    Init,
    Enabled,
    Closed,
}


/// Return an invalid data io error with specific message.
fn new_invalid_data_err(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}


/// Extension trait with Minecraft-specific packet read methods.
trait ReadPacketExt: Read {

    fn read_java_byte(&mut self) -> io::Result<i8> {
        ReadBytesExt::read_i8(self)
    }

    fn read_java_short(&mut self) -> io::Result<i16> {
        ReadBytesExt::read_i16::<BE>(self)
    }

    fn read_java_int(&mut self) -> io::Result<i32> {
        ReadBytesExt::read_i32::<BE>(self)
    }

    fn read_java_long(&mut self) -> io::Result<i64> {
        ReadBytesExt::read_i64::<BE>(self)
    }

    fn read_java_float(&mut self) -> io::Result<f32> {
        ReadBytesExt::read_f32::<BE>(self)
    }

    fn read_java_double(&mut self) -> io::Result<f64> {
        ReadBytesExt::read_f64::<BE>(self)
    }

    fn read_java_boolean(&mut self) -> io::Result<bool> {
        Ok(self.read_java_byte()? != 0)
    }

    fn read_java_char(&mut self) -> io::Result<char> {
        // FIXME: Read real UTF-16 char.
        Ok(ReadBytesExt::read_u16::<BE>(self)? as u8 as char)
    }

    fn read_java_string(&mut self, max_len: usize) -> io::Result<String> {
        
        let len = self.read_java_short()?;
        if len < 0 {
            return Err(new_invalid_data_err("negative length string"));
        }

        if len as usize > max_len {
            return Err(new_invalid_data_err("excedeed max string length"));
        }

        let mut ret = String::new();
        for _ in 0..len {
            ret.push(self.read_java_char()?);
        }

        Ok(ret)

    }

}

/// Extension trait with Minecraft-specific packet read methods.
trait WritePacketExt: Write {

    fn write_java_byte(&mut self, b: i8) -> io::Result<()> {
        WriteBytesExt::write_i8(self, b)
    }

    fn write_java_short(&mut self, s: i16) -> io::Result<()> {
        WriteBytesExt::write_i16::<BE>(self, s)
    }

    fn write_java_int(&mut self, i: i32) -> io::Result<()> {
        WriteBytesExt::write_i32::<BE>(self, i)
    }

    fn write_java_long(&mut self, l: i64) -> io::Result<()> {
        WriteBytesExt::write_i64::<BE>(self, l)
    }

    fn write_java_float(&mut self, f: f32) -> io::Result<()> {
        WriteBytesExt::write_f32::<BE>(self, f)
    }

    fn write_java_double(&mut self, d: f64) -> io::Result<()> {
        WriteBytesExt::write_f64::<BE>(self, d)
    }

    fn write_java_boolean(&mut self, b: bool) -> io::Result<()> {
        self.write_java_byte(b as i8)
    }

    fn write_java_char(&mut self, c: char) -> io::Result<()> {
        // FIXME: Write real UTF-16 char.
        Ok(WriteBytesExt::write_u16::<BE>(self, c as u16)?)
    }

    fn write_java_string(&mut self, s: &str) -> io::Result<()> {
        
        if s.len() > i16::MAX as usize {
            return Err(new_invalid_data_err("string too big"));
        }
        
        self.write_java_short(s.len() as i16)?;
        for c in s.chars() {
            self.write_java_char(c)?;
        }

        Ok(())

    }

}

impl<R: Read> ReadPacketExt for R {}
impl<W: Write> WritePacketExt for W {}


/// Structure for allocating poll tokens.
pub struct TokenAllocator {
    range: Range<usize>,
    free: Vec<usize>,
}

impl TokenAllocator {

    pub fn new(range: Range<usize>) -> Self {
        Self {
            range,
            free: Vec::new(),
        }
    }

    /// Allocate an available token.
    pub fn alloc(&mut self) -> Option<Token> {
        self.free.pop().or_else(|| self.range.next()).map(Token)
    }

    /// Free a allocated token.
    pub fn free(&mut self, id: Token) {
        self.free.push(id.0);
    }

}


/// A list of received packets that is filled by the protocol packet server.
pub struct Packets {
    inner: Vec<(ClientId, ServerPacket)>,
}

impl Packets {
    
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Clear the packets queue.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Iterate over all received packets in the queue.
    pub fn iter(&self) -> impl Iterator<Item = (ClientId, &ServerPacket)> {
        self.inner.iter().map(|(client_id, packet)| (*client_id, packet))
    }

    /// Iterate and remove all received packets in the queue.
    pub fn drain(&mut self) -> impl Iterator<Item = (ClientId, ServerPacket)> + '_ {
        self.inner.drain(..)
    }

}


/// Represent a client's handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(usize);

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
pub struct DisconnectPacket {
    /// The reason for being kicked or disconnection.
    pub reason: String,
}
