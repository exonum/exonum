use futures::{Sink, Future};
use futures::sync::mpsc;
use tokio_core::reactor::{Core, Handle, Timeout};

use std::io;
use std::sync::{Mutex, Arc};
use std::error::Error as StdError;
use std::time::{Duration, SystemTime};
use std::net::SocketAddr;

use events::Channel;
use node::{ExternalMessage, NodeTimeout};
use messages::RawMessage;

use super::error::{forget_result, log_error, into_other};

#[derive(Debug, Clone)]
pub enum NetworkRequest {
    SendMessage(SocketAddr, RawMessage),
    DisconnectWithPeer(SocketAddr),
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
