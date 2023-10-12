//! A generic single-threaded poll-based TCP server.


use std::io::{self, Read, Cursor, Write};
use std::collections::HashMap;
use std::net::{SocketAddr, Shutdown};
use std::ops::Range;
use std::time::Duration;

use mio::{Poll, Events, Interest, Token};
use mio::net::{TcpListener, TcpStream};
use mio::event::Event;


/// Internal polling token used for the listening socket.
const LISTENER_TOKEN: Token = Token(0);
/// Size of internal buffers for incoming client's data.
const BUF_SIZE: usize = 1024;


/// A server-bound packet (received and processed by the server).
pub trait TcpServerPacket: Sized {

    /// Read the packet from the writer.
    fn read(read: &mut impl Read) -> io::Result<Self>;

}

/// A client-bound packet (received and processed by the client).
pub trait TcpClientPacket {

    /// Write the packet to the given writer.
    fn write(&self, write: &mut impl Write) -> io::Result<()>;

}


/// The server accepts incoming client connections and parses incoming packets that can
/// be retrieved by polling synchronously.
/// 
/// This server only provides low-level protocol codec, it doesn't answer itself to 
/// packets or manage connection with clients.
pub struct TcpServer {
    /// The inner, actual server.
    inner: Inner,
    /// The events queue when polling.
    events: Events,
}

/// Inner structure, split from the main one to avoid borrow issue with events queue.
struct Inner {
    /// The inner TCP listener.
    listener: TcpListener,
    /// The poll used for event listening TCP events.
    poll: Poll,
    /// The id allocator with use to generate unique polling token.
    token_allocator: TokenAllocator,
    /// Connected clients, mapped to their polling token.
    clients: HashMap<Token, Client>,
}

struct Client {
    /// The client's token.
    token: Token,
    /// The client's stream.
    stream: TcpStream,
    /// The client's remote socket address.
    #[allow(unused)]
    addr: SocketAddr,
    /// Writable state.
    writable: bool,
    /// Readable state.
    readable: bool,
    /// Internal buffer to temporarily stores incoming client's data.
    buf: Box<[u8; BUF_SIZE]>,
    /// Cursor in the receiving buffer.
    buf_cursor: usize,
}

impl TcpServer {

    /// Bind this server's TCP listener to the given address.
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        
        let poll = Poll::new()?;
        let mut listener = TcpListener::bind(addr)?;
        poll.registry().register(&mut listener, LISTENER_TOKEN, Interest::READABLE)?;

        Ok(Self {
            inner: Inner {
                listener,
                poll,
                token_allocator: TokenAllocator::new(1000..10000),
                clients: HashMap::new(),
            },
            events: Events::with_capacity(128),
        })

    }

    /// Poll for incoming packets, the internal packets queue is updated.
    pub fn poll<P>(&mut self, events: &mut Vec<TcpEvent<P>>, timeout: Option<Duration>) -> io::Result<()>
    where
        P: TcpServerPacket,
    {

        self.inner.poll.poll(&mut self.events, timeout)?;

        for event in self.events.iter() {
            match event.token() {
                LISTENER_TOKEN => self.inner.handle_listener()?,
                _ => self.inner.handle_client(event, events),
            }
        }

        Ok(())

    }

    /// Send a packet to a client.
    pub fn send<P>(&mut self, client_id: usize, packet: &P) -> io::Result<()>
    where
        P: TcpClientPacket,
    {
        let client = self.inner.clients.get_mut(&Token(client_id)).unwrap();
        packet.write(&mut client.stream)
    }

}

impl Inner {

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

            let token = self.token_allocator.alloc()
            .expect("failed to allocate polling token");

            self.poll.registry().register(&mut stream, token, Interest::READABLE | Interest::WRITABLE)?;

            self.clients.insert(token, Client {
                token,
                stream,
                addr,
                writable: false,
                readable: false,
                buf: Box::new([0; BUF_SIZE]),
                buf_cursor: 0,
            });

        }

        Ok(())

    }

    /// Internal function to handle a polling event from a client. This function doesn't
    /// generate errors, if errors happen they are pushed as client events.
    fn handle_client<P>(&mut self, event: &Event, events: &mut Vec<TcpEvent<P>>)
    where
        P: TcpServerPacket,
    {

        let token = event.token();
        let client = self.clients.get_mut(&token)
            .expect("invalid client token");

        if event.is_writable() {
            client.writable = true;
        }

        let mut read_error = None;

        if event.is_readable() {
            client.readable = true;
            read_error = client.handle_read(events).err();
        }

        if read_error.is_some() || event.is_write_closed() || event.is_read_closed() {
            let _ = client.stream.shutdown(Shutdown::Both);
            let _ = self.poll.registry().deregister(&mut client.stream);
            let _ = self.clients.remove(&token);
            self.token_allocator.free(token);
            events.push(TcpEvent { client_id: token.0, kind: TcpEventKind::Lost(read_error) });
        }

    }

}

impl Client {

    /// Internal function to handle a readable event on this client's socket.
    fn handle_read<P>(&mut self, events: &mut Vec<TcpEvent<P>>) -> io::Result<()>
    where
        P: TcpServerPacket,
    {

        loop {
            match self.stream.read(&mut self.buf[self.buf_cursor..]) {
                Ok(0) => break,
                Ok(len) => self.buf_cursor += len,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e)
            }
        }

        loop {

            // TODO: Handle packet not found in a fully filled buffer.
            let buf = &self.buf[..self.buf_cursor];

            if buf.len() == 0 {
                // No packet received.
                return Ok(());
            }

            let mut cursor = Cursor::new(buf);
            let packet = P::read(&mut cursor)?;

            events.push(TcpEvent { 
                client_id: self.token.0, 
                kind: TcpEventKind::Packet(packet),
            });

            let read_length = cursor.position() as usize;
            drop(cursor);

            // Remove the buffer part that we successfully read.
            self.buf.copy_within(read_length..self.buf_cursor, 0);
            self.buf_cursor -= read_length;

        }

    }

}


#[derive(Debug)]
pub struct TcpEvent<P: TcpServerPacket> {
    /// The client's token.
    pub client_id: usize,
    /// Kind of events.
    pub kind: TcpEventKind<P>,
}

#[derive(Debug)]
pub enum TcpEventKind<P: TcpServerPacket> {
    /// The client was just accepted.
    Accepted,
    /// The client connection was lost, optionally due to an io error.
    Lost(Option<io::Error>),
    /// The client sent a packet to the server.
    Packet(P),
}


/// Structure for allocating poll tokens.
struct TokenAllocator {
    range: Range<usize>,
    free: Vec<usize>,
}

impl TokenAllocator {

    fn new(range: Range<usize>) -> Self {
        Self {
            range,
            free: Vec::new(),
        }
    }

    /// Allocate an available token.
    fn alloc(&mut self) -> Option<Token> {
        self.free.pop().or_else(|| self.range.next()).map(Token)
    }

    /// Free a allocated token.
    fn free(&mut self, token: Token) {
        self.free.push(token.0);
    }

}
