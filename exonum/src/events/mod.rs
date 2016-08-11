use std::io;
use std::net::SocketAddr;
use std::collections::VecDeque;
use time::{get_time, Timespec};

use mio;

use super::messages::RawMessage;

use super::node::{RequestData, ValidatorId};

mod network;
mod connection;

pub use self::network::{Network, NetworkConfiguration, PeerId, EventSet};

pub type EventsConfiguration = mio::EventLoopConfig;

pub type EventLoop = mio::EventLoop<EventsQueue>;

// FIXME: move this into node module
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum Timeout {
    Status,
    Round(u64, u32),
    Request(RequestData, ValidatorId),
    PeerExchange
}

pub struct InternalMessage;

pub enum Event {
    Incoming(RawMessage),
    Internal(InternalMessage),
    Timeout(Timeout),
    Io(mio::Token, mio::EventSet),
    Error(io::Error),
    Terminate
}

pub struct Events {
    event_loop: EventLoop,
    queue: EventsQueue,
    network: Network,
}

pub struct EventsQueue {
    events: VecDeque<Event>
}

impl EventsQueue {
    fn new() -> EventsQueue {
        EventsQueue {
            // FIXME: configurable capacity?
            events: VecDeque::new(),
        }
    }

    fn push(&mut self, event: Event) {
        self.events.push_back(event)
    }

    fn pop(&mut self) -> Option<Event> {
        self.events.pop_front()
    }
}

impl mio::Handler for EventsQueue {
    type Timeout = Timeout;
    type Message = InternalMessage;

    fn ready(&mut self, _: &mut EventLoop,
             token: mio::Token, events: mio::EventSet) {
        self.push(Event::Io(token, events));
    }

    fn notify(&mut self, _: &mut EventLoop, msg: Self::Message) {
        self.push(Event::Internal(msg));
    }

    fn timeout(&mut self, _: &mut EventLoop, timeout: Self::Timeout) {
        self.push(Event::Timeout(timeout));
    }

    fn interrupted(&mut self, _: &mut EventLoop) {
        self.push(Event::Terminate);
    }
}

pub trait Reactor {
    fn get_time(&self) -> Timespec;
    fn poll(&mut self) -> Event;
    fn io(&mut self, id: PeerId, set: EventSet) -> io::Result<()>;
    fn bind(&mut self) -> ::std::io::Result<()>;
    fn send_to(&mut self,
                   address: &SocketAddr,
                   message: RawMessage) -> io::Result<()>;
    fn address(&self) -> SocketAddr;
    fn add_timeout(&mut self, timeout: Timeout, time: Timespec);
}

impl Events {
    pub fn with_config(config: EventsConfiguration,
                       network: Network) -> io::Result<Events> {
        // TODO: using EventLoopConfig + capacity of queue
        Ok(Events {
            event_loop: EventLoop::configured(config)?,
            queue: EventsQueue::new(),
            network: network
        })
    }
}

impl Reactor for Events {
    fn get_time(&self) -> Timespec {
        get_time()
    }

    fn poll(&mut self) -> Event {
        loop {
            if let Some(event) = self.queue.pop() {
                return event;
            }
            if let Err(err) = self.event_loop.run_once(&mut self.queue, None) {
                self.queue.push(Event::Error(err))
            }
        }
    }

    fn io(&mut self, id: PeerId, set: EventSet) -> io::Result<()> {
        while let Some(buf) = self.network.io(&mut self.event_loop, id, set)? {
            self.queue.push(Event::Incoming(RawMessage::new(buf)));
        }
        Ok(())
    }


    fn bind(&mut self) -> ::std::io::Result<()> {
        self.network.bind(&mut self.event_loop)
    }

    fn send_to(&mut self,
                   address: &SocketAddr,
                   message: RawMessage) -> io::Result<()> {
        self.network.send_to(&mut self.event_loop, address, message)
    }

    fn address(&self) -> SocketAddr {
        self.network.address().clone()
    }

    fn add_timeout(&mut self,
                   timeout: Timeout,
                   time: Timespec) {
        let ms = (time - self.get_time()).num_milliseconds();
        if ms < 0 {
            self.queue.push(Event::Timeout(timeout))
        } else {
            // FIXME: remove unwrap here
            // TODO: use mio::Timeout
            self.event_loop.timeout_ms(timeout, ms as u64).unwrap();
        }
    }
}
