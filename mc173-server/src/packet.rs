//! Packet server for threaded decoding and encoding of packets.

use std::io::{self, Read, Write, Cursor};
use std::net::{SocketAddr, Shutdown};
use std::collections::HashMap;
use std::time::Duration;
use std::thread;

use crossbeam_channel::{bounded, Sender, Receiver, TrySendError, TryRecvError};

use mio::{Poll, Events, Interest, Token};
use mio::net::{TcpListener, TcpStream};
use mio::event::Event;


/// A server-bound packet (received and processed by the server).
pub trait InPacket: Sized {
    /// Read the packet from the writer.
    fn read(read: &mut impl Read) -> io::Result<Self>;
}

/// A client-bound packet (received and processed by the client).
pub trait OutPacket {
    /// Write the packet to the given writer.
    fn write(&self, write: &mut impl Write) -> io::Result<()>;
}


/// A packet server backed by a background thread that do all the hard processing.
pub struct PacketServer<I, O> {
    /// This channels allows sending commands to the thread.
    commands_sender: Sender<ThreadCommand<O>>,
    /// This channels allows received events from the thread.
    events_receiver: Receiver<PacketEvent<I>>,
}

impl<I: InPacket, O: OutPacket> PacketServer<I, O> {

    pub fn bind(addr: SocketAddr) -> io::Result<Self> {

        let poll = Poll::new()?;
        let mut listener = TcpListener::bind(addr)?;
        poll.registry().register(&mut listener, LISTENER_TOKEN, Interest::READABLE)?;

        let (
            commands_sender,
            commands_receiver
        ) = bounded(100);

        let (
            events_sender,
            events_receiver
        ) = bounded(400);

        thread::spawn(move || {

            let thread = Thread::<I, O> {
                commands_receiver,
                events_sender,
                listener,
                poll,
                clients: HashMap::new(),
            };

            thread.run();

        });

        Ok(Self {
            commands_sender,
            events_receiver,
        })

    }

    /// Poll events from this packet server.
    pub fn poll(&self) -> Option<PacketEvent<I>> {
        self.events_receiver.try_recv().ok()
    }

    pub fn kick(&self, client: PacketClient) {
        self.commands_sender.try_send(ThreadCommand::Kick { token: client.token });
    }

    pub fn send(&self, client: PacketClient, packet: O) {
        let _ = self.commands_sender.try_send(ThreadCommand::SingleClientPacket { 
            token: client.token, 
            packet
        });
    }

}

/// A handle to a client produced by a packet server. This handle can be used with a
/// server to send packets to a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PacketClient(Token);

/// An event of the packet
#[derive(Debug)]
pub enum PacketEvent<I> {
    /// A client 
    Accept {
        /// The clent handle that was accepted.
        client: PacketClient,
    },
    /// A packet was received from a client.
    Received {
        client: PacketClient,
        packet: I,
    },
    Lost {
        /// The client handle that was lost.
        client: PacketClient,
        /// Some error if that caused the client to be lost, no error means that the
        /// client was just kicked from the server or closed the connection itself.
        error: Option<io::Error>,
    },
    /// An I/O error that caused the background thread to crash, it's not recoverable.
    Error {
        error: io::Error,
    }
}


/// Internal polling token used for the listening socket.
const LISTENER_TOKEN: Token = Token(0);
/// Size of internal buffers for incoming client's data.
const BUF_SIZE: usize = 1024;

/// Internal server.
struct Thread<I, O> {
    /// This channels allows receiving commands from server and client handles.
    commands_receiver: Receiver<ThreadCommand<O>>,
    /// This channels allows sending events back to the server handle.
    events_sender: Sender<PacketEvent<I>>,
    /// The inner TCP listener.
    listener: TcpListener,
    /// The poll used for event listening TCP events.
    poll: Poll,
    /// Connected clients, mapped to their polling token.
    clients: HashMap<Token, ThreadClient>,
}

struct ThreadClient {
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

impl<I: InPacket, O: OutPacket> Thread<I, O> {

    /// Run the thread until termination or critical error.
    fn run(mut self) {

        let mut events = Events::with_capacity(100);

        loop {

            self.poll(&mut events);
            
            match self.commands_receiver.try_recv() {
                Ok(ThreadCommand::Kick { token }) => {
                    self.handle_client_close(token, None);
                }
                Ok(ThreadCommand::SingleClientPacket { token, packet }) => {
                    self.handle_client_send(token, packet);
                },
                Err(TryRecvError::Empty) => {},
                Err(TryRecvError::Disconnected) => {
                    // If the commands receiver channel is disconnected, all senders 
                    // should be dead, so we know that all handles are dead, we can 
                    // terminate the thread.
                    break;
                }
            }

        }

    }

    /// Events polling of internal listener and clients, if a polling or listener error
    /// happens, it is returned, client errors are sent separately and should not crash
    /// the main thread.
    fn poll(&mut self, events: &mut Events) -> io::Result<()> {

        self.poll.poll(events, Duration::as_millis(1))?;

        for event in events.iter() {
            match event.token() {
                LISTENER_TOKEN => self.handle_listener()?,
                _ => self.handle_client(event),
            }
        }

        Ok(())

    }

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

            self.clients.insert(token, ThreadClient {
                token,
                stream,
                addr,
                writable: false,
                readable: false,
                buf: Box::new([0; BUF_SIZE]),
                buf_cursor: 0,
            });

            let _ = self.events_sender.try_send(PacketEvent::Accept { 
                client: PacketClient(token),
            });

        }

        Ok(())

    }

    /// Internal function to handle a polling event from a client. This function doesn't
    /// generate errors, if errors happen they are pushed as client events.
    fn handle_client(&mut self, event: &Event) {

        let token = event.token();
        let client = self.clients.get_mut(&token)
            .expect("invalid client token");

        if event.is_writable() {
            client.writable = true;
        }

        if event.is_readable() {
            client.readable = true;
            if !self.handle_client_readable(token) {
                return;
            }
        }

        if event.is_write_closed() || event.is_read_closed() {
            self.handle_client_close(token, None);
        }

    }

    /// Handle a readable client event. This return false if the global loop should stop.
    fn handle_client_readable(&mut self, token: Token) -> bool {

        loop {
            match self.stream.read(&mut self.buf[self.buf_cursor..]) {
                Ok(0) => break,
                Ok(len) => self.buf_cursor += len,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    self.handle_client_close(token, Some(e));
                    return true;
                }
            }
        }

        loop {

            // TODO: Handle packet not found in a fully filled buffer.
            let buf = &self.buf[..self.buf_cursor];

            if buf.len() == 0 {
                break; // No packet received.
            }

            let mut cursor = Cursor::new(buf);
            let packet = match I::read(&mut cursor) {
                Ok(packet) => packet,
                Err(e) => {
                    self.handle_client_close(token, Some(e));
                    return true;
                }
            };

            let event = PacketEvent::Received { 
                client: PacketClient(token),
                packet,
            };

            match self.events_sender.try_send(event) {
                Ok(()) => {},
                Err(TrySendError::Full(_)) => {
                    // If the events queue is full, we just close this client with an 
                    // abort because the polling
                    self.handle_client_close(token, Some(new_io_abort_error("")))
                }
                Err(TrySendError::Disconnected(_)) => {
                    // If the events sender is disconnected, this means that there are no
                    // handle to send to, just stop the thread loop.
                    return false;
                }
            }

            let read_length = cursor.position() as usize;
            drop(cursor);

            // Remove the buffer part that we successfully read.
            self.buf.copy_within(read_length..self.buf_cursor, 0);
            self.buf_cursor -= read_length;

        }

        true

    }

    fn handle_client_close(&mut self, token: Token, error: Option<io::Error>) {
        
        let mut client = self.clients.remove(&token)
            .expect("invalid client token");

        let _ = client.stream.shutdown(Shutdown::Both);
        let _ = self.poll.registry().deregister(&mut client.stream);
        
        self.events_sender.send(PacketEvent::Lost { 
            client: PacketClient(token),
            error,
        });

    }

    fn handle_client_send(&mut self, token: Token, packet: O) {

        let client = self.clients.get_mut(&token)
            .expect("invalid client token");

        if let Err(e) = packet.write(&mut client.stream) {
            self.handle_client_close(token, Some(e));
        }

    }

}

enum ThreadCommand<O> {
    /// Kick a client.
    Kick {
        token: Token,
    },
    /// Send a single packet to a client.
    SingleClientPacket {
        /// The client's token.
        token: Token,
        /// The out packet to send.
        packet: O,
    }
}


fn new_io_abort_error(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionAborted, message)
}
