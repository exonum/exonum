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
    channel::{mpsc, oneshot},
    future,
    prelude::*,
    stream::{SplitSink, SplitStream},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};

use super::error::log_error;
use crate::{
    events::{
        codec::MessagesCodec,
        error::into_failure,
        noise::{Handshake, HandshakeData, HandshakeParams, NoiseHandshake},
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
    pub network_requests: mpsc::Receiver<NetworkRequest>,
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
    peers: Arc<RwLock<HashMap<PublicKey, ConnectionPoolEntry>>>,
}

impl ConnectionPool {
    fn new() -> Self {
        Self {
            peers: Arc::default(),
        }
    }

    fn count_outgoing(&self) -> usize {
        let peers = self.peers.read().unwrap();
        peers
            .iter()
            .filter(|(_, e)| !e.address.is_incoming())
            .count()
    }

    fn add(&self, key: PublicKey, address: ConnectedPeerAddr, sender: mpsc::Sender<SignedMessage>) {
        let mut peers = self.peers.write().unwrap();
        peers.insert(key, ConnectionPoolEntry { sender, address });
    }

    fn contains(&self, address: &PublicKey) -> bool {
        let peers = self.peers.read().unwrap();
        peers.get(address).is_some()
    }

    fn remove(&self, address: &PublicKey) -> Option<ConnectedPeerAddr> {
        let mut peers = self.peers.write().unwrap();
        peers.remove(address).map(|o| o.address)
    }

    fn add_incoming_address(
        &self,
        key: PublicKey,
        address: ConnectedPeerAddr,
    ) -> mpsc::Receiver<SignedMessage> {
        let (sender_tx, receiver_rx) = mpsc::channel(OUTGOING_CHANNEL_SIZE);
        self.add(key, address, sender_tx);
        receiver_rx
    }

    async fn send_message(&self, peer_key: &PublicKey, message: SignedMessage) {
        let maybe_sender = {
            // Ensure that we don't hold lock across the `await` point.
            let peers = self.peers.read().unwrap();
            peers.get(peer_key).map(|peer| peer.sender.clone())
        };

        if let Some(mut sender) = maybe_sender {
            if let Err(e) = sender.send(message).await {
                log_error(e);
                self.remove(peer_key);
            }
        }
    }

    async fn disconnect_with_peer(
        &self,
        key: PublicKey,
        network_tx: &mut mpsc::Sender<NetworkEvent>,
    ) {
        if self.remove(&key).is_some()
            && network_tx
                .send(NetworkEvent::PeerDisconnected(key))
                .await
                .is_err()
        {
            log::warn!("Cannot send disconnect for peer {}", key);
        }
    }
}

struct Connection {
    socket: Framed<TcpStream, MessagesCodec>,
    receiver_rx: mpsc::Receiver<SignedMessage>,
    address: ConnectedPeerAddr,
    key: PublicKey,
}

impl Connection {
    fn new(
        socket: Framed<TcpStream, MessagesCodec>,
        receiver_rx: mpsc::Receiver<SignedMessage>,
        address: ConnectedPeerAddr,
        key: PublicKey,
    ) -> Self {
        Self {
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
    network_config: NetworkConfiguration,
    network_tx: mpsc::Sender<NetworkEvent>,
    handshake_params: HandshakeParams,
    connect_list: SharedConnectList,
}

impl NetworkHandler {
    fn new(
        address: SocketAddr,
        connection_pool: ConnectionPool,
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

    async fn listener(self) -> Result<(), failure::Error> {
        let mut listener = TcpListener::bind(&self.listen_address).await?;
        let mut incoming_connections = listener.incoming();

        // Incoming connections limiter
        let incoming_connections_limit = self.network_config.max_incoming_connections;
        // The reference counter is used to automatically count the number of the open connections.
        let incoming_connections_counter: Arc<()> = Arc::default();

        while let Some(mut socket) = incoming_connections.try_next().await? {
            // Check incoming connections count.
            let holder = Arc::clone(&incoming_connections_counter);
            let connections_count = Arc::strong_count(&incoming_connections_counter) - 1;

            let peer_address = match socket.peer_addr() {
                Ok(address) => address,
                Err(err) => {
                    log::warn!("Peer address resolution failed: {}", err);
                    continue;
                }
            };

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
                if pool.contains(&peer_key) {
                    // We've already connected to the peer; ignore this connection.
                    return Ok(());
                }

                if connect_list.is_peer_allowed(&peer_key) {
                    let conn_addr = ConnectedPeerAddr::In(peer_address);
                    let receiver_rx = pool.add_incoming_address(peer_key, conn_addr.clone());
                    let socket = Framed::new(socket, codec);
                    let connection = Connection::new(socket, receiver_rx, conn_addr, peer_key);
                    Self::handle_connection(connection, connect, pool, network_tx).await?;
                } else {
                    bail!(
                        "Rejecting incoming connection with peer={} public_key={}, \
                         the peer is not in the connect list",
                        peer_address,
                        peer_key
                    );
                }

                drop(holder);
                Ok(())
            };

            tokio::spawn(task.unwrap_or_else(|err| log::warn!("{}", err)));
        }
        Ok(())
    }

    fn connect(
        &self,
        key: PublicKey,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Output = Result<(), failure::Error>> {
        // Resolve peer key to an address.
        let maybe_address = self.connect_list.find_address_by_key(&key);
        let unresolved_address = if let Some(address) = maybe_address {
            address
        } else {
            let err = format_err!("Trying to connect to peer not from ConnectList key={}", key);
            return future::err(err).left_future();
        };

        let max_connections = self.network_config.max_outgoing_connections;
        let (sender_tx, receiver_rx) = mpsc::channel(OUTGOING_CHANNEL_SIZE);
        let network_config = self.network_config;
        let mut handshake_params = handshake_params.clone();
        handshake_params.set_remote_key(key);
        let pool = self.pool.clone();
        let network_tx = self.network_tx.clone();

        // TODO: use retries.
        async move {
            let mut socket = TcpStream::connect(&unresolved_address).await?;
            let peer_address = match socket.peer_addr() {
                Ok(addr) => addr,
                Err(err) => {
                    let err = format_err!("Couldn't take peer addr from socket = {}", err);
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

            let connect = Self::parse_connect_msg(raw_message, &peer_key)?;
            let connection_limit_reached = pool.count_outgoing() >= max_connections;
            if pool.contains(&key) || connection_limit_reached {
                return Ok(());
            }

            let conn_addr = ConnectedPeerAddr::Out(unresolved_address, peer_address);
            pool.add(key, conn_addr.clone(), sender_tx);
            let socket = Framed::new(socket, codec);
            let connection = Connection::new(socket, receiver_rx, conn_addr, key);
            Self::handle_connection(connection, connect, pool, network_tx).await
        }
        .right_future()
    }

    fn process_messages(
        pool: ConnectionPool,
        connection: Connection,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) {
        let (sink, stream) = connection.socket.split();

        let incoming = Self::process_incoming_messages(stream, pool, connection.key, network_tx);
        let outgoing = Self::process_outgoing_messages(sink, connection.receiver_rx);
        tokio::spawn(incoming);
        tokio::spawn(outgoing);
    }

    async fn process_outgoing_messages<S>(
        sink: SplitSink<S, SignedMessage>,
        receiver_rx: mpsc::Receiver<SignedMessage>,
    ) where
        S: Sink<SignedMessage, Error = failure::Error>,
    {
        if let Err(err) = receiver_rx.map(Ok).forward(sink).await {
            log::error!("Connection terminated: {}: {}", err, err.find_root_cause());
        }
    }

    async fn process_incoming_messages<S>(
        stream: SplitStream<S>,
        pool: ConnectionPool,
        key: PublicKey,
        mut network_tx: mpsc::Sender<NetworkEvent>,
    ) where
        S: Stream<Item = Result<Vec<u8>, failure::Error>>,
    {
        let forward_outcome = (&mut network_tx)
            .sink_map_err(into_failure)
            .send_all(&mut stream.map_ok(NetworkEvent::MessageReceived))
            .await;
        if let Err(err) = forward_outcome {
            log::error!("Connection terminated: {}: {}", err, err.find_root_cause());
        }
        pool.disconnect_with_peer(key, &mut network_tx).await;
    }

    fn configure_socket(
        socket: &mut TcpStream,
        network_config: NetworkConfiguration,
    ) -> Result<(), failure::Error> {
        socket.set_nodelay(network_config.tcp_nodelay)?;
        let duration = network_config.tcp_keep_alive.map(Duration::from_millis);
        socket.set_keepalive(duration)?;
        Ok(())
    }

    async fn handle_connection(
        connection: Connection,
        connect: Verified<Connect>,
        pool: ConnectionPool,
        mut network_tx: mpsc::Sender<NetworkEvent>,
    ) -> Result<(), failure::Error> {
        let address = connection.address.clone();
        log::trace!("Established connection with peer {:?}", address);

        Self::send_peer_connected_event(address, connect, &mut network_tx).await?;
        Self::process_messages(pool, connection, network_tx);
        Ok(())
    }

    fn parse_connect_msg(
        raw: Vec<u8>,
        key: &x25519::PublicKey,
    ) -> Result<Verified<Connect>, failure::Error> {
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
            author == *key,
            "Connect message public key doesn't match with the received peer key"
        );
        Ok(connect)
    }

    pub async fn handle_requests(
        mut self,
        mut receiver: mpsc::Receiver<NetworkRequest>,
        cancel_handler: oneshot::Sender<()>,
    ) -> Result<(), failure::Error> {
        let mut cancel_sender = Some(cancel_handler);

        while let Some(request) = receiver.next().await {
            match request {
                NetworkRequest::SendMessage(key, message) => {
                    self.handle_send_message(key, message).await?;
                }
                NetworkRequest::DisconnectWithPeer(peer) => {
                    self.pool
                        .disconnect_with_peer(peer, &mut self.network_tx)
                        .await;
                }
                NetworkRequest::Shutdown => {
                    if cancel_sender.take().is_none() {
                        bail!("Shut down twice");
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_send_message(
        &mut self,
        address: PublicKey,
        message: SignedMessage,
    ) -> Result<(), failure::Error> {
        if self.pool.contains(&address) {
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
    ) -> Result<(), failure::Error> {
        let connect = self.handshake_params.connect.clone();
        self.connect(key, &self.handshake_params).await?;
        if message != *connect.as_raw() {
            self.pool.send_message(&key, message).await;
        }
        Ok(())
    }

    async fn send_peer_connected_event(
        address: ConnectedPeerAddr,
        message: Verified<Connect>,
        network_tx: &mut mpsc::Sender<NetworkEvent>,
    ) -> Result<(), failure::Error> {
        let peer_connected = NetworkEvent::PeerConnected(address, message);
        network_tx
            .send(peer_connected)
            .await
            .map_err(|_| format_err!("Cannot send `PeerConnected` notification"))
    }

    fn can_create_connections(&self) -> bool {
        self.pool.count_outgoing() < self.network_config.max_outgoing_connections
    }

    async fn send_unable_connect_event(&mut self, peer: PublicKey) -> Result<(), failure::Error> {
        let event = NetworkEvent::UnableConnectToPeer(peer);
        self.network_tx
            .send(event)
            .await
            .map_err(|_| format_err!("can't send network event"))
    }
}

impl NetworkPart {
    pub async fn run(self, handshake_params: HandshakeParams) {
        let (cancel_tx, cancel_rx) = oneshot::channel();

        let handler = NetworkHandler::new(
            self.listen_address,
            ConnectionPool::new(),
            self.network_config,
            self.network_tx,
            handshake_params,
            self.connect_list,
        );

        let listener = handler.clone().listener();
        let request_handler = handler.handle_requests(self.network_requests, cancel_tx);
        let handlers = future::join(listener, request_handler);
        futures::pin_mut!(handlers);

        let cancel_handler = cancel_rx.unwrap_or_else(|e| {
            log::trace!("Requests handler closed: {}", e);
        });
        future::select(handlers, cancel_handler).await;
    }
}
