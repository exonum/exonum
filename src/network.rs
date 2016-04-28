use std::{net, io, collections};

use mio;

use super::peers::{IncomingPeer, OutgoingPeer};
use super::events::{Events, Event};
use super::message::Message;

pub type PeerId = mio::Token;

const SERVER_ID : PeerId = mio::Token(1);

#[derive(Debug)]
pub struct Network {
    listen_address: net::SocketAddr,
    listener: Option<mio::tcp::TcpListener>,
    incoming: mio::util::Slab<IncomingPeer>,
    outgoing: mio::util::Slab<OutgoingPeer>,
    addresses: collections::HashMap<net::SocketAddr, PeerId>
}

#[derive(Debug, Clone, Copy)]
pub struct NetworkConfiguration {
    pub listen_address: net::SocketAddr,
    pub max_incoming_connections: usize,
    pub max_outgoing_connections: usize,
    // TODO: think more about config parameters
}

impl Network {
    pub fn with_config(config: NetworkConfiguration) -> Network {
        Network {
            listen_address: config.listen_address,
            listener: None,
            incoming: mio::util::Slab::new_starting_at(
                mio::Token(2),
                config.max_incoming_connections
            ),
            outgoing: mio::util::Slab::new_starting_at(
                mio::Token(config.max_incoming_connections + 2),
                config.max_outgoing_connections
            ),
            addresses: collections::HashMap::new()
        }
    }

    pub fn address(&self) -> &net::SocketAddr {
        &self.listen_address
    }

    pub fn bind(&mut self, events: &mut Events) -> io::Result<()> {
        if let Some(_) = self.listener {
            return Err(io::Error::new(io::ErrorKind::Other,
                                      "Already binded"));
        }
        let listener = try!(mio::tcp::TcpListener::bind(&self.listen_address));
        let r = events.event_loop().register(
            &listener, SERVER_ID,
            mio::EventSet::readable(),
            mio::PollOpt::edge()
        );
        match r {
            Ok(()) => {
                self.listener = Some(listener);
                Ok(())
            },
            Err(e) => Err(e),
        }
    }

    // TODO: Use ticks for fast reregistering sockets
    // TODO: Implement Connections collection with (re)registering

    pub fn io(&mut self, events: &mut Events,
             id: PeerId, set: mio::EventSet) -> io::Result<()> {
        if set.is_error() || set.is_hup() {
            // TODO: reset connection
            return Ok(())
        }

        if id == SERVER_ID {
            // Accept new connections
            // FIXME: Fail-safe accepting of new connections?
            let listener = match self.listener {
                Some(ref listener) => listener,
                None => return Ok(()),
            };
            while let Some((socket, address)) = try!(listener.accept()) {
                let peer = IncomingPeer::new(socket, address);
                let id = match self.incoming.insert(peer) {
                    Ok(id) => id,
                    Err(_) => {
                        return Err(io::Error::new(io::ErrorKind::Other,
                                                  "Maximum connections"));
                    }
                };
                let r = events.event_loop().register(
                    self.incoming[id].socket(), id,
                    mio::EventSet::readable() | mio::EventSet::hup(),
                    mio::PollOpt::edge()
                );
                if let Err(e) = r {
                    self.incoming.remove(id);
                    return Err(e)
                }
            }
            return Ok(())
        }

        if set.is_writable() {
            // Write data into socket
            try!(self.outgoing[id].writable());
            if !self.outgoing[id].is_idle() {
                let r = events.event_loop().reregister(
                    self.outgoing[id].socket(), id,
                    mio::EventSet::writable() | mio::EventSet::hup(),
                    mio::PollOpt::edge() | mio::PollOpt::oneshot()
                );
                if let Err(e) = r {
                    self.addresses.remove(self.outgoing[id].address());
                    self.outgoing.remove(id);
                    return Err(e);
                }
            }
            return Ok(());
        }

        if set.is_readable() {
            // Read new data from socket
            while let Some(data) = try!(self.incoming[id].readable()) {
                events.push(Event::Incoming(Message::new(data)))
            };
            // let r = events.event_loop().reregister(
            //     self.incoming[id].socket(), id,
            //     mio::EventSet::readable() | mio::EventSet::hup(),
            //     mio::PollOpt::level()
            // );
            // if let Err(e) = r {
            //     self.incoming.remove(id);
            //     return Err(e);
            // }
            return Ok(())
        }
        // FIXME: Can we be here?
        return Ok(())
    }

    pub fn get_peer(&mut self, events: &mut Events, address: &net::SocketAddr)
                    -> io::Result<PeerId> {
        if let Some(id) = self.addresses.get(address) {
            return Ok(*id)
        };
        let socket = try!(mio::tcp::TcpStream::connect(&address));
        let peer = OutgoingPeer::new(socket, address.clone());
        let id = match self.outgoing.insert(peer) {
            Ok(id) => id,
            Err(_) => {
                return Err(io::Error::new(io::ErrorKind::Other,
                                          "Maximum connections"));
            }
        };
        self.addresses.insert(address.clone(), id);
        let r = events.event_loop().register(
            self.outgoing[id].socket(), id,
            mio::EventSet::writable() | mio::EventSet::hup(),
            mio::PollOpt::edge() | mio::PollOpt::oneshot()
        );
        match r {
            Ok(()) => Ok(id),
            Err(e) => {
                self.addresses.remove(self.outgoing[id].address());
                self.outgoing.remove(id);
                Err(e)
            }
        }
    }

    pub fn send_to(&mut self, events:
                   &mut Events,
                   address: &net::SocketAddr,
                   message: Message) -> io::Result<()> {
        let id = try!(self.get_peer(events, address));
        self.outgoing[id].send(message);
        let r = events.event_loop().reregister(
            self.outgoing[id].socket(), id,
            mio::EventSet::writable() | mio::EventSet::hup(),
            mio::PollOpt::edge() | mio::PollOpt::oneshot()
        );
        if let Err(e) = r {
            self.outgoing.remove(id);
            return Err(e);
        } else {
            Ok(())
        }
    }
}
