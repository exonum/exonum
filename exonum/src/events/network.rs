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

use failure;
use futures::{
    future, future::{err, Either}, stream::{SplitSink, SplitStream}, sync::mpsc, unsync, Future,
    IntoFuture, Sink, Stream,
};
use tokio::net::{TcpListener, TcpStream};
use tokio_codec::Framed;
use tokio_core::reactor::Handle;

use tokio_retry::{
    strategy::{jitter, FixedInterval}, Retry,
};

use std::{cell::RefCell, collections::HashMap, net::SocketAddr, rc::Rc, time::Duration};

use super::{error::log_error, to_box};
use crypto::PublicKey;
use events::{
    codec::MessagesCodec, error::into_failure, noise::{Handshake, HandshakeParams, NoiseHandshake},
};
use helpers::Milliseconds;
use messages::{Any, Connect, Message, RawMessage};

const OUTGOING_CHANNEL_SIZE: usize = 10;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ConnectionType {
    Incoming,
    Outgoing,
}

#[derive(Debug)]
pub struct Connection {
    pub connection_type: ConnectionType,
    pub message: Connect,
}

impl Connection {
    pub fn address(&self) -> SocketAddr {
        self.message.addr()
    }

    pub fn public_key(&self) -> &PublicKey {
        self.message.pub_key()
    }

    pub fn connect(&self) -> &Connect {
        &self.message
    }

    pub fn connection_type(&self) -> ConnectionType {
        self.connection_type
    }
}

#[derive(Debug)]
pub enum NetworkEvent {
    MessageReceived(SocketAddr, RawMessage),
    PeerConnected(SocketAddr, Connection),
    PeerDisconnected(SocketAddr),
    UnableConnectToPeer(SocketAddr),
}

#[derive(Debug, Clone)]
pub enum NetworkRequest {
    SendMessage(SocketAddr, RawMessage),
    DisconnectWithPeer(SocketAddr),
    Shutdown,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct NetworkConfiguration {
    // TODO: Think more about config parameters. (ECR-162)
    pub max_incoming_connections: usize,
    pub max_outgoing_connections: usize,
    pub tcp_nodelay: bool,
    pub tcp_keep_alive: Option<u64>,
    pub tcp_connect_retry_timeout: Milliseconds,
    pub tcp_connect_max_retries: u64,
}

impl Default for NetworkConfiguration {
    fn default() -> Self {
        Self {
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
    pub our_connect_message: Connect,
    pub listen_address: SocketAddr,
    pub network_config: NetworkConfiguration,
    pub max_message_len: u32,
    pub network_requests: (mpsc::Sender<NetworkRequest>, mpsc::Receiver<NetworkRequest>),
    pub network_tx: mpsc::Sender<NetworkEvent>,
}

#[derive(Clone, Debug)]
struct ConnectionPool {
    peers: Rc<RefCell<HashMap<SocketAddr, mpsc::Sender<RawMessage>>>>,
}

impl ConnectionPool {
    fn new() -> Self {
        ConnectionPool {
            peers: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    fn len(&self) -> usize {
        self.peers.borrow().len()
    }

    fn add(&self, address: &SocketAddr, sender: mpsc::Sender<RawMessage>) {
        let mut peers = self.peers.borrow_mut();
        peers.insert(*address, sender);
    }

    fn contains(&self, address: &SocketAddr) -> bool {
        let peers = self.peers.borrow();
        peers.get(address).is_some()
    }

    fn remove(&self, address: &SocketAddr) {
        let mut peers = self.peers.borrow_mut();
        peers.remove(address);
    }

    fn add_incoming_address(&self, remote_address: &SocketAddr) -> mpsc::Receiver<RawMessage> {
        let (sender_tx, receiver_rx) = mpsc::channel::<RawMessage>(OUTGOING_CHANNEL_SIZE);
        self.add(&remote_address, sender_tx);
        receiver_rx
    }

    fn disconnect_with_peer(
        &self,
        peer: &SocketAddr,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        self.remove(&peer);
        network_tx
            .clone()
            .send(NetworkEvent::PeerDisconnected(*peer))
            .map_err(|_| format_err!("can't send disconnect"))
            .map(drop)
    }

    fn send_message(
        &self,
        address: &SocketAddr,
        message: &RawMessage,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let address = *address;
        let sender_tx = self.peers.borrow();
        let write_pool = self.clone();

        if let Some(sender) = sender_tx.get(&address) {
            Either::A(
                sender
                    .clone()
                    .send(message.clone())
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
}

struct ConnectionParams {
    handle: Handle,
    address: SocketAddr,
    socket: Framed<TcpStream, MessagesCodec>,
    receiver_rx: mpsc::Receiver<RawMessage>,
}

impl ConnectionParams {
    fn new(
        handle: Handle,
        address: SocketAddr,
        socket: Framed<TcpStream, MessagesCodec>,
        receiver_rx: mpsc::Receiver<RawMessage>,
    ) -> Self {
        ConnectionParams {
            handle,
            address,
            socket,
            receiver_rx,
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
}

impl NetworkHandler {
    fn new(
        handle: Handle,
        address: SocketAddr,
        connection_pool: ConnectionPool,
        network_config: NetworkConfiguration,
        network_tx: mpsc::Sender<NetworkEvent>,
        handshake_params: HandshakeParams,
    ) -> Self {
        NetworkHandler {
            handle,
            listen_address: address,
            pool: connection_pool,
            network_config,
            network_tx,
            handshake_params,
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

                let listener = handshake
                    .listen(incoming_connection)
                    .and_then(move |(socket, raw)| (Ok(socket), Self::parse_connect_msg(Some(raw))))
                    .and_then(move |(socket, message)| {
                        let receiver_rx = pool.add_incoming_address(&message.addr());
                        Ok((pool, socket, message, receiver_rx))
                    })
                    .and_then(move |(pool, socket, message, receiver_rx)| {
                        let connection_params =
                            ConnectionParams::new(handle, message.addr(), socket, receiver_rx);
                        let connection = Connection {
                            message,
                            connection_type: ConnectionType::Incoming,
                        };
                        Self::handle_connection(connection_params, connection, pool, &network_tx)
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
        address: SocketAddr,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let handshake_params = handshake_params.clone();
        let handle = self.handle.clone();
        let network_tx = self.network_tx.clone();
        let network_config = self.network_config;
        let timeout = self.network_config.tcp_connect_retry_timeout;
        let max_tries = self.network_config.tcp_connect_max_retries as usize;
        let strategy = FixedInterval::from_millis(timeout)
            .map(jitter)
            .take(max_tries);

        let action = move || TcpStream::connect(&address);

        let (sender_tx, receiver_rx) = mpsc::channel::<RawMessage>(OUTGOING_CHANNEL_SIZE);
        self.pool.add(&address, sender_tx);

        let pool = self.pool.clone();

        Retry::spawn(strategy, action)
            .map_err(into_failure)
            .and_then(move |socket| Self::configure_socket(socket, network_config))
            .and_then(move |outgoing_connection| {
                Self::build_handshake_initiator(outgoing_connection, &address, &handshake_params)
            })
            .and_then(move |(socket, raw)| (Ok(socket), Self::parse_connect_msg(Some(raw))))
            .and_then(move |(socket, message)| {
                let connection_params =
                    ConnectionParams::new(handle.clone(), address, socket, receiver_rx);
                let connection = Connection {
                    message,
                    connection_type: ConnectionType::Outgoing,
                };
                Self::handle_connection(connection_params, connection, pool, &network_tx)
            })
            .map(drop)
    }

    fn process_messages(
        pool: &ConnectionPool,
        handle: &Handle,
        connection: ConnectionParams,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> Result<(), failure::Error> {
        let (sink, stream) = connection.socket.split();

        let incoming = Self::process_incoming_messages(
            stream,
            pool.clone(),
            &connection.address,
            network_tx.clone(),
        );

        let outgoing = Self::process_outgoing_messages(sink, connection.receiver_rx);

        handle.spawn(incoming);
        handle.spawn(outgoing);
        Ok(())
    }

    fn process_outgoing_messages<S>(
        sink: SplitSink<S>,
        receiver_rx: mpsc::Receiver<RawMessage>,
    ) -> impl Future<Item = (), Error = ()>
    where
        S: Sink<SinkItem = RawMessage, SinkError = failure::Error>,
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
        address: &SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = (), Error = ()>
    where
        S: Stream<Item = RawMessage, Error = failure::Error>,
    {
        let address = *address;
        network_tx
            .clone()
            .sink_map_err(into_failure)
            .send_all(stream.map(move |message| NetworkEvent::MessageReceived(address, message)))
            .then(move |_| pool.disconnect_with_peer(&address, &network_tx))
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
        connection_params: ConnectionParams,
        connection: Connection,
        pool: ConnectionPool,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        trace!(
            "Established connection with peer={}",
            connection_params.address
        );
        let handle = connection_params.handle.clone();
        Self::send_peer_connected_event(&connection_params.address, connection, &network_tx)
            .and_then(move |network_tx| {
                Self::process_messages(&pool, &handle, connection_params, &network_tx)
            })
    }

    fn parse_connect_msg(raw: Option<RawMessage>) -> Result<Connect, failure::Error> {
        let raw = raw.ok_or_else(|| format_err!("Incoming socket closed"))?;
        let message = Any::from_raw(raw).map_err(into_failure)?;
        match message {
            Any::Connect(connect) => Ok(connect),
            other => bail!(
                "First message from a remote peer is not Connect, got={:?}",
                other
            ),
        }
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
                NetworkRequest::SendMessage(address, message) => {
                    to_box(self.handle_send_message(&address, message))
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
            }.map_err(log_error);

            handle.spawn(fut);
            Ok(())
        });

        handler.map_err(|_| format_err!("Error while processing outgoing Network Requests"))
    }

    fn handle_send_message(
        &self,
        address: &SocketAddr,
        message: RawMessage,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let pool = self.pool.clone();

        if pool.contains(&address) {
            to_box(pool.send_message(&address, &message))
        } else if self.can_create_connections() {
            to_box(self.create_new_connection(&address, message))
        } else {
            to_box(self.send_unable_connect_event(&address))
        }
    }

    fn create_new_connection(
        &self,
        address: &SocketAddr,
        message: RawMessage,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let pool = self.pool.clone();
        let address = *address;
        let connect = self.handshake_params.connect.clone();
        self.connect(address, &self.handshake_params)
            .and_then(move |_| {
                if &message == connect.raw() {
                    Either::A(future::ok(()))
                } else {
                    Either::B(pool.send_message(&address, &message))
                }
            })
    }

    fn send_peer_connected_event(
        address: &SocketAddr,
        connection: Connection,
        network_tx: &mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = mpsc::Sender<NetworkEvent>, Error = failure::Error> {
        let peer_connected = NetworkEvent::PeerConnected(*address, connection);
        network_tx
            .clone()
            .send(peer_connected)
            .map_err(into_failure)
    }

    fn can_create_connections(&self) -> bool {
        self.pool.len() < self.network_config.max_outgoing_connections
    }

    fn send_unable_connect_event(
        &self,
        peer: &SocketAddr,
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
        peer: &SocketAddr,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (Framed<TcpStream, MessagesCodec>, RawMessage), Error = failure::Error>
    {
        let connect_list = &handshake_params.connect_list.clone();
        if let Some(remote_public_key) = connect_list.find_key_by_address(&peer) {
            let mut handshake_params = handshake_params.clone();
            handshake_params.set_remote_key(remote_public_key);
            NoiseHandshake::initiator(&handshake_params, peer).send(stream)
        } else {
            Box::new(err(format_err!(
                "Attempt to connect to the peer with address {:?} which \
                 is not in the ConnectList",
                peer
            )))
        }
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
