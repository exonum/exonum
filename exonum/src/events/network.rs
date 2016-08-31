use std::io;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::cmp::min;

pub use mio::{EventSet, PollOpt, Token};
use mio::Timeout as MioTimeout;
use mio::tcp::{TcpListener, TcpStream};
use mio::util::Slab;

use super::connection::{Connection, Direction};
use super::{Timeout, InternalTimeout, EventLoop};

use super::super::messages::{MessageBuffer, RawMessage};

pub type PeerId = Token;

const SERVER_ID: PeerId = Token(1);
const RECONNECT_GROW_FACTOR: f32 = 2.0;

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
              -> io::Result<Option<MessageBuffer>> {
        if id == SERVER_ID {
            // Accept new connections
            // FIXME: Fail-safe accepting of new connections?
            let pair = match self.listener {
                Some(ref listener) => listener.accept()?,
                None => None,
            };
            if let Some((socket, address)) = pair {
                let peer = Connection::new(socket, address, Direction::Incoming);
                self.add_connection(event_loop, peer)?;

                trace!("{}: Accepted incoming connection from {} id: {}",
                       self.address(),
                       address,
                       id.0);

                // TODO send event to node
            }
            return Ok(None);
        }

        if set.is_error() {
            trace!("{}: connection with {} closed with error",
                  self.address(),
                  self.connections[id].address());

            self.try_reconnect(event_loop, id);
            return Ok(None);
        }

        if set.is_hup() {
            trace!("{}: connection with addr {} closed",
                   self.address(),
                   self.connections[id].address());

            self.try_reconnect(event_loop, id); // for debug only
            //self.remove_connection(id);
            return Ok(None);
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
                self.try_reconnect(event_loop, id);
                return Ok(None);
            }
            self.reregister_connection(event_loop, id, PollOpt::edge())?;

            self.mark_connected(event_loop, id);
            return Ok(None);
        }

        if set.is_readable() {
            let address = *self.connections[id].address();
            self.mark_connected(event_loop, id);

            trace!("{}: Socket is readable {} id: {}",
                   self.address(),
                   address,
                   id.0);

            return match self.connections[id].try_read() {
                Ok(buffer) => Ok(buffer),
                Err(e) => {
                    warn!("{}: An error when read from socket {} occured, error: {}",
                          self.address(),
                          address,
                          e);
                    self.try_reconnect(event_loop, id);
                    Ok(None)
                }
            };
        }
        // FIXME: Can we be here?
        Ok(None)
    }

    pub fn tick(&mut self, _: &mut EventLoop) {}

    pub fn get_peer(&mut self,
                    event_loop: &mut EventLoop,
                    address: &SocketAddr)
                    -> io::Result<PeerId> {
        if let Some(id) = self.addresses.get(address) {
            return Ok(*id);
        };

        let peer = Connection::new(TcpStream::connect(address)?, *address, Direction::Outgoing);
        let id = self.add_connection(event_loop, peer)?;

        trace!("{}: Establish connection with {}, id: {}",
               self.address(),
               address,
               id.0);
        Ok(id)
    }

    pub fn send_to(&mut self,
                   event_loop: &mut EventLoop,
                   address: &SocketAddr,
                   message: RawMessage) {
        if self.reconnects.contains_key(address) {
            info!("{}: An attempt to send data socket {} which is not online.",
                  self.address(),
                  address);
            return;
        }

        let r = match self.get_peer(event_loop, address) {
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
                        self.try_reconnect(event_loop, id);
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

    pub fn handle_timeout(&mut self, event_loop: &mut EventLoop, timeout: InternalTimeout) {
        match timeout {
            InternalTimeout::Reconnect(addr, delay) => {
                if self.reconnects.contains_key(&addr) {
                    if let Err(e) = self.get_peer(event_loop, &addr) {
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

    fn try_reconnect(&mut self, event_loop: &mut EventLoop, id: Token) {
        if self.connections[id].direction() == &Direction::Outgoing {
            let addr = *self.connections[id].address();
            match self.try_reconnect_addr(event_loop, addr) {
                Err(e) => {
                    error!("{}: An error during reconnect occured: {:?}",
                           self.address(),
                           e);
                }
                Ok(_) => {}
            }
        }
        self.remove_connection(id);
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

    fn mark_connected(&mut self, event_loop: &mut EventLoop, id: Token) {
        let address = *self.connections[id].address();
        self.clear_reconnect_request(event_loop, &address)
    }

    fn clear_reconnect_request(&mut self, event_loop: &mut EventLoop, addr: &SocketAddr) {
        if let Some(timeout) = self.reconnects.remove(addr) {
            trace!("{}: Clear reconnect timeout for {}", self.address(), addr);
            event_loop.clear_timeout(timeout);
        }
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
