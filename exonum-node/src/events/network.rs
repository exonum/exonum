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

use anyhow::{bail, ensure, format_err};
use exonum::{
    crypto::{
        x25519::{self, into_x25519_public_key},
        PublicKey,
    },
    messages::{SignedMessage, Verified},
};
use futures::{channel::mpsc, future, prelude::*};
use futures_retry::{ErrorHandler, FutureRetry, RetryPolicy};
use rand::{thread_rng, Rng};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;

use std::{
    collections::HashMap,
    io,
    net::SocketAddr,
    ops,
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::{
    events::{
        codec::MessagesCodec,
        error::{into_failure, LogError},
        noise::{Handshake, HandshakeData, HandshakeParams, NoiseHandshake},
    },
    messages::{Connect, Message, Service},
    state::SharedConnectList,
    NetworkConfiguration,
};

const OUTGOING_CHANNEL_SIZE: usize = 10;

#[derive(Debug)]
struct ErrorAction {
    retry_timeout: Duration,
    max_retries: usize,
    description: String,
}

impl ErrorAction {
    fn new(config: &NetworkConfiguration, description: String) -> Self {
        Self {
            retry_timeout: Duration::from_millis(config.tcp_connect_retry_timeout),
            max_retries: config.tcp_connect_max_retries as usize,
            description,
        }
    }
}

impl ErrorHandler<io::Error> for ErrorAction {
    type OutError = io::Error;

    fn handle(&mut self, attempt: usize, e: io::Error) -> RetryPolicy<io::Error> {
        log::info!(
            "{} failed [Attempt: {}/{}]: {}",
            self.description,
            attempt,
            self.max_retries,
            e
        );

        if attempt >= self.max_retries {
            RetryPolicy::ForwardError(e)
        } else {
            let jitter = thread_rng().gen_range(0.5, 1.0);
            let timeout = self.retry_timeout.mul_f64(jitter);
            RetryPolicy::WaitRetry(timeout)
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConnectedPeerAddr {
    In(SocketAddr),
    Out(String, SocketAddr),
}

impl ConnectedPeerAddr {
    pub fn is_incoming(&self) -> bool {
        match self {
            Self::In(_) => true,
            Self::Out(_, _) => false,
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
    DisconnectWithPeer(PublicKey),
}

#[derive(Debug)]
pub struct NetworkPart {
    pub our_connect_message: Verified<Connect>,
    pub listen_address: SocketAddr,
    pub network_config: NetworkConfiguration,
    pub max_message_len: u32,
    pub network_requests: mpsc::Receiver<NetworkRequest>,
    pub network_tx: mpsc::Sender<NetworkEvent>,
    pub(crate) connect_list: SharedConnectList,
}

#[derive(Clone, Debug)]
struct ConnectionPoolEntry {
    sender: mpsc::Sender<SignedMessage>,
    address: ConnectedPeerAddr,
    // Connection ID assigned to the connection during instantiation. This ID is unique among
    // all connections and is used in `ConnectList::remove()` to figure out whether
    // it would make sense to remove a connection, or the request has been obsoleted.
    id: u64,
}

#[derive(Clone, Debug)]
struct SharedConnectionPool {
    inner: Arc<RwLock<ConnectionPool>>,
}

impl SharedConnectionPool {
    fn new(our_key: PublicKey) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ConnectionPool::new(our_key))),
        }
    }

    fn read(&self) -> impl ops::Deref<Target = ConnectionPool> + '_ {
        self.inner.read().unwrap()
    }

    fn write(&self) -> impl ops::DerefMut<Target = ConnectionPool> + '_ {
        self.inner.write().unwrap()
    }

    async fn send_message(&self, peer_key: &PublicKey, message: SignedMessage) {
        let maybe_peer_info = {
            // Ensure that we don't hold the lock across the `await` point.
            let peers = &self.inner.read().unwrap().peers;
            peers
                .get(peer_key)
                .map(|peer| (peer.sender.clone(), peer.id))
        };

        if let Some((mut sender, connection_id)) = maybe_peer_info {
            if sender.send(message).await.is_err() {
                log::warn!("Cannot send message to peer {}", peer_key);
                self.write().remove(peer_key, Some(connection_id));
            }
        }
    }

    fn create_connection(
        &self,
        peer_key: PublicKey,
        address: ConnectedPeerAddr,
        socket: Framed<TcpStream, MessagesCodec>,
    ) -> Option<Connection> {
        let mut guard = self.write();

        if guard.contains(&peer_key) && Self::ignore_connection(guard.our_key, peer_key) {
            log::info!("Ignoring connection to {:?} per priority rules", peer_key);
            return None;
        }

        let (receiver_rx, connection_id) = guard.add(peer_key, address.clone());
        Some(Connection {
            socket,
            receiver_rx,
            address,
            key: peer_key,
            id: connection_id,
        })
    }

    /// Provides a complete, anti-symmetric relation among two peers bound in a connection.
    /// This is used by the peers to decide which one of two connections are left alive
    /// if the peers connect to each other simultaneously.
    fn ignore_connection(our_key: PublicKey, their_key: PublicKey) -> bool {
        our_key[..] < their_key[..]
    }
}

#[derive(Debug)]
struct ConnectionPool {
    peers: HashMap<PublicKey, ConnectionPoolEntry>,
    our_key: PublicKey,
    next_connection_id: u64,
}

impl ConnectionPool {
    fn new(our_key: PublicKey) -> Self {
        Self {
            peers: HashMap::new(),
            our_key,
            next_connection_id: 0,
        }
    }

    fn count_incoming(&self) -> usize {
        self.peers
            .values()
            .filter(|entry| entry.address.is_incoming())
            .count()
    }

    fn count_outgoing(&self) -> usize {
        self.peers
            .values()
            .filter(|entry| entry.address.is_incoming())
            .count()
    }

    /// Adds a peer to the connection list.
    ///
    /// # Return value
    ///
    /// Returns the receiver for outgoing messages to the peer and the connection ID.
    fn add(
        &mut self,
        key: PublicKey,
        address: ConnectedPeerAddr,
    ) -> (mpsc::Receiver<SignedMessage>, u64) {
        let id = self.next_connection_id;
        let (sender, receiver_rx) = mpsc::channel(OUTGOING_CHANNEL_SIZE);
        let entry = ConnectionPoolEntry {
            sender,
            address,
            id,
        };

        self.next_connection_id += 1;
        self.peers.insert(key, entry);
        (receiver_rx, id)
    }

    fn contains(&self, address: &PublicKey) -> bool {
        self.peers.get(address).is_some()
    }

    /// Drops the connection to a peer. The request can be optionally filtered by the connection ID
    /// in order to avoid issuing obsolete requests.
    ///
    /// # Return value
    ///
    /// Returns `true` if the connection with the peer was dropped. If the connection with the
    /// peer was not dropped (either because it did not exist, or because
    /// the provided `connection_id` is outdated), returns `false`.
    fn remove(&mut self, address: &PublicKey, connection_id: Option<u64>) -> bool {
        if let Some(entry) = self.peers.get(address) {
            if connection_id.map_or(true, |id| id == entry.id) {
                self.peers.remove(address);
                return true;
            }
        }
        false
    }
}

struct Connection {
    socket: Framed<TcpStream, MessagesCodec>,
    receiver_rx: mpsc::Receiver<SignedMessage>,
    address: ConnectedPeerAddr,
    key: PublicKey,
    id: u64,
}

#[derive(Clone)]
struct NetworkHandler {
    listen_address: SocketAddr,
    pool: SharedConnectionPool,
    network_config: NetworkConfiguration,
    network_tx: mpsc::Sender<NetworkEvent>,
    handshake_params: HandshakeParams,
    connect_list: SharedConnectList,
}

impl NetworkHandler {
    fn new(
        address: SocketAddr,
        connection_pool: SharedConnectionPool,
        network_config: NetworkConfiguration,
        network_tx: mpsc::Sender<NetworkEvent>,
        handshake_params: HandshakeParams,
        connect_list: SharedConnectList,
    ) -> Self {
        Self {
            listen_address: address,
            pool: connection_pool,
            network_config,
            network_tx,
            handshake_params,
            connect_list,
        }
    }

    async fn listener(self) -> anyhow::Result<()> {
        let mut listener = TcpListener::bind(&self.listen_address).await?;
        let mut incoming_connections = listener.incoming();

        // Incoming connections limiter
        let incoming_connections_limit = self.network_config.max_incoming_connections;

        while let Some(mut socket) = incoming_connections.try_next().await? {
            let peer_address = match socket.peer_addr() {
                Ok(address) => address,
                Err(err) => {
                    log::warn!("Peer address resolution failed: {}", err);
                    continue;
                }
            };

            // Check incoming connections count.
            let connections_count = self.pool.read().count_incoming();
            if connections_count >= incoming_connections_limit {
                log::warn!(
                    "Rejected incoming connection with peer={}, connections limit reached.",
                    peer_address
                );
                continue;
            }

            let pool = self.pool.clone();
            let connect_list = self.connect_list.clone();
            let network_tx = self.network_tx.clone();
            let handshake = NoiseHandshake::responder(&self.handshake_params);

            let task = async move {
                let HandshakeData {
                    codec,
                    raw_message,
                    peer_key,
                } = handshake.listen(&mut socket).await?;

                let connect = Self::parse_connect_msg(raw_message, &peer_key)?;
                let peer_key = connect.author();
                if !connect_list.is_peer_allowed(&peer_key) {
                    bail!(
                        "Rejecting incoming connection with peer={} public_key={}, \
                         the peer is not in the connect list",
                        peer_address,
                        peer_key
                    );
                }

                let conn_addr = ConnectedPeerAddr::In(peer_address);
                let socket = Framed::new(socket, codec);
                let maybe_connection = pool.create_connection(peer_key, conn_addr, socket);
                if let Some(connection) = maybe_connection {
                    Self::handle_connection(connection, connect, pool, network_tx).await
                } else {
                    Ok(())
                }
            };

            tokio::spawn(task.unwrap_or_else(|err| log::warn!("{}", err)));
        }
        Ok(())
    }

    /// # Return value
    ///
    /// The returned future resolves when the connection is established. The connection processing
    /// is spawned onto `tokio` runtime.
    fn connect(
        &self,
        key: PublicKey,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Output = anyhow::Result<()>> {
        // Resolve peer key to an address.
        let maybe_address = self.connect_list.find_address_by_key(&key);
        let unresolved_address = if let Some(address) = maybe_address {
            address
        } else {
            let err = format_err!("Trying to connect to peer {} not from connect list", key);
            return future::err(err).left_future();
        };

        let max_connections = self.network_config.max_outgoing_connections;
        let mut handshake_params = handshake_params.clone();
        handshake_params.set_remote_key(key);
        let pool = self.pool.clone();
        let network_tx = self.network_tx.clone();

        let network_config = self.network_config;
        let description = format!(
            "Connecting to {} (remote address = {})",
            key, unresolved_address
        );
        let on_error = ErrorAction::new(&network_config, description);

        async move {
            let connect = || TcpStream::connect(&unresolved_address);
            // The second component in returned value / error is the number of retries,
            // which we ignore.
            let (mut socket, _) = FutureRetry::new(connect, on_error)
                .await
                .map_err(|(err, _)| err)?;

            let peer_address = match socket.peer_addr() {
                Ok(addr) => addr,
                Err(err) => {
                    let err = format_err!("Couldn't take peer addr from socket: {}", err);
                    return Err(err);
                }
            };

            Self::configure_socket(&mut socket, network_config)?;

            let HandshakeData {
                codec,
                raw_message,
                peer_key,
            } = NoiseHandshake::initiator(&handshake_params)
                .send(&mut socket)
                .await?;

            if pool.read().count_outgoing() >= max_connections {
                log::info!(
                    "Ignoring outgoing connection to {:?} because the connection limit ({}) \
                     is reached",
                    key,
                    max_connections
                );
                return Ok(());
            }

            let conn_addr = ConnectedPeerAddr::Out(unresolved_address, peer_address);
            let connect = Self::parse_connect_msg(raw_message, &peer_key)?;
            let socket = Framed::new(socket, codec);
            if let Some(connection) = pool.create_connection(key, conn_addr, socket) {
                let handler = Self::handle_connection(connection, connect, pool, network_tx);
                tokio::spawn(handler);
            }
            Ok(())
        }
        .right_future()
    }

    async fn process_messages(
        pool: SharedConnectionPool,
        connection: Connection,
        mut network_tx: mpsc::Sender<NetworkEvent>,
    ) {
        let (sink, stream) = connection.socket.split();
        let key = connection.key;
        let connection_id = connection.id;

        // Processing of incoming messages.
        let incoming = async move {
            let res = (&mut network_tx)
                .sink_map_err(into_failure)
                .send_all(&mut stream.map_ok(NetworkEvent::MessageReceived))
                .await;
            if pool.write().remove(&key, Some(connection_id)) {
                network_tx
                    .send(NetworkEvent::PeerDisconnected(key))
                    .await
                    .ok();
            }
            res
        };
        futures::pin_mut!(incoming);

        // Processing of outgoing messages.
        let outgoing = connection.receiver_rx.map(Ok).forward(sink);

        // Select the first future to terminate and drop the remaining one.
        let task = future::select(incoming, outgoing).map(|res| {
            if let (Err(err), _) = res.factor_first() {
                log::info!(
                    "Connection with peer {} terminated: {} (root cause: {})",
                    key,
                    err,
                    err.root_cause()
                );
            }
        });
        task.await
    }

    fn configure_socket(
        socket: &mut TcpStream,
        network_config: NetworkConfiguration,
    ) -> anyhow::Result<()> {
        socket.set_nodelay(network_config.tcp_nodelay)?;
        let duration = network_config.tcp_keep_alive.map(Duration::from_millis);
        socket.set_keepalive(duration)?;
        Ok(())
    }

    async fn handle_connection(
        connection: Connection,
        connect: Verified<Connect>,
        pool: SharedConnectionPool,
        mut network_tx: mpsc::Sender<NetworkEvent>,
    ) -> anyhow::Result<()> {
        let address = connection.address.clone();
        log::trace!("Established connection with peer {:?}", address);

        Self::send_peer_connected_event(address, connect, &mut network_tx).await?;
        Self::process_messages(pool, connection, network_tx).await;
        Ok(())
    }

    fn parse_connect_msg(
        raw: Vec<u8>,
        key: &x25519::PublicKey,
    ) -> anyhow::Result<Verified<Connect>> {
        let message = Message::from_raw_buffer(raw)?;
        let connect: Verified<Connect> = match message {
            Message::Service(Service::Connect(connect)) => connect,
            other => bail!(
                "First message from a remote peer is not `Connect`, got={:?}",
                other
            ),
        };
        let author = into_x25519_public_key(connect.author());

        ensure!(
            author == *key,
            "Connect message public key doesn't match with the received peer key"
        );
        Ok(connect)
    }

    pub async fn handle_requests(self, mut receiver: mpsc::Receiver<NetworkRequest>) {
        while let Some(request) = receiver.next().await {
            match request {
                NetworkRequest::SendMessage(key, message) => {
                    let mut this = self.clone();
                    tokio::spawn(async move {
                        this.handle_send_message(key, message).await.log_error();
                    });
                }

                NetworkRequest::DisconnectWithPeer(peer) => {
                    let disconnected = self.pool.write().remove(&peer, None);
                    if disconnected {
                        let mut network_tx = self.network_tx.clone();
                        tokio::spawn(async move {
                            network_tx
                                .send(NetworkEvent::PeerDisconnected(peer))
                                .await
                                .ok();
                        });
                    }
                }
            }
        }
    }

    async fn handle_send_message(
        &mut self,
        address: PublicKey,
        message: SignedMessage,
    ) -> anyhow::Result<()> {
        if self.pool.read().contains(&address) {
            self.pool.send_message(&address, message).await;
            Ok(())
        } else if self.can_create_connections() {
            self.create_new_connection(address, message).await
        } else {
            self.send_unable_connect_event(address).await
        }
    }

    async fn create_new_connection(
        &self,
        key: PublicKey,
        message: SignedMessage,
    ) -> anyhow::Result<()> {
        self.connect(key, &self.handshake_params).await?;
        let connect = &self.handshake_params.connect;
        if message != *connect.as_raw() {
            self.pool.send_message(&key, message).await;
        }
        Ok(())
    }

    async fn send_peer_connected_event(
        address: ConnectedPeerAddr,
        message: Verified<Connect>,
        network_tx: &mut mpsc::Sender<NetworkEvent>,
    ) -> anyhow::Result<()> {
        let peer_connected = NetworkEvent::PeerConnected(address, message);
        network_tx
            .send(peer_connected)
            .await
            .map_err(|_| format_err!("Cannot send `PeerConnected` notification"))
    }

    fn can_create_connections(&self) -> bool {
        self.pool.read().count_outgoing() < self.network_config.max_outgoing_connections
    }

    async fn send_unable_connect_event(&mut self, peer: PublicKey) -> anyhow::Result<()> {
        let event = NetworkEvent::UnableConnectToPeer(peer);
        self.network_tx
            .send(event)
            .await
            .map_err(|_| format_err!("can't send network event"))
    }
}

impl NetworkPart {
    pub async fn run(self, handshake_params: HandshakeParams) {
        let our_key = handshake_params.connect.author();

        let handler = NetworkHandler::new(
            self.listen_address,
            SharedConnectionPool::new(our_key),
            self.network_config,
            self.network_tx,
            handshake_params,
            self.connect_list,
        );

        let listener = handler.clone().listener().unwrap_or_else(|e| {
            log::error!("Listening to incoming peer connections failed: {}", e);
        });
        futures::pin_mut!(listener);
        let request_handler = handler.handle_requests(self.network_requests);
        futures::pin_mut!(request_handler);

        // FIXME: is `select` appropriate here?
        future::select(listener, request_handler).await;
    }
}
