//! Network protocol definition and abstraction for interacting with clients.

use std::io::{self, Read, Cursor, Write};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::ops::Range;

use byteorder::{ReadBytesExt, WriteBytesExt, BE};

use mio::{Poll, Events, Interest, Token};
use mio::net::{TcpListener, TcpStream};
use mio::event::Event;


#[derive(Debug, Clone)]
pub struct Packet<I> {
    /// The client id 
    pub client: usize,
    /// The inner packet event.
    pub event: I,
}

impl<I> Packet<I> {

    /// Return a new packet with the same client but with a new kind of event, typically
    /// use to answer to a [`ServerEvent`] with a [`ClientEvent`].
    pub fn answer<J>(&self, event: J) -> Packet<J> {
        Packet { client: self.client, event }
    }

}

/// A packet received by the server.
#[derive(Debug, Clone)]
pub enum ServerEvent {
    /// Used for TCP keep alive.
    KeepAlive,
    /// Sent by the client to handshake.
    Handshake {
        /// Username of the player trying to connect.
        username: String,
    },
    Login {
        protocol_version: i32,
        username: String,
    }
}

/// A packet to send to a client.
#[derive(Debug, Clone)]
pub enum ClientEvent {
    /// Used for TCP keep alive.
    KeepAlive,
    /// Answered by the server when the client wants to handshake.
    Handshake {
        /// Server identifier that accepted the player handshake. This equals '-' in 
        /// offline mode.
        server: String,
    },
    Login {
        /// The entity id of the player being connected.
        entity_id: i32,
        /// A random seed sent to the player.
        random_seed: i64,
        /// The dimension the player is connected to.
        dimension: i8,
    },
    Kick {
        /// The reason for being kicked.
        reason: String,
    }
}


/// Internal polling token used for the listening socket.
const LISTENER_TOKEN: Token = Token(0);
/// Size of internal buffers for incoming client's data.
const BUF_SIZE: usize = 4096;

/// The server accepts incoming client connections and parses packets for events.
pub struct PacketServer {
    /// The inner, actual server.
    inner: InnerServer,
    /// The events queue when polling.
    events: Events,
}

impl PacketServer {

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

    /// Poll for incoming events on this
    pub fn poll(&mut self, events: &mut Vec<Packet<ServerEvent>>) -> io::Result<()> {

        self.inner.poll.poll(&mut self.events, None)?;

        for event in self.events.iter() {
            match event.token() {
                LISTENER_TOKEN => self.inner.handle_listener()?,
                _ => self.inner.handle_client(event, events)?,
            }
        }

        Ok(())

    }

    /// Send a packet to a client.
    pub fn send(&mut self, packet: Packet<ClientEvent>) -> io::Result<()> {

        let client = self.inner.clients.get_mut(&Token(packet.client)).unwrap();
        assert_eq!(client.writable, FunctionState::Enabled);

        client.write_packet(&packet.event)

    }

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

impl InnerServer {

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

    fn handle_client(&mut self, event: &Event, packets: &mut Vec<Packet<ServerEvent>>) -> io::Result<()> {

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
    fn handle_read(&mut self, packets: &mut Vec<Packet<ServerEvent>>) -> io::Result<()> {

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
    fn handle_packet(&mut self, packets: &mut Vec<Packet<ServerEvent>>) -> io::Result<()> {

        loop {

            let buf = &self.buf[..self.buf_cursor];

            if buf.len() == 0 {
                // No packet received.
                return Ok(());
            }

            let mut cursor = Cursor::new(buf);
            let packet_id = cursor.read_u8()?;

            let event;

            match packet_id {
                0 => {
                    event = ServerEvent::KeepAlive;
                }
                1 => {
                    event = ServerEvent::Login { 
                        protocol_version: cursor.read_java_int()?, 
                        username: cursor.read_java_string(16)?,
                    };
                    // Unused when client connects to server.
                    let _map_seed = cursor.read_java_long()?;
                    let _dimension = cursor.read_java_byte()?;
                }
                2 => {
                    event = ServerEvent::Handshake {
                        username: cursor.read_java_string(16)?,
                    };
                }
                3 => todo!("chat"),
                7 => todo!("use entity"),
                9 => todo!("respawn"),
                10 => todo!("flying"),
                11 => todo!("player position"),
                12 => todo!("player look"),
                13 => todo!("player position/look"),
                14 => todo!("break block"),
                15 => todo!("place block"),
                16 => todo!("block item switch??"),
                18 => todo!("animation"),
                19 => todo!("entity action"),
                27 => todo!("position??"),
                101 => todo!("close window"),
                102 => todo!("click window"),
                106 => todo!("transaction"),
                130 => todo!("update sign"),
                255 => todo!("kick/disconnect"),
                _ => panic!("invalid packet id {packet_id}")
            }

            let read_length = cursor.position() as usize;
            drop(cursor);

            self.buf.copy_within(read_length..self.buf_cursor, 0);
            self.buf_cursor -= read_length;

            packets.push(Packet { client: self.token.0, event });
            
        }

    }

    /// Internal function to write a given packet to this client's stream.
    fn write_packet(&mut self, packet: &ClientEvent) -> io::Result<()> {
        
        let stream = &mut self.stream;

        match *packet {
            ClientEvent::KeepAlive => stream.write_u8(0)?,
            ClientEvent::Handshake { ref server } => {
                stream.write_u8(2)?;
                stream.write_java_string(&server)?;
            }
            ClientEvent::Login { entity_id, random_seed, dimension } => {
                stream.write_u8(1)?;
                stream.write_java_int(entity_id)?;
                stream.write_java_string("")?; // No username it sent.
                stream.write_java_long(random_seed)?;
                stream.write_java_byte(dimension)?;
            }
            ClientEvent::Kick { ref reason } => {
                stream.write_u8(255)?;
                stream.write_java_string(&reason)?;
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
