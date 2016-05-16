use std::{io, net, mem, collections};

use mio;
use mio::{TryWrite, TryRead};

use super::message::{Message, RawMessage, HEADER_SIZE};

pub struct IncomingConnection {
    socket: mio::tcp::TcpStream,
    address: net::SocketAddr,
    raw: RawMessage,
    position: usize,
}

pub struct OutgoingConnection {
    socket: mio::tcp::TcpStream,
    address: net::SocketAddr,
    queue: collections::VecDeque<Message>,
    position: usize,
}

impl IncomingConnection {
    pub fn new(socket: mio::tcp::TcpStream, address: net::SocketAddr)
            -> IncomingConnection {
        IncomingConnection {
            socket: socket,
            address: address,
            raw: RawMessage::empty(),
            position: 0,
        }
    }

    pub fn socket(&self) -> &mio::tcp::TcpStream {
        &self.socket
    }

    pub fn address(&self) -> &net::SocketAddr {
        &self.address
    }

    fn read(&mut self) -> io::Result<Option<usize>> {
        if self.position == HEADER_SIZE &&
           self.raw.actual_length() == HEADER_SIZE {
            self.raw.allocate_payload();
        }
        self.socket.try_read(&mut self.raw.as_mut()[self.position..])
    }

    pub fn readable(&mut self) -> io::Result<Option<RawMessage>> {
        // TODO: raw length == 0?
        // TODO: maximum raw length?
        loop {
            match self.read()? {
                None | Some(0) => return Ok(None),
                Some(n) => {
                    self.position += n;
                    if self.position >= HEADER_SIZE &&
                       self.position == self.raw.total_length() {
                        let mut raw = RawMessage::empty();
                        mem::swap(&mut raw, &mut self.raw);
                        self.position = 0;
                        return Ok(Some(raw))
                    }
                }
            }
        }
    }
}

impl OutgoingConnection {
    pub fn new(socket: mio::tcp::TcpStream, address: net::SocketAddr)
            -> OutgoingConnection {
        OutgoingConnection {
            socket: socket,
            address: address,
            queue: collections::VecDeque::new(),
            position: 0
        }
    }

    pub fn socket(&self) -> &mio::tcp::TcpStream {
        &self.socket
    }

    pub fn address(&self) -> &net::SocketAddr {
        &self.address
    }

    pub fn writable(&mut self) -> io::Result<()> {
        // TODO: use try_write_buf
        while let Some(message) = self.queue.pop_front() {
            match self.socket.try_write(message.as_ref().as_ref())? {
                None | Some(0) => {
                    self.queue.push_front(message);
                    break
                },
                Some(n) => {
                    self.position += n;
                    if n == message.actual_length() {
                        self.position = 0;
                    }
                }
            }
        }
        // TODO: reregister
        return Ok(())
    }

    pub fn send(&mut self, message: Message) {
        // TODO: capacity overflow
        // TODO: reregister
        self.queue.push_back(message);
    }

    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }
}
