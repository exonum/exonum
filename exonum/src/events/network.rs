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
    future, future::{err, Either}, stream::SplitStream, sync::mpsc, unsync, Future, IntoFuture,
    Sink, Stream,
};
use tokio_codec::Framed;
use tokio_core::{
    net::{TcpListener, TcpStream}, reactor::Handle,
};
use tokio_retry::{
    strategy::{jitter, FixedInterval}, Retry,
};

use std::{cell::RefCell, collections::HashMap, net::SocketAddr, rc::Rc, time::Duration};

use super::{
    error::{log_error, result_ok}, to_box,
};
use events::{
    codec::MessagesCodec, error::into_failure, noise::{Handshake, HandshakeParams, NoiseHandshake},
};
use helpers::Milliseconds;
use messages::{Connect, Message, Protocol, Service, SignedMessage};

const OUTGOING_CHANNEL_SIZE: usize = 10;

#[derive(Debug)]
pub enum NetworkEvent {
    MessageReceived(SocketAddr, Vec<u8>),
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
    fn new() -> Self {
        Self::default()
    }

    fn insert(&self, peer: SocketAddr, sender: &mpsc::Sender<SignedMessage>) {
        self.inner.borrow_mut().insert(peer, sender.clone());
    }

    fn remove(&self, peer: &SocketAddr) -> Result<mpsc::Sender<SignedMessage>, failure::Error> {
        self.inner
            .borrow_mut()
            .remove(peer)
            .ok_or_else(|| format_err!("there is no sender in the connection pool"))
    }

    fn get(&self, peer: SocketAddr) -> Option<mpsc::Sender<SignedMessage>> {
        self.inner.borrow_mut().get(&peer).cloned()
    }

    fn len(&self) -> usize {
        self.inner.borrow_mut().len()
    }

    fn create_connection(
        self,
        network_config: NetworkConfiguration,
        peer: SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
        handle: &Handle,
        handshake_params: &HandshakeParams,
    ) -> mpsc::Sender<SignedMessage> {
        // Register outgoing channel.
        let (conn_tx, conn_rx) = mpsc::channel(OUTGOING_CHANNEL_SIZE);
        self.insert(peer, &conn_tx);
        // Enable retry feature for outgoing connection.
        let timeout = network_config.tcp_connect_retry_timeout;
        let max_tries = network_config.tcp_connect_max_retries as usize;
        let strategy = FixedInterval::from_millis(timeout)
            .map(jitter)
            .take(max_tries);
        let handle_cloned = handle.clone();
        let handshake_params = handshake_params.clone();

        let action = move || TcpStream::connect(&peer, &handle_cloned);
        let connect_handle = Retry::spawn(strategy, action)
            .map_err(into_failure)
            .and_then(move |socket| Self::configure_socket(socket, network_config))
            .and_then(move |socket| {
                Self::build_handshake_initiator(socket, &peer, &handshake_params)
            })
            .and_then(move |stream| Self::process_outgoing_messages(peer, stream, conn_rx))
            .then(move |_| self.disconnect_with_peer(peer, network_tx.clone()))
            .map_err(log_error);
        handle.spawn(connect_handle);
        conn_tx
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

    fn disconnect_with_peer(
        &self,
        peer: SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> Box<dyn Future<Item = (), Error = failure::Error>> {
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

    fn build_handshake_initiator(
        stream: TcpStream,
        peer: &SocketAddr,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = Framed<TcpStream, MessagesCodec>, Error = failure::Error> {
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

    // Connect socket with the outgoing channel
    fn process_outgoing_messages(
        peer: SocketAddr,
        stream: Framed<TcpStream, MessagesCodec>,
        conn_rx: mpsc::Receiver<SignedMessage>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        trace!("Established connection with peer={}", peer);
        let (sink, stream) = stream.split();

        let writer = conn_rx
            .map_err(|_| format_err!("Can't send data into socket"))
            .forward(sink);
        let reader = stream.for_each(result_ok);

        reader
            .select2(writer)
            .map_err(|_| format_err!("Socket error"))
            .and_then(move |reason| {
                Self::log_disconnect_reason(peer, &reason);
                Ok(())
            })
    }

    fn log_disconnect_reason<A, B>(peer: SocketAddr, reason: &Either<A, B>) {
        let reason = match reason {
            Either::A(_) => "by reader",
            Either::B(_) => "by writer",
        };

        trace!("Disconnection with peer={}, reason={:?}", peer, reason);
    }
}

impl NetworkPart {
    pub fn run(
        self,
        handle: &Handle,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let network_config = self.network_config;
        // `cancel_sender` is converted to future when we receive
        // `NetworkRequest::Shutdown` causing its being completed with error.
        // After that completes `cancel_handler` and event loop stopped.
        let (cancel_sender, cancel_handler) = unsync::oneshot::channel();

        let request_handler = RequestHandler::from(
            self.our_connect_message,
            network_config,
            self.network_tx.clone(),
            handle.clone(),
            handshake_params.clone(),
        ).into_handler(self.network_requests.1, cancel_sender);

        let server = Listener::new(network_config, &handle, &self.network_tx, &handshake_params)
            .bind(self.listen_address);

        let cancel_handler = cancel_handler.or_else(|e| {
            trace!("Requests handler closed: {}", e);
            Ok(())
        });
        server
            .join(request_handler)
            .map(drop)
            .select(cancel_handler)
            .map_err(|(e, _)| e)
            .map(drop)
    }
}

struct RequestHandler {
    connect_message: Message<Connect>,
    network_config: NetworkConfiguration,
    network_tx: mpsc::Sender<NetworkEvent>,
    handle: Handle,
    handshake_params: HandshakeParams,
    outgoing_connections: ConnectionsPool,
}

impl RequestHandler {
    fn from(
        connect_message: Message<Connect>,
        network_config: NetworkConfiguration,
        network_tx: mpsc::Sender<NetworkEvent>,
        handle: Handle,
        handshake_params: HandshakeParams,
    ) -> Self {
        RequestHandler {
            connect_message,
            network_config,
            network_tx,
            handle,
            handshake_params,
            outgoing_connections: ConnectionsPool::new(),
        }
    }

    fn into_handler(
        self,
        receiver: mpsc::Receiver<NetworkRequest>,
        cancel_sender: unsync::oneshot::Sender<()>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let mut cancel_sender = Some(cancel_sender);
        receiver
            .map_err(|_| format_err!("no network requests"))
            .for_each(move |request| {
                match request {
                    NetworkRequest::SendMessage(peer, message) => {
                        to_box(self.handle_send_message(peer, message))
                    }
                    NetworkRequest::DisconnectWithPeer(peer) => self.outgoing_connections
                        .disconnect_with_peer(peer, self.network_tx.clone()),
                    // Immediately stop the event loop.
                    NetworkRequest::Shutdown => to_box(
                        cancel_sender
                            .take()
                            .ok_or_else(|| format_err!("shutdown twice"))
                            .into_future(),
                    ),
                }
            })
    }

    fn handle_send_message(
        &self,
        peer: SocketAddr,
        message: SignedMessage,
    ) -> impl Future<Item = (), Error = failure::Error> + 'static {
        let connection = if let Some(connection) = self.outgoing_connections.get(peer) {
            // We have a connection with the peer already
            Either::A(future::ok(connection))
        } else if self.can_create_connections() {
            // Create a new connection with the peer
            let connection = self.connect_to_peer(peer);
            let connection_future = self.send_connect_message(connection, &message);
            Either::B(connection_future)
        } else {
            warn!(
                "Rejected outgoing connection with peer={}, \
                 connections limit reached.",
                peer
            );

            return Either::B(self.send_unable_connect_event(peer));
        };

        Either::A(Self::send_message(connection, message))
    }

    fn can_create_connections(&self) -> bool {
        self.outgoing_connections.len() <= self.network_config.max_outgoing_connections
    }

    fn send_connect_message(
        &self,
        connection: mpsc::Sender<SignedMessage>,
        message: &SignedMessage,
    ) -> impl Future<Item = mpsc::Sender<SignedMessage>, Error = failure::Error> {
        if message == self.connect_message.signed_message() {
            Either::A(to_future(Ok(connection)))
        } else {
            Either::B(to_future(
                connection
                    .send(self.connect_message.clone().into())
                    .map_err(|_| format_err!("can't send message to a connection")),
            ))
        }
    }

    fn connect_to_peer(&self, peer: SocketAddr) -> mpsc::Sender<SignedMessage> {
        self.outgoing_connections.clone().create_connection(
            self.network_config,
            peer,
            self.network_tx.clone(),
            &self.handle,
            &self.handshake_params,
        )
    }

    fn send_message<S>(
        connection: S,
        message: SignedMessage,
    ) -> impl Future<Item = (), Error = failure::Error>
    where
        S: Future<Item = mpsc::Sender<SignedMessage>, Error = failure::Error>,
    {
        connection
            .and_then(|sender| {
                sender
                    .send(message)
                    .map_err(|_| format_err!("can't send message to a connection"))
            })
            .map(drop)
    }

    fn send_unable_connect_event(
        &self,
        peer: SocketAddr,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let event = NetworkEvent::UnableConnectToPeer(peer);
        self.network_tx
            .clone()
            .send(event)
            .map(drop)
            .map_err(|_| format_err!("can't send network event"))
    }
}

struct Listener<'a> {
    network_config: NetworkConfiguration,
    handle: &'a Handle,
    network_tx: &'a mpsc::Sender<NetworkEvent>,
    handshake_params: &'a HandshakeParams,
}

impl<'a> Listener<'a> {
    fn new(
        network_config: NetworkConfiguration,
        handle: &'a Handle,
        network_tx: &'a mpsc::Sender<NetworkEvent>,
        handshake_params: &'a HandshakeParams,
    ) -> Self {
        Listener {
            network_config,
            handle,
            network_tx,
            handshake_params,
        }
    }

    fn bind(&self, listen_address: SocketAddr) -> impl Future<Item = (), Error = failure::Error> {
        // Incoming connections handler
        let listener = TcpListener::bind(&listen_address, &self.handle);

        match listener {
            Ok(listener) => Either::A(self.handle_incoming_connections(listener)),
            Err(e) => Either::B(future::err(into_failure(e))),
        }
    }

    fn handle_incoming_connections(
        &self,
        listener: TcpListener,
    ) -> impl Future<Item = (), Error = failure::Error> {
        // Incoming connections limiter
        let incoming_connections_limit = self.network_config.max_incoming_connections;
        // The reference counter is used to automatically count the number of the open connections.
        let incoming_connections_counter: Rc<()> = Rc::default();
        let network_tx = self.network_tx.clone();
        let handle = self.handle.clone();
        let handshake_params = self.handshake_params.clone();
        listener
            .incoming()
            .for_each(move |(sock, address)| {
                let holder = incoming_connections_counter.clone();
                // Check incoming connections count
                let connections_count = Rc::strong_count(&incoming_connections_counter) - 1;
                if connections_count > incoming_connections_limit {
                    warn!(
                        "Rejected incoming connection with peer={}, \
                         connections limit reached.",
                        address
                    );
                    return Ok(());
                }
                trace!("Accepted incoming connection with peer={}", address);
                let network_tx = network_tx.clone();

                let handshake = NoiseHandshake::responder(&handshake_params, &address);
                let connection_handler = handshake
                    .listen(sock)
                    .and_then(move |sock| Self::handle_single_connection(sock, address, network_tx))
                    .map(|_| {
                        drop(holder);
                    })
                    .map_err(|e| {
                        error!("Connection terminated: {}: {}", e, e.find_root_cause());
                    });

                handle.spawn(connection_handler);
                Ok(())
            })
            .map_err(into_failure)
    }

    fn handle_single_connection(
        sock: Framed<TcpStream, MessagesCodec>,
        address: SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let (_, stream) = sock.split();
        stream
            .into_future()
            .map_err(|e| e.0)
            .and_then(|(raw, stream)| (Self::parse_connect_message(raw), Ok(stream)))
            .and_then(move |(connect, stream)| {
                trace!("Received handshake message={:?}", connect);
                Self::process_incoming_messages(stream, network_tx, connect, address)
            })
    }

    fn parse_connect_message(raw: Option<Vec<u8>>) -> Result<Message<Connect>, failure::Error> {
        let raw = raw.ok_or_else(|| format_err!("Incoming socket closed"))?;
        let message = Protocol::from_raw_buffer(raw)?;
        match message {
            Protocol::Service(Service::Connect(connect)) => Ok(connect),
            other => bail!(
                "First message from a remote peer is not Connect, got={:?}",
                other
            ),
        }
    }

    fn process_incoming_messages<S>(
        stream: SplitStream<S>,
        network_tx: mpsc::Sender<NetworkEvent>,
        connect: Message<Connect>,
        address: SocketAddr,
    ) -> impl Future<Item = (), Error = failure::Error>
    where
        S: Stream<Item = Vec<u8>, Error = failure::Error>,
    {
        let event = NetworkEvent::PeerConnected(address, connect);
        let stream = stream.map(move |raw| NetworkEvent::MessageReceived(address, raw));

        network_tx
            .send(event)
            .map_err(into_failure)
            .and_then(|sender| sender.sink_map_err(into_failure).send_all(stream))
            .map(|_| ())
    }
}

fn to_future<F, I>(fut: F) -> impl Future<Item = I, Error = failure::Error>
where
    F: IntoFuture<Item = I, Error = failure::Error>,
{
    fut.into_future()
}
