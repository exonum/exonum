// Copyright 2018 The Exonum Team
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

use futures::{future, unsync, Future, IntoFuture, Poll, Sink, Stream};
use futures::{future::Either, sync::mpsc};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Handle;
use tokio_io::AsyncRead;
use tokio_retry::{Retry, strategy::{jitter, FixedInterval}};
use failure;

use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use messages::{Protocol, Connect, Message, SignedMessage, UncheckedBuffer};
use helpers::Milliseconds;
use super::to_box;
use super::error::{into_failure, log_error, result_ok};
use super::codec::MessagesCodec;

const OUTGOING_CHANNEL_SIZE: usize = 10;

#[derive(Debug)]
pub enum NetworkEvent {
    MessageReceived(SocketAddr, UncheckedBuffer),
    PeerConnected(SocketAddr, Message<Connect>),
    PeerDisconnected(SocketAddr),
    UnableConnectToPeer(SocketAddr),
}

#[derive(Debug, Clone)]
pub enum NetworkRequest {
    SendMessage(SocketAddr, SignedMessage),
    DisconnectWithPeer(SocketAddr),
    Shutdown,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct NetworkConfiguration {
    // TODO: think more about config parameters (ECR-162)
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
            tcp_nodelay: true,
            tcp_connect_retry_timeout: 15_000,
            tcp_connect_max_retries: 10,
        }
    }
}

#[derive(Debug)]
pub struct NetworkPart {
    pub our_connect_message: Message<Connect>,
    pub listen_address: SocketAddr,
    pub network_config: NetworkConfiguration,
    pub max_message_len: u32,
    pub network_requests: (mpsc::Sender<NetworkRequest>, mpsc::Receiver<NetworkRequest>),
    pub network_tx: mpsc::Sender<NetworkEvent>,
}

#[derive(Debug, Default, Clone)]
struct ConnectionsPool {
    inner: Rc<RefCell<HashMap<SocketAddr, mpsc::Sender<SignedMessage>>>>,
}

impl ConnectionsPool {
    fn new() -> ConnectionsPool {
        ConnectionsPool::default()
    }

    fn insert(&self, peer: SocketAddr, sender: &mpsc::Sender<SignedMessage>) {
        self.inner.borrow_mut().insert(peer, sender.clone());
    }

    fn remove(&self, peer: &SocketAddr) -> Result<mpsc::Sender<SignedMessage>, failure::Error> {
        self.inner.borrow_mut().remove(peer).ok_or_else(||
            format_err!("there is no sender in the connection pool"),
        )
    }

    fn get(&self, peer: SocketAddr) -> Option<mpsc::Sender<SignedMessage>> {
        self.inner.borrow_mut().get(&peer).cloned()
    }

    fn len(&self) -> usize {
        self.inner.borrow_mut().len()
    }

    fn connect_to_peer(
        self,
        network_config: NetworkConfiguration,
        max_message_len: u32,
        peer: SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
        handle: &Handle,
    ) -> Option<mpsc::Sender<SignedMessage>> {
        let limit = network_config.max_outgoing_connections;
        if self.len() >= limit {
            warn!(
                "Rejected outgoing connection with peer={}, \
                 connections limit reached.",
                peer
            );
            return None;
        }
        // Register outgoing channel.
        let (conn_tx, conn_rx) = mpsc::channel(OUTGOING_CHANNEL_SIZE);
        self.insert(peer, &conn_tx);
        // Enable retry feature for outgoing connection.
        let timeout = network_config.tcp_connect_retry_timeout;
        let max_tries = network_config.tcp_connect_max_retries as usize;
        let strategy = FixedInterval::from_millis(timeout)
            .map(jitter)
            .take(max_tries);
        let handle_clonned = handle.clone();

        let action = move || TcpStream::connect(&peer, &handle_clonned);
        let connect_handle = Retry::spawn(handle.clone(), strategy, action)
            .map_err(into_failure)
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

                let stream = sock.framed(MessagesCodec::new(max_message_len));
                let (sink, stream) = stream.split();

                let writer = conn_rx
                    .map_err(|_| format_err!("Can't send data into socket"))
                    .forward(sink);
                let reader = stream.for_each(result_ok);

                reader
                    .select2(writer)
                    .map_err(|_| format_err!("Socket error"))
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
                self.disconnect_with_peer(peer, network_tx.clone())
            })
            .map_err(log_error);
        handle.spawn(connect_handle);
        Some(conn_tx)
    }

    fn disconnect_with_peer(
        &self,
        peer: SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> Box<Future<Item = (), Error = failure::Error>> {
        let fut = self.remove(&peer)
            .into_future()
            .and_then(move |_| {
                network_tx
                    .send(NetworkEvent::PeerDisconnected(peer))
                    .map_err(|_| format_err!("can't send disconnect"))
            })
            .map(drop);
        to_box(fut)
    }
}

impl NetworkPart {
    pub fn run(self, handle: &Handle) -> Box<Future<Item = (), Error = failure::Error>> {
        let network_config = self.network_config;
        // Cancellation token
        let (cancel_sender, cancel_handler) = unsync::oneshot::channel();

        let requests_handle = RequestHandler::new(
            self.our_connect_message,
            network_config,
            self.max_message_len,
            self.network_tx.clone(),
            handle.clone(),
            self.network_requests.1,
            cancel_sender,
        );
        // TODO Don't use unwrap here!
        let server = Listener::bind(
            network_config,
            self.max_message_len,
            self.listen_address,
            handle.clone(),
            &self.network_tx,
        ).unwrap();

        let cancel_handler = cancel_handler.or_else(|e| {
            trace!("Requests handler closed: {}", e);
            Ok(())
        });
        let fut = server
            .join(requests_handle)
            .map(drop)
            .select(cancel_handler)
            .map_err(|(e, _)| e);
        to_box(fut)
    }
}

struct RequestHandler(
    // TODO: Replace with concrete type
    Box<Future<Item = (), Error = failure::Error>>,
);

impl RequestHandler {
    fn new(
        connect_message: Message<Connect>,
        network_config: NetworkConfiguration,
        max_message_len: u32,
        network_tx: mpsc::Sender<NetworkEvent>,
        handle: Handle,
        receiver: mpsc::Receiver<NetworkRequest>,
        cancel_sender: unsync::oneshot::Sender<()>,
    ) -> RequestHandler {
        let mut cancel_sender = Some(cancel_sender);
        let outgoing_connections = ConnectionsPool::new();
        let requests_handler = receiver
            .map_err(|_| format_err!("no network requests"))
            .for_each(move |request| {
                match request {
                    NetworkRequest::SendMessage(peer, msg) => {
                        let conn_tx = outgoing_connections
                            .get(peer)
                            .map(|conn_tx| conn_fut(Ok(conn_tx).into_future()))
                            .or_else(|| {
                                outgoing_connections
                                    .clone()
                                    .connect_to_peer(
                                        network_config,
                                        max_message_len,
                                        peer,
                                        network_tx.clone(),
                                        &handle,
                                    )
                                    .map(|conn_tx|
                                        // if we create new connect, we should send connect message
                                        if &msg != connect_message.as_ref() {
                                            conn_fut(conn_tx.send(connect_message.clone().into())
                                                           .map_err(|_|
                                                               format_err!("can't send message to a connection")
                                            ))
                                        }
                                        else {
                                            conn_fut(Ok(conn_tx).into_future())
                                    })
                            });
                        if let Some(conn_tx) = conn_tx {
                            let fut = conn_tx.and_then(|conn_tx| {
                                conn_tx
                                    .send(msg)
                                    .map_err(|_| format_err!("can't send message to a connection"))
                            });
                            to_box(fut)
                        } else {
                            let event = NetworkEvent::UnableConnectToPeer(peer);
                            let fut = network_tx
                                .clone()
                                .send(event)
                                .map_err(|_| format_err!("can't send network event"))
                                .into_future();
                            to_box(fut)
                        }
                    }
                    NetworkRequest::DisconnectWithPeer(peer) => {
                        outgoing_connections.disconnect_with_peer(peer, network_tx.clone())
                    }
                    // Immediately stop the event loop.
                    NetworkRequest::Shutdown => to_box(
                        cancel_sender
                            .take()
                            .ok_or_else(|| format_err!("shutdown twice"))
                            .into_future(),
                    ),
                }
            });
        RequestHandler(to_box(requests_handler))
    }
}

impl Future for RequestHandler {
    type Item = ();
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

struct Listener(Box<Future<Item = (), Error = failure::Error>>);

impl Listener {
    fn bind(
        network_config: NetworkConfiguration,
        max_message_len: u32,
        listen_address: SocketAddr,
        handle: Handle,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> Result<Listener, failure::Error> {
        // Incoming connections limiter
        let incoming_connections_limit = network_config.max_incoming_connections;
        // The reference counter is used to automatically count the number of the open connections.
        let incoming_connections_counter: Rc<()> = Rc::default();
        // Incoming connections handler
        let listener = TcpListener::bind(&listen_address, &handle)?;
        let network_tx = network_tx.clone();
        let server = listener.incoming().for_each(move |(sock, addr)| {
            let holder = Rc::downgrade(&incoming_connections_counter);
            // Check incoming connections count
            let connections_count = Rc::weak_count(&incoming_connections_counter);
            if connections_count > incoming_connections_limit {
                warn!(
                    "Rejected incoming connection with peer={}, \
                     connections limit reached.",
                    addr
                );
                return to_box(future::ok(()));
            }
            trace!("Accepted incoming connection with peer={}", addr);
            let stream = sock.framed(MessagesCodec::new(max_message_len));
            let (_, stream) = stream.split();
            let network_tx = network_tx.clone();
            let connection_handler = stream
                .into_future()
                .map_err(|e| e.0)
                .and_then(move |(raw, stream)|{
                    let raw = raw.ok_or_else(|| format_err!("Incomming socket closed."))?;
                    let signed = SignedMessage::verify_buffer(raw)?;
                    let (payload, message) = signed.into_message().into_parts();
                    match payload {
                        Protocol::Connect(payload) => Ok((Message::from_parts(payload, message)?, stream)),
                        other => bail!(
                            "First message is not Connect, got={:?}",
                            other
                            )
                    }
                })
                .and_then(move |(connect, stream)| {
                    trace!("Received handshake message={:?}", connect);
                    let event = NetworkEvent::PeerConnected(addr, connect);
                    let stream = network_tx
                        .clone()
                        .send(event)
                        .map_err(into_failure)
                        .and_then(move |_| Ok(stream))
                        .flatten_stream();

                    stream.for_each(move |raw| {
                        let event = NetworkEvent::MessageReceived(addr, raw);
                        network_tx.clone().send(event).map_err(into_failure).map(drop)
                    })
                })
                .map(|_| {
                    // Ensure that holder lives until the stream ends.
                    let _holder = holder;
                })
                .map_err(log_error);
            handle.spawn(to_box(connection_handler));
            to_box(future::ok(()))
        }).map_err(into_failure);

        Ok(Listener(to_box(server)))
    }
}

impl Future for Listener {
    type Item = ();
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

fn conn_fut<F>(fut: F) -> Box<Future<Item = mpsc::Sender<SignedMessage>, Error = failure::Error>>
where
    F: Future<Item = mpsc::Sender<SignedMessage>, Error = failure::Error> + 'static,
{
    Box::new(fut)
}
