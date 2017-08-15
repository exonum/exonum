use futures::{future, stream, Future, Stream, Sink, IntoFuture};
use futures::stream::MergedItem;
use futures::future::{Either, err};
use futures::sync::mpsc;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::{Core, Handle, Timeout};
use tokio_io::AsyncRead;

use std::io;
use std::net::SocketAddr;
use std::thread;
use std::time::{SystemTime, Duration};
use std::collections::hash_map::{HashMap, Entry};
use std::error::Error as StdError;

use crypto::{gen_keypair, PublicKey, SecretKey};
use messages::{RawMessage, Any, Connect, Message};
use events::{EventHandler, NetworkEvent};

use super::error::{other_error, result_ok, forget_result, into_other, log_error};
use super::codec::MessagesCodec;
use super::Node;
use super::handler::NetworkRequest;

pub fn run_node(node: Node) -> io::Result<()> {
    let (sender, receiver) = (node.channel.0, node.channel.1);
    let listen_addr = sender.listen_addr;
    // Channels
    let (events_tx, events_rx) = mpsc::channel(64);
    let (requests_tx, requests_rx) = (sender.network, receiver.network);
    let (timeouts_tx, timeouts_rx) = (sender.timeout, receiver.timeout);

    let events_tx = events_tx.clone();
    let requests_tx = requests_tx.clone();
    let network_thread = thread::spawn(move || {
        let mut core = Core::new().unwrap();

        // Outgoing connections handler
        let mut outgoing_connections: HashMap<SocketAddr, mpsc::Sender<RawMessage>> =
            HashMap::new();

        // Requests handler
        let handle = core.handle();
        let events_tx = events_tx.clone();
        let requests_handle = requests_rx.for_each(|request| {
            debug!("Tokio: Handle request {:?}", request);

            match request {
                NetworkRequest::SendMessage(peer, msg) => {
                    let conn_tx = match outgoing_connections.entry(peer) {
                        Entry::Occupied(entry) => entry.get().clone(),
                        Entry::Vacant(entry) => {
                            let (conn_tx, conn_rx) = mpsc::channel(10);
                            let socket = TcpStream::connect(&peer, &handle);

                            let requests_tx = requests_tx.clone();
                            let connect_handle = socket
                                .and_then(move |sock| {
                                    info!("Tokio: Established connection with peer={}", peer);

                                    let stream = sock.framed(MessagesCodec);
                                    let (sink, stream) = stream.split();

                                    let writer = conn_rx
                                        .map_err(|_| other_error("Can't send data into socket"))
                                        .forward(sink);
                                    let reader = stream.for_each(result_ok).map_err(into_other);

                                    reader
                                        .select2(writer)
                                        .map_err(|_| other_error("Socket error"))
                                        .and_then(|res| match res {
                                            Either::A((_, _reader)) => Ok(()).into_future(),
                                            Either::B((_, _writer)) => Ok(()).into_future(),
                                        })
                                })
                                .map_err(log_error)
                                .then(move |_| {
                                    info!("Tokio: Connection with peer={} closed", peer);
                                    let request = NetworkRequest::DisconnectWithPeer(peer);
                                    requests_tx
                                        .clone()
                                        .send(request)
                                        .map(forget_result)
                                        .map_err(into_other)
                                })
                                .map_err(log_error);
                            handle.spawn(connect_handle);

                            entry.insert(conn_tx.clone());
                            conn_tx
                        }
                    };

                    let duration = Duration::from_secs(5);
                    let send_timeout = Timeout::new(duration, &handle)
                        .unwrap()
                        .and_then(result_ok)
                        .map_err(|_| other_error("Can't timeout"));

                    let send_handle = conn_tx.send(msg).map(forget_result).map_err(log_error);

                    let requests_tx = requests_tx.clone();
                    let timeouted_connect = send_handle
                        .select2(send_timeout)
                        .map_err(|_| other_error("Unable to send message"))
                        .and_then(move |either| match either {
                            Either::A((send, _timeout_fut)) => Ok(send),
                            Either::B((_, _connect_fut)) => Err(other_error("Send timeout")),
                        })
                        .or_else(move |_| {
                            let request = NetworkRequest::DisconnectWithPeer(peer);
                            requests_tx.clone().send(request).map(forget_result)
                        })
                        .map_err(log_error);

                    handle.spawn(timeouted_connect);
                }
                NetworkRequest::DisconnectWithPeer(peer) => {
                    outgoing_connections.remove(&peer);

                    let event = NetworkEvent::PeerDisconnected(peer);
                    let event_handle = events_tx.clone().send(event).map(forget_result).map_err(
                        log_error,
                    );
                    handle.spawn(event_handle);
                }
            }

            Ok(())
        });

        // Incoming connections handler
        let listener = TcpListener::bind(&listen_addr, &core.handle()).unwrap();
        let events_tx = events_tx.clone();
        let server = listener
            .incoming()
            .fold(events_tx, move |events_tx, (sock, addr)| {
                info!("Tokio: Accepted incoming connection with peer={}", addr);

                let stream = sock.framed(MessagesCodec);
                let (_, stream) = stream.split();
                let events_tx = events_tx.clone();
                stream
                    .into_future()
                    .map_err(|e| e.0)
                    .and_then(move |(raw, stream)| {
                        let msg = raw.map(Any::from_raw);
                        if let Some(Ok(Any::Connect(msg))) = msg {
                            Ok((msg, stream))
                        } else {
                            Err(other_error("First message is not Connect"))
                        }
                    })
                    .and_then(move |(connect, stream)| {
                        info!("Tokio: Received handshake message={:?}", connect);

                        let addr = connect.addr();
                        let event = NetworkEvent::PeerConnected(addr, connect);
                        let connect_event = events_tx.clone().send(event).map_err(into_other);

                        let events_tx = events_tx.clone();
                        let messages_stream = stream.for_each(move |raw| {
                            let event = NetworkEvent::MessageReceived(addr, raw);
                            events_tx.clone().send(event).map(forget_result).map_err(
                                into_other,
                            )
                        });

                        messages_stream
                            .join(connect_event)
                            .map(move |(_, stream)| stream)
                            .map_err(into_other)
                    })
                    .map_err(into_other)
            })
            .map(forget_result)
            .map_err(log_error);
        core.handle().spawn(server);

        core.run(requests_handle).unwrap();
    });

    let mut handler = node.handler;
    let mut core = node.core;
    handler.initialize();

    let events_handle = events_rx.merge(timeouts_rx).for_each(move |item| {
        match item {
            MergedItem::First(event) => {
                handler.handle_network_event(event);
            }
            MergedItem::Second(timeout) => {
                handler.handle_timeout(timeout);
            }
            MergedItem::Both(event, timeout) => {
                handler.handle_timeout(timeout);
                handler.handle_network_event(event);
            }
        }
        Ok(())
    });
    core.run(events_handle).unwrap();
    debug!("Tokio: Handler thread is gone");

    network_thread.join().unwrap();
    Ok(())
}