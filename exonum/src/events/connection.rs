use std::io;
use std::mem::swap;
use std::net::SocketAddr;
use std::collections::VecDeque;

use byteorder::{ByteOrder, LittleEndian};

use mio::tcp::TcpStream;
use mio::{TryWrite, TryRead, EventSet};

use super::super::messages::{RawMessage, MessageBuffer, HEADER_SIZE};

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

    pub fn allocate(&mut self) {
        let size = self.total_len();
        self.raw.resize(size, 0);
    }

    pub fn into_raw(self) -> MessageBuffer {
        MessageBuffer::from_vec(self.raw)
    }

    pub fn read(&mut self, socket: &mut TcpStream) -> io::Result<Option<usize>> {
        // FIXME: we shouldn't read more than HEADER_SIZE or total_length()
        // TODO: read into growable Vec, not into [u8]
        if self.position == HEADER_SIZE && self.actual_len() == HEADER_SIZE {
            self.allocate();
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

pub struct Connection {
    socket: TcpStream,
    address: SocketAddr,

    reader: MessageReader,
    writer: MessageWriter,
}

impl Connection {
    pub fn new(socket: TcpStream, address: SocketAddr) -> Connection {
        Connection {
            socket: socket,
            address: address,

            reader: MessageReader::empty(),
            writer: MessageWriter::empty(),
        }
    }

    pub fn socket(&self) -> &TcpStream {
        &self.socket
    }

    pub fn socket_mut(&mut self) -> &mut TcpStream {
        &mut self.socket
    }

    pub fn address(&self) -> &SocketAddr {
        &self.address
    }

    pub fn writable(&mut self) -> io::Result<()> {
        // TODO: reregister
        self.writer.write(&mut self.socket)
    }

    pub fn readable(&mut self) -> io::Result<Option<MessageBuffer>> {
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

    pub fn send(&mut self, message: RawMessage) -> io::Result<()> {
        // TODO: capacity overflow
        // TODO: reregister
        self.writer.queue.push_back(message);
        // TODO proper test that we can write immediately
        self.writable().or_else(|e| {
            warn!("Unable to write to socket {}, error is {:?}", self.address, e);
            Ok(())
        })
    }

    pub fn is_idle(&self) -> bool {
        self.writer.is_idle()
    }

    pub fn interest(&self) -> EventSet {
        let mut set = EventSet::hup() | EventSet::error() | EventSet::readable();
        if !self.writer.is_idle() {
            set = set | EventSet::writable();
        }
        set
    }
}
