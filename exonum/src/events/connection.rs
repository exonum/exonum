use std::io;
use std::mem::swap;
use std::net::SocketAddr;
use std::collections::VecDeque;

use byteorder::{ByteOrder, LittleEndian};

use mio::tcp::TcpStream;
use mio::{TryWrite, TryRead, EventSet};

use super::super::messages::{RawMessage, MessageBuffer, HEADER_SIZE};

const MAX_MESSAGE_LEN: usize = 1024 * 1024; //1 MB

#[derive(Debug, PartialEq)]
pub struct MessageReader {
    raw: Vec<u8>,
    position: usize,
}

impl MessageReader {
    pub fn empty() -> MessageReader {
        MessageReader {
            raw: vec![0; HEADER_SIZE],
            position: 0,
        }
    }

    pub fn read_finished(&self) -> bool {
        self.position >= HEADER_SIZE && self.position == self.total_len()
    }

    pub fn actual_len(&self) -> usize {
        self.raw.len()
    }

    pub fn total_len(&self) -> usize {
        LittleEndian::read_u32(&self.raw[4..8]) as usize
    }

    pub fn allocate(&mut self) -> io::Result<()> {
        let size = self.total_len();
        if size > MAX_MESSAGE_LEN {
            Err(io::Error::new(io::ErrorKind::Other,
                               format!("Received message is too long: {}", size)))
        } else {
            self.raw.resize(size, 0);
            Ok(())
        }
    }

    pub fn into_raw(self) -> MessageBuffer {
        MessageBuffer::from_vec(self.raw)
    }

    pub fn read(&mut self, socket: &mut TcpStream) -> io::Result<Option<usize>> {
        // FIXME: we shouldn't read more than HEADER_SIZE or total_length()
        // TODO: read into growable Vec, not into [u8]
        if self.position == HEADER_SIZE && self.actual_len() == HEADER_SIZE {
            self.allocate()?;
        }
        let pos = self.position;
        let r = socket.try_read(&mut self.raw[pos..])?;
        if let Some(n) = r {
            self.position += n;
        }
        Ok(r)
    }
}

pub struct MessageWriter {
    queue: VecDeque<RawMessage>,
    position: usize,
}

impl MessageWriter {
    pub fn empty() -> MessageWriter {
        MessageWriter {
            queue: VecDeque::new(),
            position: 0,
        }
    }

    pub fn write(&mut self, socket: &mut TcpStream) -> io::Result<()> {
        // TODO: use try_write_buf
        while let Some(message) = self.queue.front().cloned() {
            let buf = message.as_ref().as_ref();
            if buf.len() > MAX_MESSAGE_LEN {
                self.queue.pop_front();
                return Err(io::Error::new(io::ErrorKind::Other,
                                          "Attempted to send too long message. It will be \
                                           dropped."));
            }
            match socket.try_write(&buf[self.position..])? {
                None | Some(0) => {
                    break;
                }
                Some(n) => {
                    self.position += n;
                    if n == message.len() {
                        self.queue.pop_front();
                        self.position = 0;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }
}

pub trait Connection {
    fn socket(&self) -> &TcpStream;
    fn address(&self) -> &SocketAddr;
    fn interest(&self) -> EventSet;
}

pub struct IncomingConnection {
    socket: TcpStream,
    address: SocketAddr,
    reader: MessageReader,
}

pub struct OutgoingConnection {
    socket: TcpStream,
    address: SocketAddr,
    writer: MessageWriter,
}

impl IncomingConnection {
    pub fn new(socket: TcpStream, address: SocketAddr) -> IncomingConnection {
        IncomingConnection {
            socket: socket,
            address: address,

            reader: MessageReader::empty(),
        }
    }

    pub fn try_read(&mut self) -> io::Result<Option<MessageBuffer>> {
        // TODO: raw length == 0?
        // TODO: maximum raw length?
        loop {
            match self.reader.read(&mut self.socket)? {
                None | Some(0) => return Ok(None),
                Some(_) => {
                    if self.reader.read_finished() {
                        let mut raw = MessageReader::empty();
                        swap(&mut raw, &mut self.reader);
                        return Ok(Some(raw.into_raw()));
                    }
                }
            }
        }
    }
}

impl OutgoingConnection {
    pub fn new(socket: TcpStream, address: SocketAddr) -> OutgoingConnection {
        OutgoingConnection {
            socket: socket,
            address: address,
            writer: MessageWriter::empty(),
        }
    }

    pub fn try_write(&mut self) -> io::Result<()> {
        // TODO: reregister
        self.writer.write(&mut self.socket).or_else(|e| {
            match e.kind() {
                io::ErrorKind::WouldBlock |
                io::ErrorKind::WriteZero => {
                    warn!("Unable to write to socket {}, socket is blocked",
                          self.address);
                    Ok(())
                }
                _ => Err(e),
            }
        })
    }

    pub fn send(&mut self, message: RawMessage) -> io::Result<()> {
        // TODO: capacity overflow
        // TODO: reregister
        self.writer.queue.push_back(message);
        // TODO proper test that we can write immediately
        self.try_write()
    }

    pub fn is_idle(&self) -> bool {
        self.writer.is_idle()
    }
}

impl Connection for IncomingConnection {
    fn socket(&self) -> &TcpStream {
        &self.socket
    }

    fn address(&self) -> &SocketAddr {
        &self.address
    }

    fn interest(&self) -> EventSet {
        EventSet::hup() | EventSet::error() | EventSet::readable()
    }
}
impl Connection for OutgoingConnection {
    fn socket(&self) -> &TcpStream {
        &self.socket
    }

    fn address(&self) -> &SocketAddr {
        &self.address
    }

    fn interest(&self) -> EventSet {
        // readable interest is needed to receive hup event on macos if socket closed by other side.
        let mut set = EventSet::readable() | EventSet::hup() | EventSet::error();
        if !self.is_idle() {
            set = set | EventSet::writable();
        }
        set
    }
}
