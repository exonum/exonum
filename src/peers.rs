use std::{io, net, sync, collections};

use mio;
use mio::{TryWrite, TryRead};

use super::message::{Message, MessageHeader, HEADER_SIZE};

// TODO: implement trully continuation reading and writing

// TODO: Use Message type here, not Vec<u8>
pub type OutgoingMessage = sync::Arc<Vec<u8>>;

pub struct IncomingPeer {
    socket: mio::tcp::TcpStream,
    address: net::SocketAddr,
    continuation: Option<MessageHeader>,
}

pub struct OutgoingPeer {
    socket: mio::tcp::TcpStream,
    address: net::SocketAddr,
    queue: collections::VecDeque<OutgoingMessage>,
}

impl IncomingPeer {
    pub fn new(socket: mio::tcp::TcpStream, address: net::SocketAddr)
            -> IncomingPeer {
        IncomingPeer {
            socket: socket,
            address: address,
            continuation: None,
        }
    }

    pub fn socket(&self) -> &mio::tcp::TcpStream {
        &self.socket
    }

    pub fn address(&self) -> &net::SocketAddr {
        &self.address
    }

    fn read_header(&mut self) -> io::Result<Option<MessageHeader>> {
        let mut header = MessageHeader::new();
        match try!(self.socket.try_read(header.as_mut())) {
            None => Ok(None),
            Some(n) => {
                if n != HEADER_SIZE {
                    return Err(io::Error::new(io::ErrorKind::InvalidData,
                                              "Invalid message header"));
                }
                Ok(Some(header))
            }
        }
    }

    pub fn readable(&mut self) -> io::Result<Option<Message>> {
        let header = match self.continuation {
            Some(header) => header,
            None => match try!(self.read_header()) {
                Some(header) => header,
                None => return Ok(None)
            }
        };
        // TODO: data length == 0?
        // TODO: maximum data length?
        let mut buf = vec![0; header.length()];

        match try!(self.socket.try_read(&mut buf)) {
            None | Some(0) => {
                self.continuation = Some(header);
                Ok(None)
            },
            Some(n) => {
                if n != header.length() {
                    return Err(io::Error::new(io::ErrorKind::InvalidData,
                                              "Did not read enough bytes"));
                }
                self.continuation = None;
                Ok(Some(Message::new(header, buf)))
            },
        }
    }
}

impl OutgoingPeer {
    pub fn new(socket: mio::tcp::TcpStream, address: net::SocketAddr)
            -> OutgoingPeer {
        OutgoingPeer {
            socket: socket,
            address: address,
            queue: collections::VecDeque::new(),
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
            match self.socket.try_write(&message) {
                Ok(None) => {
                    self.queue.push_front(message);
                    break
                },
                Ok(Some(n)) => {
                    // TODO: Continuation sending
                    assert_eq!(n, message.len())
                }
                Err(e) => return Err(e)
            }
        }
        // TODO: reregister
        return Ok(())
    }

    pub fn send(&mut self, message: OutgoingMessage) {
        // TODO: capacity overflow
        // TODO: reregister
        self.queue.push_back(message);
    }

    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }
}
