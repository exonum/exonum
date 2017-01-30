use std::collections::{VecDeque, BinaryHeap};
use std::sync::{Arc, Mutex};
use std::net::SocketAddr;

use time::Timespec;

use ::node::{ExternalMessage, NodeTimeout};
use ::messages::RawMessage;
use ::events::{Event, InternalEvent, Channel, Result as EventsResult};

type SandboxEvent = InternalEvent<ExternalMessage, NodeTimeout>;

#[derive(PartialEq, Eq)]
pub struct TimerPair(pub Timespec, pub NodeTimeout);

impl PartialOrd for TimerPair {
    fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
        Some((&self.0, &self.1).cmp(&(&other.0, &other.1)).reverse())
    }
}

impl Ord for TimerPair {
    fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
        (&self.0, &self.1).cmp(&(&other.0, &other.1)).reverse()
    }
}

pub struct SandboxInner {
    pub address: SocketAddr,
    pub time: Timespec,
    pub sended: VecDeque<(SocketAddr, RawMessage)>,
    pub events: VecDeque<SandboxEvent>,
    pub timers: BinaryHeap<TimerPair>,
}

#[derive(Clone)]
pub struct SandboxChannel {
    pub inner: Arc<Mutex<SandboxInner>>,
}

impl SandboxChannel {
    fn send_event(&self, event: SandboxEvent) {
        self.inner.lock().unwrap().events.push_back(event);
    }

    fn send_message(&self, address: &SocketAddr, message: RawMessage) {
        self.inner.lock().unwrap().sended.push_back((address.clone(), message));
    }
}

impl Channel for SandboxChannel {
    type ApplicationEvent = ExternalMessage;
    type Timeout = NodeTimeout;

    fn address(&self) -> SocketAddr {
        self.inner.lock().unwrap().address
    }

    fn get_time(&self) -> Timespec {
        self.inner.lock().unwrap().time
    }

    fn post_event(&self, event: Self::ApplicationEvent) -> EventsResult<()> {
        let msg = InternalEvent::Application(event);
        self.send_event(msg);
        Ok(())
    }

    fn send_to(&mut self, address: &SocketAddr, message: RawMessage) {
        // TODO handle attempts to send message to offline nodes
        self.send_message(address, message);
    }

    fn connect(&mut self, address: &SocketAddr) {
        let event = InternalEvent::Node(Event::Connected(*address));
        self.send_event(event);
    }

    fn add_timeout(&mut self, timeout: Self::Timeout, time: Timespec) {
        // assert!(time < self.inner.borrow().time, "Tring to add timeout for the past");
        let pair = TimerPair(time, timeout);
        self.inner.lock().unwrap().timers.push(pair);
    }
}