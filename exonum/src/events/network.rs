use std::io;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::cmp::min;

pub use mio::{EventSet, PollOpt, Token};
use mio::Timeout as MioTimeout;
use mio::tcp::{TcpListener, TcpStream};
use mio::util::Slab;

use super::connection::{Connection};
use super::{Timeout, InternalTimeout, EventLoop};

use super::super::messages::{MessageBuffer, RawMessage};

pub type PeerId = Token;

const SERVER_ID: PeerId = Token(1);
const RECONNECT_GROW_FACTOR: f32 = 2.0;

#[derive(Debug)]
pub enum Output {
    Data(MessageBuffer),
    Connected(SocketAddr),
    Disconnected(SocketAddr)
}

#[derive(Debug, Clone, Copy)]
pub struct NetworkConfiguration {
    pub listen_address: SocketAddr,
    pub max_connections: usize, // TODO: think more about config parameters
    pub tcp_nodelay: bool,
    pub tcp_keep_alive: Option<u32>,
    pub tcp_reconnect_timeout: u64,
    pub tcp_reconnect_timeout_max: u64,
}

pub struct Network {
    config: NetworkConfiguration,
    listener: Option<TcpListener>,
    connections: Slab<Connection>,
    addresses: HashMap<SocketAddr, PeerId>,
    reconnects: HashMap<SocketAddr, MioTimeout>,
}

impl Network {
    pub fn with_config(config: NetworkConfiguration) -> Network {
        Network {
            config: config,
            listener: None,
            connections: Slab::new_starting_at(Token(2), config.max_connections),
            addresses: HashMap::new(),
            reconnects: HashMap::new(),
        }
    }

    pub fn address(&self) -> &SocketAddr {
        &self.config.listen_address
    }

    pub fn bind(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        if let Some(_) = self.listener {
            return Err(io::Error::new(io::ErrorKind::Other, "Already binded"));
        }
        let listener = TcpListener::bind(&self.config.listen_address)?;
        let r = event_loop.register(&listener, SERVER_ID, EventSet::readable(), PollOpt::edge());
        match r {
            Ok(()) => {
                self.listener = Some(listener);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    // TODO: Use ticks for fast reregistering sockets
    // TODO: Implement Connections collection with (re)registering

    pub fn io(&mut self,
              event_loop: &mut EventLoop,
              id: PeerId,
              set: EventSet)
              -> io::Result<Option<Output>> {
        if id == SERVER_ID {
            // Accept new connections
            // FIXME: Fail-safe accepting of new connections?
            let pair = match self.listener {
                Some(ref listener) => listener.accept()?,
                None => None,
            };
            if let Some((socket, address)) = pair {
                let peer = Connection::new(socket, address);
                self.add_connection(event_loop, peer)?;

                trace!("{}: Accepted incoming connection from {} id: {}",
                       self.address(),
                       address,
                       id.0);
            }
            return Ok(None);
        }

        if !self.connections.contains(id) {
            return Ok(None);
        }

        if set.is_error() {
            let address = *self.connections[id].address();

            trace!("{}: connection with {} closed with error",
                  self.address(),
                  address);

            self.remove_connection(id);
            return Ok(None);
        }

        if set.is_hup() {
            let address = *self.connections[id].address();

            trace!("{}: connection with addr {} closed",
                   self.address(),
                   self.connections[id].address());

            self.remove_connection(id);
            if self.reconnects.contains_key(&address) {
                return Ok(None);
            }
            return Ok(Some(Output::Disconnected(address)));
        }

        if set.is_writable() {
            let address = *self.connections[id].address();

            trace!("{}: Socket is writable {} id: {}",
                   self.address(),
                   address,
                   id.0);

            // Write data into socket
            if let Err(e) = self.connections[id].try_write() {
                warn!("{}: An error when write to socket {} occured, error: {}",
                      self.address(),
                      address,
                      e);
                self.remove_connection(id);
                return Ok(None);
            }
            self.reregister_connection(event_loop, id, PollOpt::edge())?;

            return Ok(self.mark_connected(event_loop, id))
        }

        if set.is_readable() {
            let address = *self.connections[id].address();
            self.mark_connected(event_loop, id);

            trace!("{}: Socket is readable {} id: {}",
                   self.address(),
                   address,
                   id.0);

            return match self.connections[id].try_read() {
                Ok(Some(buffer)) => Ok(Some(Output::Data(buffer))),
                Ok(None) => Ok(None),
                Err(e) => {
                    warn!("{}: An error when read from socket {} occured, error: {}",
                          self.address(),
                          address,
                          e);
                    self.remove_connection(id);
                    Ok(None)
                }
            };
        }
        // FIXME: Can we be here?
        Ok(None)
    }

    pub fn tick(&mut self, _: &mut EventLoop) {}

    pub fn get_peer(&mut self,
                    address: &SocketAddr)
                    -> io::Result<PeerId> {
        if let Some(id) = self.addresses.get(address) {
            return Ok(*id);
        };
        Err(io::Error::new(io::ErrorKind::Other, format!("Peer not found {}", address)))
    }

    pub fn send_to(&mut self,
                   event_loop: &mut EventLoop,
                   address: &SocketAddr,
                   message: RawMessage) {
        let r = match self.get_peer(address) {
            Ok(id) => {
                self.connections[id]
                    .send(message)
                    .and_then(|_| {
                        event_loop.reregister(self.connections[id].socket(),
                                        id,
                                        self.connections[id].interest(),
                                        PollOpt::edge())?;
                        self.mark_connected(event_loop, id);
                        Ok(())
                    })
                    .or_else(|e| {
                        self.remove_connection(id);
                        Err(e)
                    })
            }
            Err(e) => Err(e),
        };

        if let Err(e) = r {
            warn!("{}: An error occured when try to send a message (address: {}, error: {:?})",
                  self.address(),
                  address,
                  e);
            return;
        }
    }

    pub fn connect(&mut self, event_loop: &mut EventLoop, address: &SocketAddr) -> io::Result<()> {
        if !self.addresses.contains_key(address) {
            let peer = Connection::new(TcpStream::connect(address)?, *address);
            let id = self.add_connection(event_loop, peer)?;

            self.try_reconnect_addr(event_loop, *address)?;

            trace!("{}: Establish connection with {}, id: {}",
                self.address(),
                address,
                id.0);
        }
        Ok(())
    }

    pub fn handle_timeout(&mut self, event_loop: &mut EventLoop, timeout: InternalTimeout) {
        match timeout {
            InternalTimeout::Reconnect(addr, delay) => {
                if self.reconnects.contains_key(&addr) {
                    if let Err(e) = self.connect(event_loop, &addr) {
                        error!("{}: Unable to create connection to addr {}, error {:?}",
                               self.address(),
                               addr,
                               e);
                    }

                    let delay = min((delay as f32 * RECONNECT_GROW_FACTOR) as u64, self.config.tcp_reconnect_timeout_max);
                    // TODO Fail-safe timeout error handling
                    self.add_reconnect_timeout(event_loop, addr, delay).unwrap();
                    trace!("Try to reconnect with delay {}", delay);
                }
            }
        }
    }

    fn remove_connection(&mut self, id: Token) {
        let address = *self.connections[id].address();
        self.addresses.remove(&address);
        self.connections.remove(id);
    }

    fn try_reconnect_addr(&mut self,
                          event_loop: &mut EventLoop,
                          address: SocketAddr)
                          -> io::Result<()> {
        if !self.reconnects.contains_key(&address) {
            let delay = self.config.tcp_reconnect_timeout;
            return self.add_reconnect_timeout(event_loop, address, delay);
        }
        Ok(())
    }

    fn add_reconnect_timeout(&mut self,
                             event_loop: &mut EventLoop,
                             address: SocketAddr,
                             delay: u64)
                             -> io::Result<()> {
        let reconnect = Timeout::Internal(InternalTimeout::Reconnect(address, delay));
        let timeout = event_loop.timeout_ms(reconnect, delay)
            .map_err(|e| {
                io::Error::new(io::ErrorKind::Other, format!("A mio error occured {:?}", e))
            })?;
        self.reconnects.insert(address, timeout);

        trace!("Add reconnect timeout {}", delay);
        Ok(())
    }

    fn add_connection(&mut self,
                      event_loop: &mut EventLoop,
                      mut connection: Connection)
                      -> io::Result<PeerId> {
        self.configure_stream(connection.socket_mut())?;
        let address = *connection.address();
        let id = self.connections
            .insert(connection)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Maximum connections"))?;
        self.addresses.insert(address, id);

        self.register_connection(event_loop, id, PollOpt::edge())?;
        Ok(id)
    }

    fn mark_connected(&mut self, event_loop: &mut EventLoop, id: Token) -> Option<Output> {
        let address = *self.connections[id].address();
        self.clear_reconnect_request(event_loop, &address)
    }

    fn clear_reconnect_request(&mut self, event_loop: &mut EventLoop, addr: &SocketAddr) -> Option<Output> {
        if let Some(timeout) = self.reconnects.remove(addr) {
            event_loop.clear_timeout(timeout);
            return Some(Output::Connected(*addr));
        }
        None
    }

    fn register_connection(&mut self,
                           event_loop: &mut EventLoop,
                           id: Token,
                           opts: PollOpt)
                           -> io::Result<()> {
        event_loop.register(self.connections[id].socket(),
                      id,
                      self.connections[id].interest() | EventSet::writable(),
                      opts)
            .or_else(|e| {
                self.remove_connection(id);
                Err(e)
            })
    }

    fn reregister_connection(&mut self,
                             event_loop: &mut EventLoop,
                             id: Token,
                             opts: PollOpt)
                             -> io::Result<()> {
        event_loop.reregister(self.connections[id].socket(),
                        id,
                        self.connections[id].interest(),
                        opts)
            .or_else(|e| {
                self.remove_connection(id);
                Err(e)
            })
    }

    fn configure_stream(&self, stream: &mut TcpStream) -> io::Result<()> {
        stream.set_keepalive(self.config.tcp_keep_alive)?;
        stream.set_nodelay(self.config.tcp_nodelay)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {}
