// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use futures::{future, unsync, Future, IntoFuture, Sink, Stream};
use futures::future::Either;
use futures::sync::mpsc;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Handle;
use tokio_io::AsyncRead;
use tokio_retry::Retry;
use tokio_retry::strategy::{jitter, FixedInterval};

use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use messages::{Any, Connect, RawMessage};
use node::{ExternalMessage, NodeTimeout};
use helpers::Milliseconds;

use super::error::{into_other, log_error, other_error, result_ok};
use super::codec::MessagesCodec;
use super::{EventsAggregator, EventHandler};

const OUTGOING_CHANNEL_SIZE: usize = 10;

#[derive(Debug)]
pub enum NetworkEvent {
    MessageReceived(SocketAddr, RawMessage),
    PeerConnected(SocketAddr, Connect),
    PeerDisconnected(SocketAddr),
}

#[derive(Debug, Clone)]
pub enum NetworkRequest {
    SendMessage(SocketAddr, RawMessage),
    DisconnectWithPeer(SocketAddr),
    Shutdown,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct NetworkConfiguration {
    // TODO: think more about config parameters
    pub max_incoming_connections: usize,
    pub max_outgoing_connections: usize,
    pub tcp_nodelay: bool,
    pub tcp_keep_alive: Option<u64>,
    pub tcp_connect_retry_timeout: Milliseconds,
    pub tcp_connect_max_retries: u64,
}

impl Default for NetworkConfiguration {
    fn default() -> NetworkConfiguration {
        NetworkConfiguration {
            max_incoming_connections: 128,
            max_outgoing_connections: 128,
            tcp_keep_alive: None,
            tcp_nodelay: false,
            tcp_connect_retry_timeout: 15_000,
            tcp_connect_max_retries: 10,
        }
    }
}

#[derive(Debug)]
pub struct HandlerPart<H: EventHandler> {
    pub handler: H,
    pub timeout_rx: mpsc::Receiver<NodeTimeout>,
    pub network_rx: mpsc::Receiver<NetworkEvent>,
    pub api_rx: mpsc::Receiver<ExternalMessage>,
}

#[derive(Debug)]
pub struct NetworkPart {
    pub listen_address: SocketAddr,
    pub network_config: NetworkConfiguration,
    pub network_requests: (mpsc::Sender<NetworkRequest>, mpsc::Receiver<NetworkRequest>),
    pub network_tx: mpsc::Sender<NetworkEvent>,
}

#[derive(Debug, Default, Clone)]
struct ConnectionsPool {
    inner: Rc<RefCell<HashMap<SocketAddr, mpsc::Sender<RawMessage>>>>,
}

impl ConnectionsPool {
    fn new() -> ConnectionsPool {
        ConnectionsPool::default()
    }

    fn insert(&self, peer: SocketAddr, sender: &mpsc::Sender<RawMessage>) {
        self.inner.borrow_mut().insert(peer, sender.clone());
    }

    fn remove(&self, peer: &SocketAddr) -> Result<mpsc::Sender<RawMessage>, &'static str> {
        self.inner.borrow_mut().remove(peer).ok_or("there is no sender in the connection pool")
    }

    fn get(&self, peer: SocketAddr) -> Option<mpsc::Sender<RawMessage>> {
        self.inner.borrow_mut().get(&peer).cloned()
    }

    fn len(&self) -> usize {
        self.inner.borrow_mut().len()
    }
}

impl<H: EventHandler + 'static> HandlerPart<H> {
    pub fn run(self) -> Box<Future<Item = (), Error = ()>> {
        let mut handler = self.handler;

        let fut = EventsAggregator::new(self.timeout_rx, self.network_rx, self.api_rx)
            .for_each(move |event| {
                handler.handle_event(event);
                Ok(())
            });

        tobox(fut)
    }
}

impl NetworkPart {
    pub fn run(self, handle_orig: Handle) -> Box<Future<Item = (), Error = io::Error>> {
        let network_config = self.network_config;
        // Cancelation token
        let (cancel_sender, cancel_handler) = unsync::oneshot::channel();
        let mut cancel_sender = Some(cancel_sender);
        // Outgoing connections
        let outgoing_connections = ConnectionsPool::new();
        let network_tx = self.network_tx.clone();
        // Outgoing connections limiter
        let outgoing_connections_limit = network_config.max_outgoing_connections;
        let handle = handle_orig.clone();
        let requests_handle = self.network_requests
            .1
            .map_err(|_| other_error("no network requests"))
            .for_each(move |request| {
                let handle = handle.clone();
                let outgoing_connections = outgoing_connections.clone();
                match request {
                    NetworkRequest::SendMessage(peer, msg) => {
                        let network_tx_cloned = network_tx.clone();
                        let outgoing_connections_cloned = outgoing_connections.clone();
                        let new_connection = || {
                            if outgoing_connections_cloned.len() >= outgoing_connections_limit {
                                warn!(
                                    "Rejected outgoing connection with peer={}, \
                                 connections limit reached.",
                                    peer
                                );
                                None
                            } else {
                                // Register outgoing channel.
                                let (conn_tx, conn_rx) = mpsc::channel(OUTGOING_CHANNEL_SIZE);
                                outgoing_connections.insert(peer, &conn_tx);
                                // Enable retry feature for outgoing connection.
                                let timeout = network_config.tcp_connect_retry_timeout;
                                let max_tries = network_config.tcp_connect_max_retries as usize;
                                let strategy = FixedInterval::from_millis(timeout)
                                    .map(jitter)
                                    .take(max_tries);
                                let handle_cloned = handle.clone();
                                let action = move || TcpStream::connect(&peer, &handle_cloned);
                                let connect_handle = Retry::spawn(handle.clone(), strategy, action)
                                .map_err(into_other)
                                // Configure socket
                                .and_then(move |sock| {
                                    sock.set_nodelay(network_config.tcp_nodelay)?;
                                    let duration =
                                        network_config.tcp_keep_alive.map(Duration::from_millis);
                                    sock.set_keepalive(duration)?;
                                    Ok(sock)
                                })
                                // Connect socket with the outgoing channel
                                .and_then(move |sock| {
                                    trace!("Established connection with peer={}", peer);

                                    let stream = sock.framed(MessagesCodec);
                                    let (sink, stream) = stream.split();

                                    let writer = conn_rx
                                        .map_err(|_| other_error("Can't send data into socket"))
                                        .forward(sink);
                                    let reader = stream.for_each(result_ok);

                                    reader
                                        .select2(writer)
                                        .map_err(|_| other_error("Socket error"))
                                        .and_then(|res| match res {
                                            Either::A((_, _reader)) => Ok("by reader"),
                                            Either::B((_, _writer)) => Ok("by writer"),
                                        })
                                })
                                .then(move |res| {
                                    trace!(
                                        "Disconnection with peer={}, reason={:?}",
                                        peer,
                                        res
                                    );
                                    let network_tx = network_tx_cloned.clone();
                                    outgoing_connections_cloned.clone().remove(&peer)
                                        .into_future()
                                        .map_err(other_error)
                                        .and_then(move |_| {
                                            network_tx
                                                .send(NetworkEvent::PeerDisconnected(peer))
                                                .map_err(|_| other_error("can't send disconnect"))
                                        })
                                        .map(drop)
                                })
                                .map_err(log_error);
                                handle.spawn(connect_handle);
                                Some(conn_tx)
                            }
                        };
                        let conn_tx = outgoing_connections.get(peer).or_else(new_connection);
                        if let Some(conn_tx) = conn_tx {
                            let fut = conn_tx.send(msg).map_err(|_| {
                                other_error("can't send message to a connection")
                            });
                            tobox(fut)
                        } else {
                            let event = NetworkEvent::PeerDisconnected(peer);
                            let fut = network_tx
                                .clone()
                                .send(event)
                                .map_err(|_| other_error("can't send network event"))
                                .into_future();
                            tobox(fut)
                        }
                    }
                    NetworkRequest::DisconnectWithPeer(peer) => {
                        let network_tx = network_tx.clone();
                        let fut = outgoing_connections.remove(&peer)
                            .into_future()
                            .map_err(other_error)
                            .and_then(move |_| {
                                network_tx
                                    .send(NetworkEvent::PeerDisconnected(peer))
                                    .map_err(|_| other_error("can't send disconnect"))
                            })
                            .map(drop);
                        tobox(fut)
                    }
                    // Immediately stop the event loop.
                    NetworkRequest::Shutdown => {
                        let fut = cancel_sender
                            .take()
                            .ok_or_else(|| other_error("shutdown twice"))
                            .and_then(|sender| {
                                sender.send(()).map_err(
                                    |_| other_error("can't send shutdown signal"),
                                )
                            })
                            .into_future();
                        tobox(fut)
                    }
                }
            });

        // Incoming connections limiter
        let incoming_connections_limit = network_config.max_incoming_connections;
        let incoming_connections_counter: Rc<()> = Rc::default();
        // Incoming connections handler
        // TODO Don't use unwrap here!
        let listener = TcpListener::bind(&self.listen_address, &handle_orig).unwrap();
        let network_tx = self.network_tx.clone();
        let server = listener.incoming().for_each(move |(sock, addr)| {
            // Increment reference counter
            let holder = Rc::downgrade(&incoming_connections_counter);
            // Check incoming connections count
            let connections_count = Rc::weak_count(&incoming_connections_counter);
            if connections_count > incoming_connections_limit {
                warn!(
                    "Rejected incoming connection with peer={}, \
                         connections limit reached.",
                    addr
                );
                tobox(future::ok(()))
            } else {
                trace!("Accepted incoming connection with peer={}", addr);
                let stream = sock.framed(MessagesCodec);
                let (_, stream) = stream.split();
                let network_tx = network_tx.clone();
                let connection_handler = stream
                    .into_future()
                    .map_err(|e| e.0)
                    .and_then(move |(raw, stream)| match raw.map(Any::from_raw) {
                        Some(Ok(Any::Connect(msg))) => Ok((msg, stream)),
                        Some(Ok(other)) => Err(other_error(&format!(
                            "First message is not Connect, got={:?}",
                            other
                        ))),
                        Some(Err(e)) => Err(into_other(e)),
                        None => Err(other_error("Incoming socket closed")),
                    })
                    .and_then(move |(connect, stream)| {
                        trace!("Received handshake message={:?}", connect);
                        let event = NetworkEvent::PeerConnected(addr, connect);
                        let stream = network_tx
                            .clone()
                            .send(event)
                            .map_err(into_other)
                            .and_then(move |_| Ok(stream))
                            .flatten_stream();

                        stream.for_each(move |raw| {
                            let event = NetworkEvent::MessageReceived(addr, raw);
                            network_tx.clone().send(event).map_err(into_other).map(drop)
                        })
                    })
                    .map(|_| {
                        // Ensure that holder lives until the stream ends.
                        let _holder = holder;
                    });
                tobox(connection_handler)
            }
        });

        let cancel_handler = cancel_handler.map_err(|_| other_error("can't cancel routine"));
        let fut = server
            .join(requests_handle)
            .map(drop)
            .select(cancel_handler)
            .map_err(|(e, _)| e);
        tobox(fut)
    }
}

fn tobox<F: Future + 'static>(f: F) -> Box<Future<Item = (), Error = F::Error>> {
    Box::new(f.map(drop))
}
