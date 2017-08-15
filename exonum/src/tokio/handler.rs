use futures::sync::mpsc;
use futures::{Future, Stream, Sink, Poll, Async};
use futures::stream::Fuse;
use tokio_core::reactor::{Handle, Timeout};

use std::io;
use std::time::{Duration, SystemTime};
use std::net::SocketAddr;

use events::Channel;
use node::{ExternalMessage, NodeTimeout};
use messages::RawMessage;

use super::error::{forget_result, log_error, into_other};
use super::network::{NetworkEvent, NetworkRequest};

#[derive(Debug)]
pub enum Event {
    Network(NetworkEvent),
    Timeout(NodeTimeout),
    Api(ExternalMessage),
}

/// Channel for messages and timeouts.
#[derive(Debug, Clone)]
pub struct NodeSender {
    pub listen_addr: SocketAddr,
    pub timeout: mpsc::Sender<NodeTimeout>,
    pub network: mpsc::Sender<NetworkRequest>,
    pub external: mpsc::Sender<ExternalMessage>,
}

#[derive(Debug)]
pub struct NodeReceiver {
    pub timeout: mpsc::Receiver<NodeTimeout>,
    pub network: mpsc::Receiver<NetworkRequest>,
    pub external: mpsc::Receiver<ExternalMessage>,
}

#[derive(Debug)]
pub struct NodeChannel(pub NodeSender, pub NodeReceiver);

impl NodeChannel {
    pub fn new(listen_addr: SocketAddr, buffer: usize) -> NodeChannel {
        let (timeout_sender, timeout_receiver) = mpsc::channel(buffer);
        let (network_sender, network_receiver) = mpsc::channel(buffer);
        let (external_sender, external_receiver) = mpsc::channel(buffer);

        let sender = NodeSender {
            listen_addr,
            timeout: timeout_sender,
            network: network_sender,
            external: external_sender,
        };
        let receiver = NodeReceiver {
            timeout: timeout_receiver,
            network: network_receiver,
            external: external_receiver,
        };
        NodeChannel(sender, receiver)
    }
}

impl Channel for NodeSender {
    type ApplicationEvent = ExternalMessage;
    type Timeout = NodeTimeout;

    fn address(&self) -> SocketAddr {
        self.listen_addr
    }

    fn get_time(&self) -> SystemTime {
        SystemTime::now()
    }

    fn post_event(&self, handle: Handle, event: Self::ApplicationEvent) -> Result<(), io::Error> {
        let event_futute = self.external
            .clone()
            .send(event)
            .map(forget_result)
            .map_err(log_error);
        handle.spawn(event_futute);
        Ok(())
    }

    fn send_to(&mut self, handle: Handle, address: SocketAddr, message: RawMessage) {
        let request = NetworkRequest::SendMessage(address, message);
        let send_future = self.network
            .clone()
            .send(request)
            .map(forget_result)
            .map_err(log_error);
        handle.spawn(send_future);
    }

    fn add_timeout(&mut self, handle: Handle, timeout: Self::Timeout, time: SystemTime) {
        let duration = time.duration_since(self.get_time()).unwrap_or_else(|_| {
            Duration::from_secs(0)
        });
        let sender = self.timeout.clone();
        let timeout_handle = Timeout::new(duration, &handle)
            .expect("Unable to create timeout")
            .and_then(move |_| {
                sender.send(timeout).map(forget_result).map_err(into_other)
            })
            .map_err(|_| panic!("Can't timeout"));
        handle.spawn(timeout_handle);
    }
}

#[derive(Debug)]
pub struct EventsAggregator<S1, S2, S3>
where
    S1: Stream,
    S2: Stream,
    S3: Stream,
{
    timeout: Fuse<S1>,
    network: Fuse<S2>,
    api: Fuse<S3>,
}

impl<S1, S2, S3> EventsAggregator<S1, S2, S3>
where
    S1: Stream,
    S2: Stream,
    S3: Stream,
{
    pub fn new(timeout: S1, network: S2, api: S3) -> EventsAggregator<S1, S2, S3> {
        EventsAggregator {
            network: network.fuse(),
            timeout: timeout.fuse(),
            api: api.fuse(),
        }
    }
}

impl<S1, S2, S3> Stream for EventsAggregator<S1, S2, S3>
where
    S1: Stream<Item = NodeTimeout>,
    S2: Stream<
        Item = NetworkEvent,
        Error = S1::Error,
    >,
    S3: Stream<
        Item = ExternalMessage,
        Error = S1::Error,
    >,
{
    type Item = Event;
    type Error = S1::Error;

    fn poll(&mut self) -> Poll<Option<Event>, Self::Error> {
        let mut stream_finished = false;
        // Check timeout events
        match self.timeout.poll()? {
            Async::Ready(Some(item)) => return Ok(Async::Ready(Some(Event::Timeout(item)))),
            // Just finish stream
            Async::Ready(None) => stream_finished = true,
            Async::NotReady => {}
        };
        // Check network events
        match self.network.poll()? {
            Async::Ready(Some(item)) => return Ok(Async::Ready(Some(Event::Network(item)))),
            // Just finish stream
            Async::Ready(None) => stream_finished = true,
            Async::NotReady => {}
        };
        // Check api events
        match self.api.poll()? {
            Async::Ready(Some(item)) => return Ok(Async::Ready(Some(Event::Api(item)))),
            // Just finish stream
            Async::Ready(None) => stream_finished = true,
            Async::NotReady => {}
        };

        Ok(if stream_finished {
            Async::Ready(None)
        } else {
            Async::NotReady
        })
    }
}