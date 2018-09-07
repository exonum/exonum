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
    address: SocketAddr,
    pool: ConnectionPool2,
}

impl Node {
    pub fn new(address: SocketAddr, connection_pool: ConnectionPool2) -> Self {
        Node {
            address,
            pool: connection_pool,
        }
    }

    pub fn listen(&self, network_tx: mpsc::Sender<String>) -> impl Future<Item = (), Error=failure::Error> {
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

    pub fn process_pool(&self, receiver_rx: mpsc::Receiver<String>) -> impl Future<Item = (), Error=failure::Error>  {
        let mut read_pool = self.pool.clone();
            let sender = receiver_rx.for_each(move |message| {
                let sender_tx: Vec<mpsc::Sender<String>> =
                    read_pool.peers.read().unwrap().values().cloned().collect();

                println!("pool count {}", sender_tx.len());

                sender_tx.iter().for_each(move |sen| {
                    let fut = sen.clone()
                        .send(message.clone())
                        .map(drop)
                        .map_err(log_error);
                    tokio::spawn(fut);
                });

                Ok(())
            }).map_err(|e| format_err!("pool processing failed {:?}", e));

        sender
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
                    .map_err(log_error)
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

    pub fn connect(&self, address: &SocketAddr, network_tx: mpsc::Sender<String>) ->
        impl Future<Item=(), Error= failure::Error> {
        let address = address.clone();
        let self_address = self.address.clone();
        let timeout = 1000;
        let max_tries = 5000;
        let strategy = FixedInterval::from_millis(timeout)
            .map(jitter)
            .take(max_tries);

        let action = move || TcpStream::connect(&address);
        let pool = self.pool.clone();

        let future = Retry::spawn(strategy, action)
            .map_err(into_failure)
            .and_then(move |outgoing_connection| {
                Self::process_connection(&self_address, outgoing_connection, pool, network_tx, false)
            });

        future
    }

    pub fn request_handler(&self, receiver: mpsc::Receiver<String>) -> impl Future<Item=(), Error=failure::Error> {

        let handler = receiver.for_each(|line| {
            match line.as_str() {
                "connect" => {

                }
                _ => {}
            }
        });


        future::ok::<(), failure::Error>(())
    }
}


impl NetworkPart {
    pub fn run2(
        self,
        handle: &Handle,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let network_config = self.network_config;
        // Cancellation token
        let (cancel_sender, cancel_handler) = unsync::oneshot::channel();

        let pool = ConnectionPool2::new();

        let listen_address = self.listen_address.clone();
        let (connect_sender_tx, connect_receiver_rx) = mpsc::channel::<String>(1024);
        let (sender_tx, receiver_rx) = mpsc::channel::<String>(1024);

        let node = Node::new(listen_address, pool);
        let connector = node.clone();

        let remote_sender = sender_tx.clone();

        let connect = connector.connect(&remote_address, remote_sender);

        let listener = node.clone();

        let server =
            listener.listen(sender_tx);

        let pool_processor = node.process_pool(connect_receiver_rx);

        thread::spawn(|| {
            tokio::run(server.join3(connect, pool_processor).map_err(log_error).map(drop))
        });

        future::ok::<(), failure::Error>(())
    }
}
