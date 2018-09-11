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
use events::network::Listener;
use events::network::RequestHandler;
use events::noise::Handshake;
use events::noise::HandshakeParams;
use events::noise::NoiseHandshake;
use events::noise::NoiseWrapper;
use events::NetworkPart;
use events::NetworkRequest;
use failure;
use futures::sync::mpsc;
use futures::unsync;
use futures::Future;
use futures::IntoFuture;
use futures::Sink;
use futures::{future, Stream};
use messages::RawMessage;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;
use tokio;
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
        let mut peers = self.peers.write().expect("ConnectionPool write lock");
        peers.insert(*address, sender);
    }
}

#[derive(Clone)]
pub struct Node {
    pub listen_address: SocketAddr,
    pool: ConnectionPool2,
}

impl Node {
    pub fn new(address: SocketAddr, connection_pool: ConnectionPool2) -> Self {
        Node {
            listen_address: address,
            pool: connection_pool,
        }
    }

    pub fn listen(
        &self,
        network_tx: mpsc::Sender<RawMessage>,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let server = TcpListener::bind(&self.listen_address).unwrap().incoming();
        let pool = self.pool.clone();
        let mut connection_counter = 0;

        let handshake_params = handshake_params.clone();
        let address = self.listen_address.clone();

        let fut = server
            .map_err(into_failure)
            .for_each(move |incoming_connection| {
                println!("connected from {:?}", incoming_connection);

                connection_counter += 1;

                let handshake = NoiseHandshake::responder(&handshake_params, &address);

                handshake.listen(incoming_connection).and_then(|socket| {
                    Self::process_connection(
                        &address,
                        socket,
                        pool.clone(),
                        network_tx.clone(),
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
        let mut read_pool = pool.clone();
        let sender_tx = read_pool.peers.read().unwrap();
        let sender = sender_tx.get(&address).unwrap();

        sender
            .clone()
            .send(message.clone())
            .map_err(into_failure)
            .map(drop)
    }

    fn process_connection(
        address: &SocketAddr,
        connection: Framed<TcpStream, MessagesCodec>,
        pool: ConnectionPool2,
        network_tx: mpsc::Sender<RawMessage>,
        incoming: bool,
    ) -> Result<(), failure::Error> {
        let (sender_tx, receiver_rx) = mpsc::channel::<RawMessage>(1024);

        let (sink, stream) = connection.split();

        let sender = receiver_rx
            .map_err(|e| format_err!("error! "))
            .forward(sink)
            .map(drop)
            .map_err(|e| println!("error!"));

        let fut = stream
            .into_future()
            .map_err(|e| log_error(e.0))
            .and_then(move |(line, stream)| {
                // TODO: get remote address from connect message
                let remote_address: SocketAddr = "127.0.0.1:8000".parse().unwrap();

                pool.add_peer(&remote_address, sender_tx);

                network_tx
                    .sink_map_err(into_failure)
                    .send_all(stream)
                    .map_err(log_error)
                    .into_future()
                    .map(drop)
            })
            .map(drop);

        tokio::spawn(fut);
        tokio::spawn(sender);
        Ok(())
    }

    pub fn request_handler(
        &self,
        handshake_params: &HandshakeParams,
        receiver: mpsc::Receiver<NetworkRequest>,
        network_tx: mpsc::Sender<RawMessage>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let listen_address = self.listen_address.clone();
        let pool = self.pool.clone();
        let handshake_params = handshake_params.clone();

        let handler = receiver.for_each(move |request| {
            let fut = match request {
                NetworkRequest::ConnectToPeer(address) => future::Either::A(Self::connect(
                    pool.clone(),
                    &listen_address,
                    &address,
                    network_tx.clone(),
                    &handshake_params,
                )),
                NetworkRequest::SendMessage(address, message) => future::Either::B(
                    Self::send_message(pool.clone(), message, &"127.0.0.1:9000".parse().unwrap()),
                ),
                _ => unimplemented!(),
            }.map_err(log_error);

            tokio::spawn(fut);
            Ok(())
        });

        handler.map_err(|e| format_err!(""))
    }

    pub fn ok() -> impl Future<Item = (), Error = failure::Error> {
        future::ok::<(), failure::Error>(())
    }

    pub fn connect(
        pool: ConnectionPool2,
        listen_addres: &SocketAddr,
        address: &SocketAddr,
        network_tx: mpsc::Sender<RawMessage>,
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

        let future = Retry::spawn(strategy, action)
            .map_err(into_failure)
            .and_then(move |outgoing_connection| {
                let handshake = NoiseHandshake::initiator(&handshake_params, &address);
                handshake.send(outgoing_connection)
            })
            .and_then(|socket| {
                Self::process_connection(&listen_address, socket, pool, network_tx, false)
            })
            .map(drop);

        future
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

        let (connect_sender_tx, connect_receiver_rx) = mpsc::channel::<String>(1024);
        let (sender_tx, receiver_rx) = mpsc::channel::<RawMessage>(1024);

        let node = Node::new(listen_address, pool.clone());
        let connector = node.clone();

        let remote_sender = sender_tx.clone();

        let listener = node.clone();

        let server = listener.listen(sender_tx.clone(), &handshake_params);
        let handler = node.request_handler(&handshake_params, self.network_requests.1, sender_tx);
        //        thread::spawn(|| tokio::run(server.join(handler).map_err(log_error).map(drop)));

        server.join(handler).map(drop)
    }
}
