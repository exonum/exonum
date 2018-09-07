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



use events::NetworkPart;
use tokio_core::reactor::Handle;
use events::noise::HandshakeParams;
use futures::Future;
use futures::unsync;
use failure;
use events::network::Listener;
use events::network::RequestHandler;
use std::sync::Arc;
use std::sync::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use futures::sync::mpsc;
use tokio::net::TcpListener;
use events::error::log_error;
use tokio::net::TcpStream;
use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;
use tokio_codec::LinesCodec;
use tokio;
use tokio_retry::strategy::jitter;
use futures::{future, Stream};
use futures::Sink;
use tokio_io::AsyncRead;
use events::error::into_failure;
use futures::IntoFuture;
use std::thread;


#[derive(Clone, Debug)]
pub struct ConnectionPool2 {
    pub peers: Arc<RwLock<HashMap<SocketAddr, mpsc::Sender<String>>>>,
}

impl ConnectionPool2 {
    pub fn new() -> Self {
        ConnectionPool2 {
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn add_peer(&self, address: &SocketAddr, sender: mpsc::Sender<String>) {
        let mut peers = self.peers.write().expect("ConnectionPool write lock");
        peers.insert(*address, sender);
    }
}

#[derive(Clone)]
pub struct Node {
    pub address: SocketAddr,
    pool: ConnectionPool2,
}

impl Node {
    pub fn new(address: SocketAddr, connection_pool: ConnectionPool2) -> Self {
        Node {
            address,
            pool: connection_pool,
        }
    }

    pub fn listen(
        &self,
        network_tx: mpsc::Sender<String>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let server = TcpListener::bind(&self.address).unwrap().incoming();
        let pool = self.pool.clone();
        let mut connection_counter = 0;

        let address = self.address.clone();

        let fut = server
            .map_err(into_failure)
            .for_each(move |incoming_connection| {
                println!("connected from {:?}", incoming_connection);

                connection_counter += 1;
                Self::process_connection(
                    &address,
                    incoming_connection,
                    pool.clone(),
                    network_tx.clone(),
                    true,
                )
            });

        fut
    }

    fn send_message(pool: ConnectionPool2, message: String, address: &SocketAddr) -> impl Future<Item = (), Error = failure::Error> {
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
        connection: TcpStream,
        pool: ConnectionPool2,
        network_tx: mpsc::Sender<String>,
        incoming: bool,
    ) -> Result<(), failure::Error> {
        let (sender_tx, receiver_rx) = mpsc::channel::<String>(1024);

        let peer_addr = connection.local_addr().unwrap();
        let (sink, stream) = connection.framed(LinesCodec::new()).split();

        let sender = sink.send(address.to_string())
            .map_err(log_error)
            .and_then(|sink| {
                receiver_rx
                    .filter(|line| !line.is_empty())
                    .map_err(|e| format_err!("error! "))
                    .forward(sink)
                    .map(drop)
                    .map_err(|e| println!("error!"))
            });

        let fut = stream
            .into_future()
            .map_err(|e| log_error(e.0))
            .and_then(move |(line, stream)| {
                let remote_address: SocketAddr = line.unwrap().parse().unwrap();
                println!("connected from {}, incoming {}", remote_address, incoming);

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
        receiver: mpsc::Receiver<String>,
        network_tx: mpsc::Sender<String>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let address = self.address.clone();
        let pool = self.pool.clone();

        let handler = receiver.for_each(move |line| {
            let fut = match line.as_str() {
                "connect" => future::Either::A(Self::connect(
                    pool.clone(),
                    &address,
                    &"127.0.0.1:9000".parse().unwrap(),
                    network_tx.clone(),
                )),
                _ => future::Either::B(Self::send_message(pool.clone(), line, &"127.0.0.1:9000".parse().unwrap())),
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
        self_address: &SocketAddr,
        address: &SocketAddr,
        network_tx: mpsc::Sender<String>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let address = address.clone();
        let self_address = self_address.clone();
        let timeout = 1000;
        let max_tries = 5000;
        let strategy = FixedInterval::from_millis(timeout)
            .map(jitter)
            .take(max_tries);

        let action = move || TcpStream::connect(&address);
        let pool = pool.clone();

        let future = Retry::spawn(strategy, action).map_err(into_failure).and_then(
            move |outgoing_connection| {
                Self::process_connection(
                    &self_address,
                    outgoing_connection,
                    pool,
                    network_tx,
                    false,
                )
            },
        );

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
        let (sender_tx, receiver_rx) = mpsc::channel::<String>(1024);

        let node = Node::new(listen_address, pool.clone());
        let connector = node.clone();

        let remote_sender = sender_tx.clone();

        let listener = node.clone();

        let server = listener.listen(sender_tx.clone());
        let handler = node.request_handler(connect_receiver_rx, sender_tx);
        thread::spawn(|| tokio::run(server.join(handler).map_err(log_error).map(drop)));

        thread::spawn(move || {
            let receiver = receiver_rx.for_each(|line| {
                println!("> {}", line);
                Ok(())
            });
            tokio::run(receiver);
        });

        future::ok::<(), failure::Error>(())
    }
}
