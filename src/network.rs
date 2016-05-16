use std::io;
use std::collections::HashMap;
use std::net::SocketAddr;

use mio::{EventSet, PollOpt, Token};
use mio::tcp::{TcpListener, TcpStream};
use mio::util::Slab;

use super::connection::{IncomingConnection, OutgoingConnection};
use super::events::{Events, Event};
use super::message::Message;

pub type PeerId = Token;

const SERVER_ID : PeerId = Token(1);

#[derive(Debug)]
pub struct Network {
    listen_address: SocketAddr,
    listener: Option<TcpListener>,
    incoming: Slab<IncomingConnection>,
    outgoing: Slab<OutgoingConnection>,
    addresses: HashMap<SocketAddr, PeerId>
}

#[derive(Debug, Clone, Copy)]
pub struct NetworkConfiguration {
    pub listen_address: SocketAddr,
    pub max_incoming_connections: usize,
    pub max_outgoing_connections: usize,
    // TODO: think more about config parameters
}

impl Network {
    pub fn with_config(config: NetworkConfiguration) -> Network {
        Network {
            listen_address: config.listen_address,
            listener: None,
            incoming: Slab::new_starting_at(
                Token(2),
                config.max_incoming_connections
            ),
            outgoing: Slab::new_starting_at(
                Token(config.max_incoming_connections + 2),
                config.max_outgoing_connections
            ),
            addresses: HashMap::new()
        }
    }

    pub fn address(&self) -> &SocketAddr {
        &self.listen_address
    }

    pub fn bind(&mut self, events: &mut Events) -> io::Result<()> {
        if let Some(_) = self.listener {
            return Err(io::Error::new(io::ErrorKind::Other,
                                      "Already binded"));
        }
        let listener = TcpListener::bind(&self.listen_address)?;
        let r = events.event_loop().register(
            &listener, SERVER_ID,
            EventSet::readable(),
            PollOpt::edge()
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
             id: PeerId, set: EventSet) -> io::Result<()> {
        if set.is_error() {
            // TODO: TEMPORARY FIX, MAKE IT NORMAL
            self.addresses.remove(self.outgoing[id].address());
            self.outgoing.remove(id);
            return Ok(())
        }

        if set.is_hup() {
            // TODO: TEMPORARY FIX, MAKE IT NORMAL
            self.incoming.remove(id);
            return Ok(())
        }

        if id == SERVER_ID {
            // Accept new connections
            // FIXME: Fail-safe accepting of new connections?
            let listener = match self.listener {
                Some(ref listener) => listener,
                None => return Ok(()),
            };
            while let Some((socket, address)) = listener.accept()? {
                let peer = IncomingConnection::new(socket, address);
                let id = match self.incoming.insert(peer) {
                    Ok(id) => id,
                    Err(_) => {
                        return Err(io::Error::new(io::ErrorKind::Other,
                                                  "Maximum connections"));
                    }
                };
                let r = events.event_loop().register(
                    self.incoming[id].socket(), id,
                    EventSet::readable() | EventSet::hup(),
                    PollOpt::edge()
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
            self.outgoing[id].writable()?;
            if !self.outgoing[id].is_idle() {
                let r = events.event_loop().reregister(
                    self.outgoing[id].socket(), id,
                    EventSet::writable() | EventSet::hup(),
                    PollOpt::edge() | PollOpt::oneshot()
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
            while let Some(data) = self.incoming[id].readable()? {
                events.push(Event::Incoming(Message::new(data)))
            };
            // let r = events.event_loop().reregister(
            //     self.incoming[id].socket(), id,
            //     EventSet::readable() | EventSet::hup(),
            //     PollOpt::level()
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

    pub fn get_peer(&mut self, events: &mut Events, address: &SocketAddr)
                    -> io::Result<PeerId> {
        if let Some(id) = self.addresses.get(address) {
            return Ok(*id)
        };
        let socket = TcpStream::connect(&address)?;
        let peer = OutgoingConnection::new(socket, address.clone());
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
            EventSet::writable() | EventSet::hup(),
            PollOpt::edge() | PollOpt::oneshot()
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
                   address: &SocketAddr,
                   message: Message) -> io::Result<()> {
        let id = self.get_peer(events, address)?;
        self.outgoing[id].send(message);
        let r = events.event_loop().reregister(
            self.outgoing[id].socket(), id,
            EventSet::writable() | EventSet::hup(),
            PollOpt::edge() | PollOpt::oneshot()
        );
        if let Err(e) = r {
            self.outgoing.remove(id);
            return Err(e);
        } else {
            Ok(())
        }
    }
}
