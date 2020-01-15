// Copyright 2020 The Exonum Team
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

use exonum::{
    crypto::{
        x25519::{self, into_x25519_public_key},
        PublicKey,
    },
    messages::{SignedMessage, Verified},
};
use failure::{bail, ensure, format_err};
use futures::{
    future::{self, err, Either},
    stream::{SplitSink, SplitStream},
    sync::mpsc,
    unsync, Future, IntoFuture, Sink, Stream,
};
use log::{error, trace, warn};
use tokio::net::{TcpListener, TcpStream};
use tokio_codec::Framed;
use tokio_core::reactor::Handle;
use tokio_retry::{
    strategy::{jitter, FixedInterval},
    Retry,
};

use std::{cell::RefCell, collections::HashMap, net::SocketAddr, rc::Rc, time::Duration};

use super::{error::log_error, to_box};
use crate::{
    events::{
        codec::MessagesCodec,
        error::into_failure,
        noise::{Handshake, HandshakeParams, NoiseHandshake},
    },
    messages::{Connect, Message, Service},
    state::SharedConnectList,
    NetworkConfiguration,
};

const OUTGOING_CHANNEL_SIZE: usize = 10;

#[derive(Debug, Clone)]
pub enum ConnectedPeerAddr {
    In(SocketAddr),
    Out(String, SocketAddr),
}

impl ConnectedPeerAddr {
    pub fn is_incoming(&self) -> bool {
        match self {
            ConnectedPeerAddr::In(_) => true,
            ConnectedPeerAddr::Out(_, _) => false,
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum NetworkEvent {
    MessageReceived(Vec<u8>),
    PeerConnected(ConnectedPeerAddr, Verified<Connect>),
    PeerDisconnected(PublicKey),
    UnableConnectToPeer(PublicKey),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum NetworkRequest {
    SendMessage(PublicKey, SignedMessage),
    // TODO: This variant is never constructed in main code. Is it necessary? (ECR-4118)
    DisconnectWithPeer(PublicKey),
    // TODO: This variant is never constructed in main code. Is it necessary? (ECR-4118)
    Shutdown,
}

#[derive(Debug)]
pub struct NetworkPart {
    pub our_connect_message: Verified<Connect>,
    pub listen_address: SocketAddr,
    pub network_config: NetworkConfiguration,
    pub max_message_len: u32,
    pub network_requests: (mpsc::Sender<NetworkRequest>, mpsc::Receiver<NetworkRequest>),
    pub network_tx: mpsc::Sender<NetworkEvent>,
    pub(crate) connect_list: SharedConnectList,
}

#[derive(Clone, Debug)]
struct ConnectionPoolEntry {
    sender: mpsc::Sender<SignedMessage>,
    address: ConnectedPeerAddr,
}

#[derive(Clone, Debug)]
struct ConnectionPool {
    peers: Rc<RefCell<HashMap<PublicKey, ConnectionPoolEntry>>>,
}

impl ConnectionPool {
    fn new() -> Self {
        ConnectionPool {
            peers: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    fn count_outgoing(&self) -> usize {
        let peers = self.peers.borrow();
        peers
            .iter()
            .filter(|(_, e)| !e.address.is_incoming())
            .count()
    }

    fn add(
        &self,
        key: &PublicKey,
        address: ConnectedPeerAddr,
        sender: mpsc::Sender<SignedMessage>,
    ) {
        let mut peers = self.peers.borrow_mut();
        peers.insert(*key, ConnectionPoolEntry { sender, address });
    }

    fn contains(&self, address: &PublicKey) -> bool {
        let peers = self.peers.borrow();
        peers.get(address).is_some()
    }

    fn remove(&self, address: &PublicKey) -> Option<ConnectedPeerAddr> {
        let mut peers = self.peers.borrow_mut();
        peers.remove(address).map(|o| o.address)
    }

    fn add_incoming_address(
        &self,
        key: &PublicKey,
        address: &ConnectedPeerAddr,
    ) -> mpsc::Receiver<SignedMessage> {
        let (sender_tx, receiver_rx) = mpsc::channel::<SignedMessage>(OUTGOING_CHANNEL_SIZE);
        self.add(key, address.clone(), sender_tx);
        receiver_rx
    }

    fn send_message(
        &self,
        address: &PublicKey,
        message: SignedMessage,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let address = *address;
        let sender_tx = self.peers.borrow();
        let write_pool = self.clone();

        if let Some(entry) = sender_tx.get(&address) {
            let sender = &entry.sender;
            Either::A(
                sender
                    .clone()
                    .send(message)
                    .map(drop)
                    .or_else(move |e| {
                        log_error(e);
                        write_pool.remove(&address);
                        Ok(())
                    })
                    .map(drop),
            )
        } else {
            Either::B(future::ok(()))
        }
    }

    fn disconnect_with_peer(
        &self,
        key: &PublicKey,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        if self.remove(key).is_some() {
            let send_disconnected = network_tx
                .clone()
                .send(NetworkEvent::PeerDisconnected(*key))
                .map_err(|_| format_err!("can't send disconnect"))
                .map(drop);
            Either::A(send_disconnected)
        } else {
            Either::B(future::ok(()))
        }
    }
}

struct Connection {
    handle: Handle,
    socket: Framed<TcpStream, MessagesCodec>,
    receiver_rx: mpsc::Receiver<SignedMessage>,
    address: ConnectedPeerAddr,
    key: PublicKey,
}

impl Connection {
    fn new(
        handle: Handle,
        socket: Framed<TcpStream, MessagesCodec>,
        receiver_rx: mpsc::Receiver<SignedMessage>,
        address: ConnectedPeerAddr,
        key: PublicKey,
    ) -> Self {
        Connection {
            handle,
            socket,
            receiver_rx,
            address,
            key,
        }
    }
}

#[derive(Clone)]
struct NetworkHandler {
    listen_address: SocketAddr,
    pool: ConnectionPool,
    handle: Handle,
    network_config: NetworkConfiguration,
    network_tx: mpsc::Sender<NetworkEvent>,
    handshake_params: HandshakeParams,
    connect_list: SharedConnectList,
}

impl NetworkHandler {
    fn new(
        handle: Handle,
        address: SocketAddr,
        connection_pool: ConnectionPool,
        network_config: NetworkConfiguration,
        network_tx: mpsc::Sender<NetworkEvent>,
        handshake_params: HandshakeParams,
        connect_list: SharedConnectList,
    ) -> Self {
        NetworkHandler {
            handle,
            listen_address: address,
            pool: connection_pool,
            network_config,
            network_tx,
            handshake_params,
            connect_list,
        }
    }

    fn listener(self) -> impl Future<Item = (), Error = failure::Error> {
        let listen_address = self.listen_address;
        let server = TcpListener::bind(&listen_address).unwrap().incoming();
        let pool = self.pool.clone();

        let handshake_params = self.handshake_params.clone();
        let network_tx = self.network_tx.clone();
        let handle = self.handle.clone();

        // Incoming connections limiter
        let incoming_connections_limit = self.network_config.max_incoming_connections;
        // The reference counter is used to automatically count the number of the open connections.
        let incoming_connections_counter: Rc<()> = Rc::default();

        server
            .map_err(into_failure)
            .for_each(move |incoming_connection| {
                let address = incoming_connection
                    .peer_addr()
                    .expect("Remote peer address resolve failed");
                let conn_addr = ConnectedPeerAddr::In(address);
                let pool = pool.clone();
                let network_tx = network_tx.clone();
                let handle = handle.clone();

                let handshake = NoiseHandshake::responder(&handshake_params, &listen_address);
                let holder = incoming_connections_counter.clone();
                // Check incoming connections count
                let connections_count = Rc::strong_count(&incoming_connections_counter) - 1;
                if connections_count >= incoming_connections_limit {
                    warn!(
                        "Rejected incoming connection with peer={}, \
                         connections limit reached.",
                        address
                    );
                    return Ok(());
                }

                let connect_list = self.connect_list.clone();
                let listener = handshake
                    .listen(incoming_connection)
                    .and_then(move |(socket, raw, key)| (Ok(socket), Self::parse_connect_msg(Some(raw), key)))
                    .and_then(move |(socket, message)| {
                        if pool.contains(&message.author()) {
                            Box::new(future::ok(()))
                        } else if connect_list.is_peer_allowed(&message.author()) {
                            let receiver_rx =
                                pool.add_incoming_address(&message.author(), &conn_addr);
                            let connection = Connection::new(
                                handle.clone(),
                                socket,
                                receiver_rx,
                                conn_addr,
                                message.author(),
                            );
                            to_box(Self::handle_connection(
                                connection,
                                message,
                                pool,
                                &network_tx,
                            ))
                        } else {
                            warn!( "Rejecting incoming connection with peer={} public_key={}, peer is not in the ConnectList",
                                   address, message.author()
                            );
                            Box::new(future::ok(()))
                        }
                    })
                    .map(|_| {
                        drop(holder);
                    })
                    .map_err(log_error);

                self.handle.spawn(listener);
                Ok(())
            })
    }

    fn connect(
        &self,
        key: PublicKey,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let handshake_params = handshake_params.clone();
        let handle = self.handle.clone();
        let network_tx = self.network_tx.clone();
        let network_config = self.network_config;
        let timeout = self.network_config.tcp_connect_retry_timeout;
        let max_tries = self.network_config.tcp_connect_max_retries as usize;
        let max_connections = self.network_config.max_outgoing_connections;
        let strategy = FixedInterval::from_millis(timeout)
            .map(jitter)
            .take(max_tries);

        let unresolved_address = self.connect_list.find_address_by_key(&key);

        if let Some(unresolved_address) = unresolved_address {
            let action = {
                let unresolved_address = unresolved_address.clone();
                move || tokio_dns::TcpStream::connect(unresolved_address.as_str())
            };

            let (sender_tx, receiver_rx) = mpsc::channel::<SignedMessage>(OUTGOING_CHANNEL_SIZE);
            let pool = self.pool.clone();
            Either::A(
                Retry::spawn(strategy, action)
                    .map_err(into_failure)
                    .and_then(move |socket| Self::configure_socket(socket, network_config))
                    .and_then(move |outgoing_connection| {
                        Self::build_handshake_initiator(outgoing_connection, key, &handshake_params)
                    })
                    .and_then(move |(socket, raw, key)| {
                        (Ok(socket), Self::parse_connect_msg(Some(raw), key))
                    })
                    .and_then(move |(socket, message)| {
                        let connection_limit_reached = pool.count_outgoing() >= max_connections;
                        if pool.contains(&message.author()) || connection_limit_reached {
                            Box::new(future::ok(()))
                        } else {
                            let addr = match socket.get_ref().peer_addr() {
                                Ok(addr) => addr,
                                Err(e) => {
                                    return Box::new(err(format_err!(
                                        "Couldn't take peer addr from socket = {}",
                                        e
                                    )))
                                        as Box<dyn Future<Error = failure::Error, Item = ()>>;
                                }
                            };
                            let conn_addr = ConnectedPeerAddr::Out(unresolved_address, addr);
                            pool.add(&key, conn_addr.clone(), sender_tx);
                            let connection = Connection::new(
                                handle,
                                socket,
                                receiver_rx,
                                conn_addr,
                                message.author(),
                            );
                            to_box(Self::handle_connection(
                                connection,
                                message,
                                pool,
                                &network_tx,
                            ))
                        }
                    })
                    .map(drop),
            )
        } else {
            Either::B(err(format_err!(
                "Trying to connect to peer not from ConnectList key={}",
                key
            )))
        }
    }

    fn process_messages(
        pool: &ConnectionPool,
        handle: &Handle,
        connection: Connection,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> Result<(), failure::Error> {
        let (sink, stream) = connection.socket.split();

        let incoming = Self::process_incoming_messages(
            stream,
            pool.clone(),
            &connection.key,
            network_tx.clone(),
        );

        let outgoing = Self::process_outgoing_messages(sink, connection.receiver_rx);

        handle.spawn(incoming);
        handle.spawn(outgoing);
        Ok(())
    }

    fn process_outgoing_messages<S>(
        sink: SplitSink<S>,
        receiver_rx: mpsc::Receiver<SignedMessage>,
    ) -> impl Future<Item = (), Error = ()>
    where
        S: Sink<SinkItem = SignedMessage, SinkError = failure::Error>,
    {
        receiver_rx
            .map_err(|_| format_err!("Receiver is gone."))
            .forward(sink)
            .map(drop)
            .map_err(|e| {
                error!("Connection terminated: {}: {}", e, e.find_root_cause());
            })
    }

    fn process_incoming_messages<S>(
        stream: SplitStream<S>,
        pool: ConnectionPool,
        key: &PublicKey,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = (), Error = ()>
    where
        S: Stream<Item = Vec<u8>, Error = failure::Error>,
    {
        let key = *key;
        network_tx
            .clone()
            .sink_map_err(into_failure)
            .send_all(stream.map(NetworkEvent::MessageReceived))
            .then(move |_| pool.disconnect_with_peer(&key, &network_tx))
            .map_err(|e| {
                error!("Connection terminated: {}: {}", e, e.find_root_cause());
            })
    }

    fn configure_socket(
        socket: TcpStream,
        network_config: NetworkConfiguration,
    ) -> Result<TcpStream, failure::Error> {
        socket.set_nodelay(network_config.tcp_nodelay)?;
        let duration = network_config.tcp_keep_alive.map(Duration::from_millis);
        socket.set_keepalive(duration)?;
        Ok(socket)
    }

    fn handle_connection(
        connection: Connection,
        message: Verified<Connect>,
        pool: ConnectionPool,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        trace!("Established connection with peer={:?}", connection.address);
        let handle = connection.handle.clone();
        Self::send_peer_connected_event(&connection.address, message, &network_tx).and_then(
            move |network_tx| Self::process_messages(&pool, &handle, connection, &network_tx),
        )
    }

    fn parse_connect_msg(
        raw: Option<Vec<u8>>,
        key: x25519::PublicKey,
    ) -> Result<Verified<Connect>, failure::Error> {
        let raw = raw.ok_or_else(|| format_err!("Incoming socket closed"))?;
        let message = Message::from_raw_buffer(raw)?;
        let connect: Verified<Connect> = match message {
            Message::Service(Service::Connect(connect)) => connect,
            other => bail!(
                "First message from a remote peer is not Connect, got={:?}",
                other
            ),
        };
        let author = into_x25519_public_key(connect.author());

        ensure!(
            author == key,
            "Connect message public key doesn't match with the received peer key"
        );

        Ok(connect)
    }

    pub fn request_handler(
        self,
        receiver: mpsc::Receiver<NetworkRequest>,
        cancel_handler: unsync::oneshot::Sender<()>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let mut cancel_sender = Some(cancel_handler);
        let handle = self.handle.clone();

        let handler = receiver.for_each(move |request| {
            let fut = match request {
                NetworkRequest::SendMessage(key, message) => {
                    to_box(self.handle_send_message(&key, message))
                }
                NetworkRequest::DisconnectWithPeer(peer) => {
                    to_box(self.pool.disconnect_with_peer(&peer, &self.network_tx))
                }
                NetworkRequest::Shutdown => to_box(
                    cancel_sender
                        .take()
                        .ok_or_else(|| format_err!("shutdown twice"))
                        .into_future(),
                ),
            }
            .map_err(log_error);

            handle.spawn(fut);
            Ok(())
        });

        handler.map_err(|_| format_err!("Error while processing outgoing Network Requests"))
    }

    fn handle_send_message(
        &self,
        address: &PublicKey,
        message: SignedMessage,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let pool = self.pool.clone();

        if pool.contains(address) {
            to_box(pool.send_message(address, message))
        } else if self.can_create_connections() {
            to_box(self.create_new_connection(*address, message))
        } else {
            to_box(self.send_unable_connect_event(address))
        }
    }

    fn create_new_connection(
        &self,
        key: PublicKey,
        message: SignedMessage,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let pool = self.pool.clone();
        let connect = self.handshake_params.connect.clone();
        self.connect(key, &self.handshake_params)
            .and_then(move |_| {
                if &message == connect.as_raw() {
                    Either::A(future::ok(()))
                } else {
                    Either::B(pool.send_message(&key, message))
                }
            })
    }

    fn send_peer_connected_event(
        address: &ConnectedPeerAddr,
        message: Verified<Connect>,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = mpsc::Sender<NetworkEvent>, Error = failure::Error> {
        let peer_connected = NetworkEvent::PeerConnected(address.clone(), message);
        network_tx
            .clone()
            .send(peer_connected)
            .map_err(into_failure)
    }

    fn can_create_connections(&self) -> bool {
        self.pool.count_outgoing() < self.network_config.max_outgoing_connections
    }

    fn send_unable_connect_event(
        &self,
        peer: &PublicKey,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let event = NetworkEvent::UnableConnectToPeer(*peer);
        self.network_tx
            .clone()
            .send(event)
            .map(drop)
            .map_err(|_| format_err!("can't send network event"))
    }

    fn build_handshake_initiator(
        stream: TcpStream,
        key: PublicKey,
        handshake_params: &HandshakeParams,
    ) -> impl Future<
        Item = (Framed<TcpStream, MessagesCodec>, Vec<u8>, x25519::PublicKey),
        Error = failure::Error,
    > {
        let mut handshake_params = handshake_params.clone();
        handshake_params.set_remote_key(key);
        NoiseHandshake::initiator(&handshake_params, &stream.peer_addr().unwrap()).send(stream)
    }
}

impl NetworkPart {
    pub fn run(
        self,
        handle: &Handle,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let listen_address = self.listen_address;
        // `cancel_sender` is converted to future when we receive
        // `NetworkRequest::Shutdown` causing its being completed with error.
        // After that completes `cancel_handler` and event loop stopped.
        let (cancel_sender, cancel_handler) = unsync::oneshot::channel::<()>();

        let handler = NetworkHandler::new(
            handle.clone(),
            listen_address,
            ConnectionPool::new(),
            self.network_config,
            self.network_tx.clone(),
            handshake_params.clone(),
            self.connect_list.clone(),
        );

        let listener = handler.clone().listener();
        let request_handler = handler.request_handler(self.network_requests.1, cancel_sender);

        let cancel_handler = cancel_handler.or_else(|e| {
            trace!("Requests handler closed: {}", e);
            Ok(())
        });

        listener
            .join(request_handler)
            .map(drop)
            .select(cancel_handler)
            .map_err(|(e, _)| e)
            .map(drop)
    }
}
