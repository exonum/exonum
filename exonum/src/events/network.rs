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

use futures::{Future, Stream, Sink, IntoFuture, Poll, Async};
use futures::future::Either;
use futures::sync::mpsc;
use futures::unsync;
use tokio_core::net::{TcpListener, TcpStream, TcpStreamNew};
use tokio_core::reactor::{Core, Timeout, Handle, Interval};
use tokio_io::AsyncRead;

use std::cmp;
use std::io;
use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};
use std::collections::HashMap;

use messages::{Any, Connect, RawMessage};
use node::{ExternalMessage, NodeTimeout};
use helpers::Milliseconds;

use super::EventHandler;
use super::error::{other_error, result_ok, forget_result, into_other, log_error};
use super::codec::MessagesCodec;
use super::EventsAggregator;

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
            tcp_reconnect_timeout_max: 600_000,
        }
    }
}

#[derive(Debug)]
pub struct HandlerPart<H: EventHandler> {
    pub core: Core,
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

impl<H: EventHandler> HandlerPart<H> {
    pub fn run(self) -> Result<(), ()> {
        let mut core = self.core;
        let mut handler = self.handler;

        let events_handle = EventsAggregator::new(self.timeout_rx, self.network_rx, self.api_rx)
            .for_each(move |event| {
                handler.handle_event(event);
                Ok(())
            });
        core.run(events_handle)
    }
}

macro_rules! try_future_boxed {
    ($e:expr) =>(
        match $e {
            Ok(v) => v,
            Err(e) => {
                return Err(into_other(e)).into_future().boxed();
            }
        }
    )
}

impl NetworkPart {
    pub fn run(self) -> Result<(), ()> {
        let mut core = Core::new().unwrap();

        // Cancelation token
        let (cancel_sender, cancel_handler) = unsync::mpsc::channel(1);

        // Outgoing connections handler
        let mut outgoing_connections: HashMap<SocketAddr, mpsc::Sender<RawMessage>> =
            HashMap::new();

        // Requests handler
        let handle = core.handle();
        let network_tx = self.network_tx.clone();
        let requests_tx = self.network_requests.0.clone();
        let network_config = self.network_config;
        let requests_handle = self.network_requests.1.for_each(move |request| {
            let cancel_sender = cancel_sender.clone();
            match request {
                NetworkRequest::SendMessage(peer, msg) => {
                    let conn_tx = if let Some(conn_tx) = outgoing_connections.get(&peer).cloned() {
                        conn_tx
                    } else {
                        let (conn_tx, conn_rx) = mpsc::channel(10);
                        outgoing_connections.insert(peer, conn_tx.clone());

                        let requests_tx = requests_tx.clone();
                        let connect_handle = NewConnection::create(
                            peer,
                            handle.clone(),
                            network_config.tcp_reconnect_timeout,
                            network_config.tcp_reconnect_timeout_max,
                        ).and_then(move |sock| {
                            try_future_boxed!(sock.set_nodelay(network_config.tcp_nodelay));
                            try_future_boxed!(sock.set_keepalive(
                                network_config.tcp_keep_alive.map(Duration::from_millis),
                            ));

                            info!("Established connection with peer={}", peer);

                            let stream = sock.framed(MessagesCodec);
                            let (sink, stream) = stream.split();

                            let writer = conn_rx
                                .map_err(|_| other_error("Can't send data into socket"))
                                .forward(sink);
                            let reader = stream.for_each(result_ok).map_err(into_other);

                            reader
                                .select2(writer)
                                .map_err(|_| other_error("Socket error"))
                                .and_then(|res| match res {
                                    Either::A((_, _reader)) => Ok(()).into_future(),
                                    Either::B((_, _writer)) => Ok(()).into_future(),
                                })
                                .boxed()
                        })
                            .then(move |res| {
                                info!("Connection with peer={} closed, reason={:?}", peer, res);
                                // outgoing_connections.remove(&peer);

                                let request = NetworkRequest::DisconnectWithPeer(peer);
                                requests_tx
                                    .clone()
                                    .send(request)
                                    .map(forget_result)
                                    .map_err(into_other)
                            })
                            .map_err(log_error);
                        handle.spawn(connect_handle);
                        conn_tx
                    };

                    let duration = Duration::from_secs(5);
                    let send_timeout = Timeout::new(duration, &handle)
                        .unwrap()
                        .and_then(result_ok)
                        .map_err(|_| other_error("Can't timeout"));

                    let send_handle = conn_tx.send(msg).map(forget_result).map_err(log_error);

                    let timeouted_connect = send_handle
                        .select2(send_timeout)
                        .map_err(|_| other_error("Unable to send message"))
                        .and_then(move |either| match either {
                            Either::A((send, _timeout_fut)) => Ok(send),
                            Either::B((_, _connect_fut)) => Err(other_error("Send timeout")),
                        })
                        .map_err(log_error);

                    handle.spawn(timeouted_connect);
                }
                NetworkRequest::DisconnectWithPeer(peer) => {
                    outgoing_connections.remove(&peer);

                    let event = NetworkEvent::PeerDisconnected(peer);
                    let event_handle = network_tx.clone().send(event).map(forget_result).map_err(
                        log_error,
                    );
                    handle.spawn(event_handle);
                }
                // Immediately stop the event loop.
                NetworkRequest::Shutdown => {
                    cancel_sender
                        .clone()
                        .send(())
                        .map(forget_result)
                        .map_err(log_error)
                        .wait()?
                }
            }

            Ok(())
        });

        // Incoming connections handler
        let listener = TcpListener::bind(&self.listen_address, &core.handle()).unwrap();
        let network_tx = self.network_tx.clone();
        let handle = core.handle();
        let server = listener
            .incoming()
            .for_each(move |(sock, addr)| {
                info!("Accepted incoming connection with peer={}", addr);

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
                        info!("Received handshake message={:?}", connect);

                        let event = NetworkEvent::PeerConnected(addr, connect);
                        let connect_event = network_tx.clone().send(event).map_err(into_other);

                        let network_tx = network_tx.clone();
                        let messages_stream = stream.for_each(move |raw| {
                            let event = NetworkEvent::MessageReceived(addr, raw);
                            network_tx.clone().send(event).map(forget_result).map_err(
                                into_other,
                            )
                        });

                        messages_stream
                            .join(connect_event)
                            .map(move |(_, stream)| stream)
                            .map_err(into_other)
                    })
                    .map(forget_result)
                    .map_err(log_error);
                handle.spawn(connection_handler);
                Ok(())
            })
            .map_err(log_error);

        core.handle().spawn(server);
        core.handle().spawn(requests_handle);
        core.run(
            cancel_handler
                .into_future()
                .map(|_| info!("Network thread shutdown"))
                .map_err(|_| error!("An error during shutdown occured")),
        )
    }
}

struct NewConnection {
    socket: Option<TcpStreamNew>,
    timeout: Interval,
    address: SocketAddr,
    handle: Handle,
    groth_factor: f32,
    start_time: SystemTime,
    reconnect_timeout: Milliseconds,
    reconnect_timeout_max: Milliseconds,
}

impl NewConnection {
    pub fn create(
        address: SocketAddr,
        handle: Handle,
        reconnect_timeout: Milliseconds,
        reconnect_timeout_max: Milliseconds,
    ) -> NewConnection {
        NewConnection {
            socket: Some(TcpStream::connect(&address, &handle)),
            timeout: Interval::new(Duration::from_millis(reconnect_timeout), &handle).unwrap(),
            address,
            handle,
            groth_factor: 1.5,
            start_time: SystemTime::now(),
            reconnect_timeout,
            reconnect_timeout_max,
        }
    }

    fn next_attempt(&mut self) -> Poll<TcpStream, io::Error> {
        self.start_time = SystemTime::now();
        if self.reconnect_timeout == self.reconnect_timeout_max {
            return Err(other_error(
                &format!("Maximum reconnect timeout reached for connection with the peer: {}",
                self.address,
            ),
            ));
        }

        self.reconnect_timeout = (self.reconnect_timeout as f32 * self.groth_factor) as
            Milliseconds;
        self.reconnect_timeout = cmp::min(self.reconnect_timeout, self.reconnect_timeout_max);
        self.socket = Some(TcpStream::connect(&self.address, &self.handle));
        self.timeout = Interval::new(Duration::from_millis(self.reconnect_timeout), &self.handle)
            .unwrap();

        trace!(
            "Add reconnect timeout to {}, delay = {} ms",
            self.address,
            self.reconnect_timeout
        );
        self.socket.as_mut().unwrap().poll()
    }

    fn poll_timeout(&mut self) -> Poll<TcpStream, io::Error> {
        match self.timeout.poll() {
            Ok(Async::Ready(_)) => self.next_attempt(),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(into_other(e)),
        }
    }
}

impl Future for NewConnection {
    type Item = TcpStream;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<TcpStream, io::Error> {
        if let Some(mut stream) = self.socket.take() {
            match stream.poll() {
                Ok(Async::Ready(stream)) => Ok(Async::Ready(stream)),
                Ok(Async::NotReady) => {
                    self.socket = Some(stream);
                    Ok(Async::NotReady)
                }
                Err(e) => {
                    trace!(
                        "An error occured during connecting to the peer: {}, error: {}",
                        self.address,
                        e
                    );
                    self.socket = None;
                    self.poll_timeout()
                }
            }
        } else {
            self.poll_timeout()
        }
    }
}

impl fmt::Debug for NewConnection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("NewConnection { .. }")
    }
}
