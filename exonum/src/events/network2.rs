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

use events::codec::MessagesCodec;
use events::error::into_failure;
use events::error::log_error;
use events::network::NetworkEvent;
use events::noise::Handshake;
use events::noise::HandshakeParams;
use events::noise::NoiseHandshake;
use events::noise::NoiseWrapper;
use events::to_box;
use events::NetworkPart;
use events::NetworkRequest;
use failure;
use futures::future::err;
use futures::future::Either;
use futures::sync::mpsc;
use futures::unsync;
use futures::Future;
use futures::IntoFuture;
use futures::Sink;
use futures::{future, Stream};
use messages::Any;
use messages::Connect;
use messages::RawMessage;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_codec::Framed;
use tokio_codec::LinesCodec;
use tokio_core::reactor::Handle;
use tokio_io::AsyncRead;
use tokio_retry::strategy::jitter;
use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;

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

    pub fn add_peer(&self, address: &SocketAddr, sender: mpsc::Sender<RawMessage>) {
        info!("add peer {:?}", address);
        let mut peers = self.peers.write().expect("ConnectionPool write lock");
        peers.insert(*address, sender);
    }

    pub fn contains(&self, address: &SocketAddr) -> bool {
        let mut peers = self.peers.read().expect("ConnectionPool read lock");

        peers.get(address).is_some()
    }
}

#[derive(Clone)]
pub struct Node {
    pub listen_address: SocketAddr,
    pool: ConnectionPool2,
    handle: Handle,
}

impl Node {
    pub fn new(handle: Handle, address: SocketAddr, connection_pool: ConnectionPool2) -> Self {
        Node {
            handle,
            listen_address: address,
            pool: connection_pool,
        }
    }

    pub fn listen(
        &self,
        network_tx: mpsc::Sender<NetworkEvent>,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let server = TcpListener::bind(&self.listen_address).unwrap().incoming();
        let pool = self.pool.clone();
        let mut connection_counter = 0;

        let handshake_params = handshake_params.clone();
        let listen_address = self.listen_address.clone();
        let network_tx = network_tx.clone();
        let handle = self.handle.clone();

        let fut = server
            .map_err(into_failure)
            .for_each(move |incoming_connection| {
                info!("connected from {:?}", incoming_connection);

                connection_counter += 1;

                let listen_address = listen_address.clone();
                let pool = pool.clone();
                let network_tx = network_tx.clone();
                let handle = handle.clone();

                let handshake = NoiseHandshake::responder(&handshake_params, &listen_address);

                handshake
                    .listen(incoming_connection)
                    .and_then(move |(socket, raw)| (Ok(socket), Self::parse_connect_msg(Some(raw))))
                    .and_then(move |(socket, message)| {
                        let remote_address = message.addr();
                        //TOOD: change to real peer
                        let peer_connected =
                            NetworkEvent::PeerConnected("127.0.0.1:8000".parse().unwrap(), message);
                        (
                            Ok(socket),
                            Ok(remote_address),
                            network_tx
                                .clone()
                                .send(peer_connected)
                                .map_err(into_failure),
                        )
                    })
                    .and_then(move |(socket, address, network_tx)| {
                        Self::process_connection(
                            &handle,
                            &address,
                            socket,
                            pool.clone(),
                            network_tx,
                            true,
                        )
                    })
            });

        fut
    }

    fn send_message(
        pool: ConnectionPool2,
        message: RawMessage,
        address: &SocketAddr,
    ) -> impl Future<Item = (), Error = failure::Error> {
        info!("sending message to {:?} {:?}", address, message);

        let mut read_pool = pool.clone();
        let sender_tx = read_pool.peers.read().expect("pool read lock");
        let sender = sender_tx.get(&address);

        if let Some(sender) = sender_tx.get(&address) {
            Either::A(
                sender
                    .clone()
                    .send(message.clone())
                    .map_err(into_failure)
                    .map(drop),
            )
        } else {
            Either::B(future::ok(()))
        }
    }

    fn process_connection(
        handle: &Handle,
        address: &SocketAddr,
        connection: Framed<TcpStream, MessagesCodec>,
        pool: ConnectionPool2,
        network_tx: mpsc::Sender<NetworkEvent>,
        incoming: bool,
    ) -> Result<(), failure::Error> {
        info!(
            "handhake has finished {:?}, incoming {}",
            connection, incoming
        );

        let (sender_tx, receiver_rx) = mpsc::channel::<RawMessage>(1024);

        let remote_address = if incoming {
            connection.get_ref().local_addr().unwrap()
        } else {
            connection.get_ref().peer_addr().unwrap()
        };

        let (sink, stream) = connection.split();
        pool.add_peer(&address, sender_tx.clone());

        let sender = receiver_rx
            .inspect(|message| {
                info!("sending message to sink {:?}", message);
            })
            .map_err(|e| format_err!("error! "))
            .forward(sink)
            .map(drop)
            .map_err(|e| println!("error!"));

        let address = address.clone();

        let fut = network_tx.sink_map_err(into_failure).send_all(
            stream.map(move |message| NetworkEvent::MessageReceived(address, message)),
        ).map_err(log_error)
            .map(drop);

        handle.spawn(fut);
        handle.spawn(sender);
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
        let listen_address = self.listen_address.clone();
        let pool = self.pool.clone();
        let handshake_params = handshake_params.clone();
        let handle = self.handle.clone();
        let pool2 = pool.clone();

        let mut cancel_sender = Some(cancel_handler);

        let handler = receiver.for_each(move |request| {
            let pool2 = pool2.clone();
            let handle = handle.clone();
            let handle2 = handle.clone();
            let fut = match request {
                NetworkRequest::SendMessage(address, message) => {
                    to_box(if pool.contains(&address) {
                        info!("connection exists");
                        let pool2 = pool2.clone();
                        to_box(Self::send_message(pool2, message, &address))
                    } else {
                        info!("creating new connection");
                        to_box(
                            Self::connect(
                                handle,
                                pool.clone(),
                                &listen_address,
                                &address,
                                network_tx.clone(),
                                &handshake_params,)
                        //TODO: send message if not connect
//                            ).and_then(move |_| {
//                                let pool2 = pool2.clone();
//                                Self::send_message(pool2, message, &address)
//                            }),
                        )
                    })
                }
                NetworkRequest::DisconnectWithPeer(peer) => {
                    //Remove peer from pool
                    Self::disconnect_with_peer(peer, network_tx.clone())
                }
                NetworkRequest::Shutdown => to_box(
                    cancel_sender
                        .take()
                        .ok_or_else(|| format_err!("shutdown twice"))
                        .into_future(),
                ),
            }.map_err(log_error);

            handle2.spawn(fut);
            Ok(())
        });

        handler.map_err(|e| format_err!("unknown error in request handler"))
    }

    fn disconnect_with_peer(
        peer: SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> Box<dyn Future<Item = (), Error = failure::Error>> {
        let fut = network_tx
            .send(NetworkEvent::PeerDisconnected(peer))
            .map_err(|_| format_err!("can't send disconnect"))
            .map(drop);
        to_box(fut)
    }

    pub fn ok() -> impl Future<Item = (), Error = failure::Error> {
        future::ok::<(), failure::Error>(())
    }

    pub fn connect(
        handle: Handle,
        pool: ConnectionPool2,
        listen_addres: &SocketAddr,
        address: &SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let address = address.clone();
        let listen_address = listen_addres.clone();
        let handshake_params = handshake_params.clone();
        let timeout = 1000;
        let max_tries = 5000;
        let strategy = FixedInterval::from_millis(timeout)
            .map(jitter)
            .take(max_tries);

        let action = move || TcpStream::connect(&address);
        let pool = pool.clone();
        let handle = handle.clone();

        let future = Retry::spawn(strategy, action)
            .map_err(into_failure)
            .and_then(move |outgoing_connection| {
                Self::build_handshake_initiator(outgoing_connection, &address, &handshake_params)
            })
            .and_then(move |(socket, raw)| (Ok(socket), Self::parse_connect_msg(Some(raw))))
            .and_then(move |(socket, message)| {
                let remote_address = message.addr();
                //TOOD: change to real peer
                let peer_connected =
                    NetworkEvent::PeerConnected("127.0.0.1:8000".parse().unwrap(), message);
                (
                    Ok(socket),
                    Ok(remote_address),
                    network_tx
                        .clone()
                        .send(peer_connected)
                        .map_err(into_failure),
                )
            })
            .and_then(move |(socket, address, network_tx)| {
                info!("proccessing outgoing connection");
                Self::process_connection(&handle, &address, socket, pool, network_tx, false)
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
        let network_config = self.network_config;
        let listen_address = self.listen_address;
        // Cancellation token
        let (cancel_sender, cancel_handler) = unsync::oneshot::channel::<()>();

        let pool = ConnectionPool2::new();

        let node = Node::new(handle.clone(), listen_address, pool.clone());
        let connector = node.clone();

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
