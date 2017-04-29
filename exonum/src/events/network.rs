use mio::Timeout as MioTimeout;
use mio::tcp::{TcpListener, TcpStream};
use mio::util::Slab;

use std::borrow::Borrow;
use std::io;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::cmp::min;
use std::default::Default;

use messages::RawMessage;
use super::connection::{Connection, IncomingConnection, OutgoingConnection};
use super::{Timeout, InternalTimeout, EventLoop, EventHandler, Event};

pub use mio::{EventSet, PollOpt, Token};

pub type PeerId = Token;

const SERVER_ID: PeerId = Token(1);
const RECONNECT_GROW_FACTOR: f32 = 2.0;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct NetworkConfiguration {
    // TODO: think more about config parameters
    pub max_incoming_connections: usize,
    pub max_outgoing_connections: usize,
    pub tcp_nodelay: bool,
    pub tcp_keep_alive: Option<u32>,
    pub tcp_reconnect_timeout: u64,
    pub tcp_reconnect_timeout_max: u64,
}

impl Default for NetworkConfiguration {
    fn default() -> NetworkConfiguration {
        NetworkConfiguration {
            max_incoming_connections: 128,
            max_outgoing_connections: 128,
            tcp_keep_alive: None,
            tcp_nodelay: false,
            tcp_reconnect_timeout: 500,
            tcp_reconnect_timeout_max: 600000,
        }
    }
}

// TODO Implement generic ConnectionPool struct to avoid copy paste.
// Write proper code to configure outgoing streams
pub struct Network {
    config: NetworkConfiguration,
    listen_address: SocketAddr,
    listener: Option<TcpListener>,

    incoming: Slab<IncomingConnection>,
    outgoing: Slab<OutgoingConnection>,
    // FIXME addresses only needs for outgoing connections
    addresses: HashMap<SocketAddr, PeerId>,

    reconnects: HashMap<SocketAddr, MioTimeout>,
}

enum PeerKind {
    Server,
    Incoming,
    Outgoing,
}

fn make_io_error<T: Borrow<str>>(s: T) -> io::Error {
    io::Error::new(io::ErrorKind::Other, s.borrow())
}

impl Network {
    pub fn with_config(address: SocketAddr, config: NetworkConfiguration) -> Network {
        Network {
            config: config,
            listen_address: address,
            listener: None,

            incoming: Slab::new_starting_at(Token(2), config.max_incoming_connections),
            outgoing: Slab::new_starting_at(Token(2 + config.max_incoming_connections),
                                            config.max_outgoing_connections),
            addresses: HashMap::new(),

            reconnects: HashMap::new(),
        }
    }

    pub fn address(&self) -> &SocketAddr {
        &self.listen_address
    }

    // TODO use error trait
    pub fn bind<H: EventHandler>(&mut self, event_loop: &mut EventLoop<H>) -> io::Result<()> {
        if self.listener.is_some() {
            return Err(make_io_error("Already binded"));
        }
        let listener = TcpListener::bind(&self.listen_address)?;
        event_loop.register(&listener, SERVER_ID, EventSet::readable(), PollOpt::edge())?;
        self.listener = Some(listener);
        Ok(())
    }

    // TODO: Use ticks for fast reregistering sockets
    // TODO: Implement Connections collection with (re)registering
    pub fn io<H: EventHandler>(&mut self,
                               event_loop: &mut EventLoop<H>,
                               handler: &mut H,
                               id: PeerId,
                               set: EventSet)
                               -> io::Result<()> {

        match self.peer_kind(id) {
            PeerKind::Server => {
                // Accept new connections
                // FIXME: Fail-safe accepting of new connections?
                let pair = match self.listener {
                    Some(ref listener) => listener.accept()?,
                    None => None,
                };
                if let Some((mut stream, address)) = pair {
                    self.configure_stream(&mut stream)?;
                    let peer = IncomingConnection::new(stream, address);
                    self.add_incoming_connection(event_loop, peer)?;

                    trace!("{}: Accepted incoming connection from {} id: {}",
                           self.address(),
                           address,
                           id.0);
                }
                return Ok(());
            }
            PeerKind::Incoming => {
                if !self.incoming.contains(id) {
                    return Ok(());
                }

                if set.is_hup() | set.is_error() {
                    trace!("{}: incoming connection with addr {} closed",
                           self.address(),
                           self.incoming[id].address());

                    self.remove_incoming_connection(event_loop, id);
                    return Ok(());
                }

                if set.is_readable() {
                    loop {
                        match self.incoming[id].try_read() {
                            Ok(Some(buf)) => {
                                let msg = RawMessage::new(buf);
                                handler.handle_event(Event::Incoming(msg));
                            }
                            Ok(None) => return Ok(()),
                            Err(e) => {
                                self.remove_incoming_connection(event_loop, id);
                                return Err(e);
                            }
                        };
                    }
                }
            }
            PeerKind::Outgoing => {
                if !self.outgoing.contains(id) {
                    return Ok(());
                }

                if set.is_hup() | set.is_error() {
                    let address = *self.outgoing[id].address();

                    trace!("{}: outgoing connection with addr {} closed",
                           self.address(),
                           self.outgoing[id].address());

                    self.remove_outgoing_connection(event_loop, id);
                    if !self.reconnects.contains_key(&address) {
                        handler.handle_event(Event::Disconnected(address));
                    }
                    return Ok(());
                }

                if set.is_writable() {
                    let address = *self.outgoing[id].address();
                    let r = {
                        // Write data into socket
                        self.outgoing[id].try_write()?;
                        event_loop.reregister(self.outgoing[id].socket(),
                                        id,
                                        self.outgoing[id].interest(),
                                        PollOpt::edge())?;
                        Ok(())
                    };
                    if let Err(e) = r {
                        self.remove_outgoing_connection(event_loop, id);
                        handler.handle_event(Event::Disconnected(address));
                        return Err(e);
                    }

                    if self.mark_connected(event_loop, id) {
                        trace!("{}: Handle connected with={}", self.address(), address);
                        handler.handle_event(Event::Connected(address));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn tick<H: EventHandler>(&mut self, _: &mut EventLoop<H>) {}

    pub fn send_to<H: EventHandler>(&mut self,
                                    event_loop: &mut EventLoop<H>,
                                    address: &SocketAddr,
                                    message: RawMessage)
                                    -> io::Result<()> {
        match self.get_outgoing_peer(address) {
            Ok(id) => {
                self.outgoing[id]
                    .send(message)
                    .and_then(|_| {
                        event_loop.reregister(self.outgoing[id].socket(),
                                        id,
                                        self.outgoing[id].interest(),
                                        PollOpt::edge())?;
                        self.mark_connected(event_loop, id);
                        Ok(())
                    })
                    .or_else(|e| {
                        self.remove_outgoing_connection(event_loop, id);
                        Err(e)
                    })
            }
            Err(e) => Err(e),
        }
    }

    pub fn connect<H: EventHandler>(&mut self,
                                    event_loop: &mut EventLoop<H>,
                                    address: &SocketAddr)
                                    -> io::Result<()> {
        if !self.is_connected(address) {
            self.add_reconnect_request(event_loop, *address)?;

            let mut stream = TcpStream::connect(address)?;
            self.configure_stream(&mut stream)?;
            let peer = OutgoingConnection::new(stream, *address);
            let id = self.add_outgoing_connection(event_loop, peer)?;

            trace!("{}: Establish connection with {}, id: {}",
                   self.address(),
                   address,
                   id.0);
        }
        Ok(())
    }

    pub fn is_connected(&self, address: &SocketAddr) -> bool {
        self.addresses.contains_key(address)
    }

    pub fn handle_timeout<H: EventHandler>(&mut self,
                                           event_loop: &mut EventLoop<H>,
                                           timeout: InternalTimeout) {
        match timeout {
            InternalTimeout::Reconnect(addr, delay) => {
                if self.reconnects.contains_key(&addr) {
                    trace!("Try to reconnect with delay {}", delay);

                    if let Err(e) = self.connect(event_loop, &addr) {
                        error!("{}: Unable to create connection to addr {}, error: {:?}",
                               self.address(),
                               addr,
                               e);
                    }

                    let delay = min((delay as f32 * RECONNECT_GROW_FACTOR) as u64,
                                    self.config.tcp_reconnect_timeout_max);

                    if let Err(e) = self.add_reconnect_timeout(event_loop, addr, delay) {
                        error!("{}: Unable to add timeout, error: {:?}", self.address(), e);
                    }
                }
            }
        }
    }

    fn peer_kind(&self, id: PeerId) -> PeerKind {
        if id == SERVER_ID {
            PeerKind::Server
        } else if id.0 >= (2 + self.config.max_incoming_connections) {
            PeerKind::Outgoing
        } else {
            PeerKind::Incoming
        }
    }

    fn get_outgoing_peer(&self, addr: &SocketAddr) -> io::Result<PeerId> {
        if let Some(id) = self.addresses.get(addr) {
            return Ok(*id);
        };
        Err(make_io_error(format!("{}: Outgoing peer not found {}", self.address(), addr)))
    }

    fn add_incoming_connection<H: EventHandler>(&mut self,
                                                event_loop: &mut EventLoop<H>,
                                                connection: IncomingConnection)
                                                -> io::Result<PeerId> {
        let address = *connection.address();
        let id = self.incoming
            .insert(connection)
            .map_err(|_| make_io_error("Maximum incoming onnections"))?;
        self.addresses.insert(address, id);

        let r = event_loop.register(self.incoming[id].socket(),
                                    id,
                                    self.incoming[id].interest(),
                                    PollOpt::edge());

        if let Err(e) = r {
            self.remove_incoming_connection(event_loop, id);
            return Err(e);
        }
        Ok(id)
    }

    fn add_outgoing_connection<H: EventHandler>(&mut self,
                                                event_loop: &mut EventLoop<H>,
                                                connection: OutgoingConnection)
                                                -> io::Result<PeerId> {
        let address = *connection.address();
        let id = self.outgoing
            .insert(connection)
            .map_err(|_| make_io_error("Maximum outgoing onnections"))?;
        self.addresses.insert(address, id);

        let r = event_loop.register(self.outgoing[id].socket(),
                                    id,
                                    self.outgoing[id].interest() | EventSet::writable(),
                                    PollOpt::edge());

        if let Err(e) = r {
            self.remove_outgoing_connection(event_loop, id);
            return Err(e);
        }
        Ok(id)
    }

    fn remove_incoming_connection<H: EventHandler>(&mut self,
                                                   event_loop: &mut EventLoop<H>,
                                                   id: PeerId) {
        let addr = *self.incoming[id].address();
        self.addresses.remove(&addr);
        if let Some(connection) = self.incoming.remove(id) {
            if let Err(e) = event_loop.deregister(connection.socket()) {
                error!("{}: Unable to deregister incoming connection, id: {}, error: {:?}",
                       self.address(),
                       id.0,
                       e);
            }
        }
    }

    fn remove_outgoing_connection<H: EventHandler>(&mut self,
                                                   event_loop: &mut EventLoop<H>,
                                                   id: PeerId) {
        let addr = *self.outgoing[id].address();
        self.addresses.remove(&addr);
        if let Some(connection) = self.outgoing.remove(id) {
            if let Err(e) = event_loop.deregister(connection.socket()) {
                error!("{}: Unable to deregister outgoing connection, id: {}, error: {:?}",
                       self.address(),
                       id.0,
                       e);
            }
        }
    }

    fn configure_stream(&self, stream: &mut TcpStream) -> io::Result<()> {
        stream.take_socket_error()?;
        stream.set_keepalive(self.config.tcp_keep_alive)?;
        stream.set_nodelay(self.config.tcp_nodelay)?;
        stream.take_socket_error()
    }

    fn add_reconnect_request<H: EventHandler>(&mut self,
                                              event_loop: &mut EventLoop<H>,
                                              address: SocketAddr)
                                              -> io::Result<()> {
        if !self.reconnects.contains_key(&address) {
            let delay = self.config.tcp_reconnect_timeout;
            return self.add_reconnect_timeout(event_loop, address, delay);
        }
        Ok(())
    }

    fn add_reconnect_timeout<H: EventHandler>(&mut self,
                                              event_loop: &mut EventLoop<H>,
                                              address: SocketAddr,
                                              delay: u64)
                                              -> io::Result<()> {
        trace!("{}: Add reconnect timeout to={}, delay={}",
               self.address(),
               address,
               delay);
        let reconnect = Timeout::Internal(InternalTimeout::Reconnect(address, delay));
        let timeout = event_loop.timeout_ms(reconnect, delay)
            .map_err(|e| make_io_error(format!("A mio error occured {:?}", e)))?;
        self.reconnects.insert(address, timeout);
        Ok(())
    }

    fn mark_connected<H: EventHandler>(&mut self,
                                       event_loop: &mut EventLoop<H>,
                                       id: Token)
                                       -> bool {
        let address = *self.outgoing[id].address();
        self.clear_reconnect_request(event_loop, &address)
    }

    fn clear_reconnect_request<H: EventHandler>(&mut self,
                                                event_loop: &mut EventLoop<H>,
                                                addr: &SocketAddr)
                                                -> bool {
        if let Some(timeout) = self.reconnects.remove(addr) {
            trace!("{}: Clear reconnect timeout to={}", self.address(), addr);
            event_loop.clear_timeout(timeout);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {}
