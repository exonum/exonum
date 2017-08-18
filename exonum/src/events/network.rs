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

use futures::{Future, Stream, Sink, IntoFuture};
use futures::future::Either;
use futures::sync::mpsc;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::{Core, Timeout};
use tokio_io::AsyncRead;

use std::net::SocketAddr;
use std::time::Duration;
use std::collections::HashMap;

use messages::{Any, Connect, RawMessage};
use node::{ExternalMessage, NodeTimeout};

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
}

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

impl NetworkPart {
    pub fn run(self) -> Result<(), ()> {
        let mut core = Core::new().unwrap();

        // Outgoing connections handler
        let mut outgoing_connections: HashMap<SocketAddr, mpsc::Sender<RawMessage>> =
            HashMap::new();

        // Requests handler
        let handle = core.handle();
        let network_tx = self.network_tx.clone();
        let requests_tx = self.network_requests.0.clone();
        let requests_handle = self.network_requests.1.for_each(|request| {
            match request {
                NetworkRequest::SendMessage(peer, msg) => {
                    let conn_tx = if let Some(conn_tx) = outgoing_connections.get(&peer).cloned() {
                        conn_tx
                    } else {
                        let (conn_tx, conn_rx) = mpsc::channel(10);
                        outgoing_connections.insert(peer, conn_tx.clone());

                        let requests_tx = requests_tx.clone();
                        let connect_handle = TcpStream::connect(&peer, &handle)
                            .and_then(move |sock| {
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
            }

            Ok(())
        });

        // Incoming connections handler
        let listener = TcpListener::bind(&self.listen_address, &core.handle()).unwrap();
        let network_tx = network_tx.clone();
        let server = listener
            .incoming()
            .fold(network_tx, move |network_tx, (sock, addr)| {
                info!("Accepted incoming connection with peer={}", addr);

                let stream = sock.framed(MessagesCodec);
                let (_, stream) = stream.split();
                let network_tx = network_tx.clone();
                stream
                    .into_future()
                    .map_err(|e| e.0)
                    .and_then(move |(raw, stream)| {
                        let msg = raw.map(Any::from_raw);
                        if let Some(Ok(Any::Connect(msg))) = msg {
                            Ok((msg, stream))
                        } else {
                            Err(other_error("First message is not Connect"))
                        }
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
                    .map_err(into_other)
            })
            .map(forget_result)
            .map_err(log_error);
        core.handle().spawn(server);

        core.run(requests_handle)
    }
}
