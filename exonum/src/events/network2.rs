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
    future, future::{err, Either}, sync::mpsc, unsync, Future, IntoFuture, Sink, Stream,
};
use tokio_codec::Framed;
use tokio_core::reactor::Handle;

use tokio_retry::{
    strategy::{jitter, FixedInterval}, Retry,
};

use std::{
    collections::HashMap, net::SocketAddr, rc::Rc, sync::{Arc, RwLock},
};

use super::{error::log_error, to_box};
use events::{
    codec::MessagesCodec, error::into_failure, noise::{Handshake, HandshakeParams, NoiseHandshake},
    NetworkConfiguration, NetworkEvent, NetworkPart, NetworkRequest,
};
use messages::{Any, Connect, Message, RawMessage};
use tokio::net::{TcpListener, TcpStream};
use std::time::Duration;

const OUTGOING_CHANNEL_SIZE: usize = 10;

#[derive(Clone, Debug)]
pub struct ConnectionPool2 {
    pub peers: Arc<RwLock<HashMap<SocketAddr, mpsc::Sender<RawMessage>>>>,
}

impl ConnectionPool2 {
    pub fn new() -> Self {
        ConnectionPool2 {
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn len(&self) -> usize {
        self.peers.read().expect("ConnectionPool read lock").len()
    }

    pub fn add(&self, address: &SocketAddr, sender: mpsc::Sender<RawMessage>) {
        let mut peers = self.peers.write().expect("ConnectionPool write lock");
        peers.insert(*address, sender);
    }

    pub fn contains(&self, address: &SocketAddr) -> bool {
        let peers = self.peers.read().expect("ConnectionPool read lock");
        peers.get(address).is_some()
    }

    pub fn remove(&self, address: &SocketAddr) {
        let mut peers = self.peers.write().expect("ConnectionPool write lock");
        peers.remove(address);
    }
}

//TODO: implement connection
pub struct Connection {
    handle: Handle,
    address: SocketAddr,
    socket: Framed<TcpStream, MessagesCodec>,
    receiver_rx: mpsc::Receiver<RawMessage>,
}

impl Connection {
    fn new(
        handle: Handle,
        address: SocketAddr,
        socket: Framed<TcpStream, MessagesCodec>,
        receiver_rx: mpsc::Receiver<RawMessage>,
    ) -> Self {
        Connection {
            handle,
            address,
            socket,
            receiver_rx,
        }
    }
}

#[derive(Clone)]
pub struct NetworkHandler {
    pub listen_address: SocketAddr,
    pool: ConnectionPool2,
    handle: Handle,
    network_config: NetworkConfiguration,
    network_tx: mpsc::Sender<NetworkEvent>,
}

impl NetworkHandler {
    pub fn new(
        handle: Handle,
        address: SocketAddr,
        connection_pool: ConnectionPool2,
        network_config: NetworkConfiguration,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> Self {
        NetworkHandler {
            handle,
            listen_address: address,
            pool: connection_pool,
            network_config,
            network_tx,
        }
    }

    pub fn listen(
        &self,
        network_tx: mpsc::Sender<NetworkEvent>,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let server = TcpListener::bind(&self.listen_address).unwrap().incoming();
        let pool = self.pool.clone();

        let handshake_params = handshake_params.clone();
        let listen_address = self.listen_address.clone();
        let network_tx = network_tx.clone();
        let handle = self.handle.clone();

        // Incoming connections limiter
        let incoming_connections_limit = self.network_config.max_incoming_connections;
        // The reference counter is used to automatically count the number of the open connections.
        let incoming_connections_counter: Rc<()> = Rc::default();

        let fut = server
            .map_err(into_failure)
            .for_each(move |incoming_connection| {
                let listen_address = listen_address.clone();
                //TODO: change to real peer address.
                let address = listen_address.clone();
                let pool = pool.clone();
                let network_tx = network_tx.clone();
                let handle = handle.clone();

                let handshake = NoiseHandshake::responder(&handshake_params, &listen_address);
                let holder = incoming_connections_counter.clone();
                // Check incoming connections count
                let connections_count = Rc::strong_count(&incoming_connections_counter) - 1;
                if connections_count > incoming_connections_limit {
                    warn!(
                        "Rejected incoming connection with peer={}, \
                         connections limit reached.",
                        address
                    );
                    return Either::A(future::ok(()));
                }

                let listener = handshake
                    .listen(incoming_connection)
                    .and_then(move |(socket, raw)| (Ok(socket), Self::parse_connect_msg(Some(raw))))
                    .and_then(move |(socket, message)| {
                        let (sender_tx, receiver_rx) =
                            mpsc::channel::<RawMessage>(OUTGOING_CHANNEL_SIZE);
                        let remote_address = message.addr();
                        pool.add(&remote_address, sender_tx);

                        (
                            Ok(Connection::new(
                                handle.clone(),
                                remote_address,
                                socket,
                                receiver_rx,
                            )),
                            Ok(message),
                        )
                    })
                    .and_then(move |(connection, message)| {
                        Self::process_connection_init(connection, message, network_tx)
                    })
                    .map(|_| {
                        drop(holder);
                    });
                Either::B(listener)
            });

        fut
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

    fn process_connection_init(
        connection: Connection,
        message: Connect,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let handle = connection.handle.clone();
        Self::send_peer_connected_event(&connection.address, message, network_tx)
            .and_then(move |network_tx| Self::process_connection(&handle, connection, network_tx))
    }

    fn send_message(
        pool: ConnectionPool2,
        message: RawMessage,
        address: &SocketAddr,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let address = address.clone();
        let sender_tx = pool.peers.read().expect("ConnectionPool read lock");
        let write_pool = pool.clone();

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

    fn process_connection(
        handle: &Handle,
        connection: Connection,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> Result<(), failure::Error> {
        let address = connection.address.clone();
        let (sink, stream) = connection.socket.split();

        let incoming_connection = network_tx
            .sink_map_err(into_failure)
            .send_all(stream.map(move |message| NetworkEvent::MessageReceived(address, message)))
            .map_err(log_error)
            .map(drop);

        let outgoing_connection = connection
            .receiver_rx
            .map_err(|_| format_err!("Remote peer has disconnected."))
            .forward(sink)
            .map(drop)
            .map_err(log_error);

        handle.spawn(incoming_connection);
        handle.spawn(outgoing_connection);
        Ok(())
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
        &self,
        handshake_params: &HandshakeParams,
        receiver: mpsc::Receiver<NetworkRequest>,
        network_tx: mpsc::Sender<NetworkEvent>,
        cancel_handler: unsync::oneshot::Sender<()>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let pool = self.pool.clone();
        let handshake_params = handshake_params.clone();
        let handle = self.handle.clone();
        let network_config = self.network_config;
        let mut cancel_sender = Some(cancel_handler);

        let handler = receiver.for_each(move |request| {
            let pool = pool.clone();
            let handle = handle.clone();
            let fut = match request {
                NetworkRequest::SendMessage(address, message) => to_box(Self::handle_send_message(
                    &address,
                    &handle,
                    message,
                    pool,
                    &handshake_params,
                    network_tx.clone(),
                    network_config,
                )),
                NetworkRequest::DisconnectWithPeer(peer) => {
                    Self::disconnect_with_peer(peer, pool, network_tx.clone())
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

        handler.map_err(|_| format_err!("unknown error in request handler"))
    }

    fn handle_send_message(
        address: &SocketAddr,
        handle: &Handle,
        message: RawMessage,
        pool: ConnectionPool2,
        handshake_params: &HandshakeParams,
        network_tx: mpsc::Sender<NetworkEvent>,
        network_config: NetworkConfiguration,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let pool = pool.clone();
        let connect = handshake_params.connect.clone();
        let handle = handle.clone();
        let address = address.clone();

        //TODO: refactor
        if pool.contains(&address) {
            to_box(Self::send_message(pool, message, &address))
        } else if Self::can_create_connections(pool.clone(), network_config) {
            to_box(
                Self::connect(
                    handle,
                    pool.clone(),
                    &address,
                    network_tx.clone(),
                    &handshake_params,
                    network_config,
                ).and_then(move |_| {
                    if &message != connect.raw() {
                        to_box(Self::send_message(pool, message, &address))
                    } else {
                        to_box(future::ok(()))
                    }
                }),
            )
        } else {
            to_box(Self::send_unable_connect_event(
                network_tx.clone(),
                &address,
            ))
        }
    }

    fn send_peer_connected_event(
        address: &SocketAddr,
        message: Connect,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = mpsc::Sender<NetworkEvent>, Error = failure::Error> {
        let peer_connected = NetworkEvent::PeerConnected(*address, message);
        network_tx
            .clone()
            .send(peer_connected)
            .map_err(into_failure)
    }

    fn can_create_connections(pool: ConnectionPool2, network_config: NetworkConfiguration) -> bool {
        pool.len() <= network_config.max_outgoing_connections
    }

    fn disconnect_with_peer(
        peer: SocketAddr,
        pool: ConnectionPool2,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> Box<dyn Future<Item = (), Error = failure::Error>> {
        pool.remove(&peer);
        let fut = network_tx
            .send(NetworkEvent::PeerDisconnected(peer))
            .map_err(|_| format_err!("can't send disconnect"))
            .map(drop);
        to_box(fut)
    }

    fn send_unable_connect_event(
        network_tx: mpsc::Sender<NetworkEvent>,
        peer: &SocketAddr,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let event = NetworkEvent::UnableConnectToPeer(peer.clone());
        network_tx
            .clone()
            .send(event)
            .map(drop)
            .map_err(|_| format_err!("can't send network event"))
    }

    pub fn connect(
        handle: Handle,
        pool: ConnectionPool2,
        address: &SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
        handshake_params: &HandshakeParams,
        network_config: NetworkConfiguration,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let address = address.clone();
        let handshake_params = handshake_params.clone();
        let timeout = network_config.tcp_connect_retry_timeout;
        let max_tries = network_config.tcp_connect_max_retries as usize;
        let strategy = FixedInterval::from_millis(timeout)
            .map(jitter)
            .take(max_tries);

        let action = move || TcpStream::connect(&address);
        let pool = pool.clone();
        let handle = handle.clone();

        let (sender_tx, receiver_rx) = mpsc::channel::<RawMessage>(OUTGOING_CHANNEL_SIZE);
        pool.add(&address, sender_tx);

        let future = Retry::spawn(strategy, action)
            .map_err(into_failure)
            .and_then(move |socket| Self::configure_socket(socket, network_config))
            .and_then(move |outgoing_connection| {
                Self::build_handshake_initiator(outgoing_connection, &address, &handshake_params)
            })
            .and_then(move |(socket, raw)| (Ok(socket), Self::parse_connect_msg(Some(raw))))
            .and_then(move |(socket, message)| {
                let remote_address = message.addr();
                (
                    Ok(Connection::new(
                        handle.clone(),
                        remote_address,
                        socket,
                        receiver_rx,
                    )),
                    Ok(message),
                )
            })
            .and_then(move |(connection, message)| {
                Self::process_connection_init(connection, message, network_tx)
            })
            .map(drop);

        future
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
    pub fn run2(
        self,
        handle: &Handle,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let listen_address = self.listen_address;
        // Cancellation token
        let (cancel_sender, cancel_handler) = unsync::oneshot::channel::<()>();

        let pool = ConnectionPool2::new();

        let node = NetworkHandler::new(
            handle.clone(),
            listen_address,
            pool.clone(),
            self.network_config,
            self.network_tx.clone(),
        );

        let listener = node.clone();

        let server = listener.listen(self.network_tx.clone(), &handshake_params);
        let handler = node.request_handler(
            &handshake_params,
            self.network_requests.1,
            self.network_tx,
            cancel_sender,
        );

        let cancel_handler = cancel_handler.or_else(|e| {
            trace!("Requests handler closed: {}", e);
            Ok(())
        });

        server
            .join(handler)
            .map(drop)
            .select(cancel_handler)
            .map_err(|(e, _)| e)
            .map(drop)
    }
}
