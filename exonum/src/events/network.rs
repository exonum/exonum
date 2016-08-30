use std::io;
use std::collections::HashMap;
use std::net::SocketAddr;

pub use mio::{EventSet, PollOpt, Token};
use mio::tcp::{TcpListener, TcpStream};
use mio::util::Slab;

use super::connection::Connection;
use super::EventLoop;

use super::super::messages::{MessageBuffer, RawMessage};

pub type PeerId = Token;

const SERVER_ID: PeerId = Token(1);

#[derive(Debug, Clone, Copy)]
pub struct NetworkConfiguration {
    pub listen_address: SocketAddr,
    pub max_connections: usize, // TODO: think more about config parameters
    pub tcp_nodelay: bool,
    pub tcp_keep_alive: Option<u32>,
}

#[derive(Debug)]
pub struct Network {
    config: NetworkConfiguration,
    listener: Option<TcpListener>,
    connections: Slab<Connection>,
    addresses: HashMap<SocketAddr, PeerId>,
}

impl Network {
    pub fn with_config(config: NetworkConfiguration) -> Network {
        Network {
            config: config,
            listener: None,
            connections: Slab::new_starting_at(Token(2), config.max_connections),
            addresses: HashMap::new(),
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
        if set.is_error() | set.is_hup() {
            trace!("{}: connection {} will be closed, id: {}",
                   self.address(),
                   self.connections[id].address(),
                   id.0);
            self.remove_connection(id);
            return Ok(None);
        }

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

        if set.is_writable() {
            trace!("{}: Socket is writable {} id: {}",
                   self.address(),
                   self.connections[id].address(),
                   id.0);
            // Write data into socket
            self.connections[id].writable()?;
            if !self.connections[id].is_idle() {
                trace!("{}: Socket is blocked {} id: {}",
                       self.address(),
                       self.connections[id].address(),
                       id.0);

                self.reregister_connection(event_loop, id, PollOpt::edge())?;
            }
            return Ok(None);
        }

        if set.is_readable() {
            trace!("{}: Socket is readable {} id: {}",
                   self.address(),
                   self.connections[id].address(),
                   id.0);
            // Read new data from socket
            return self.connections[id].readable();
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

        let peer = Connection::new(TcpStream::connect(address)?, *address);
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
                   message: RawMessage)
                   -> io::Result<()> {
        let id = self.get_peer(event_loop, address)?;

        trace!("{}: Send message to outgoing {}, id: {}, size: {}",
               self.address(),
               address,
               id.0,
               message.clone().len());

        self.connections[id].send(message)?;
        let r = event_loop.reregister(self.connections[id].socket(),
                                      id,
                                      self.connections[id].interest(),
                                      PollOpt::edge());
        if let Err(e) = r {
            self.remove_connection(id);
            return Err(e);
        } else {
            Ok(())
        }
    }

    fn remove_connection(&mut self, id: Token) {
        self.addresses.remove(self.connections[id].address());
        self.connections.remove(id);
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

    fn register_connection(&mut self,
                           event_loop: &mut EventLoop,
                           id: Token,
                           opts: PollOpt)
                           -> io::Result<()> {
        event_loop.register(self.connections[id].socket(),
                      id,
                      self.connections[id].interest(),
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
