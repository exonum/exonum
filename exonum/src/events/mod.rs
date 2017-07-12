/*
 * Copyright 2017 The Exonum Team
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
*/
use mio;

use std::io;
use std::fmt;
use std::error;
use std::result;
use std::net::SocketAddr;
use std::time::{SystemTime, Duration};

use messages::RawMessage;

pub use self::network::{Network, NetworkConfiguration, PeerId, EventSet};

mod network;
mod connection;
#[cfg(test)]
mod tests;

pub type EventsConfiguration = mio::EventLoopConfig;

pub type EventLoop<H> = mio::EventLoop<MioAdapter<H>>;

pub type Milliseconds = u64;

#[derive(Debug)]
pub enum Event {
    Incoming(RawMessage),
    Connected(SocketAddr),
    Disconnected(SocketAddr),
    Error(io::Error),
}

#[derive(Debug)]
pub enum InternalEvent<A: Send, T: Send> {
    Node(Event),
    Application(A),
    Invoke(Invoke<T>),
}

#[derive(Debug)]
pub enum InternalTimeout {
    Reconnect(SocketAddr, u64),
}

#[derive(Debug)]
pub enum Timeout<N: Send> {
    Node(N),
    Internal(InternalTimeout),
}

#[derive(Debug)]
pub enum Invoke<T: Send> {
    SendTo(SocketAddr, RawMessage),
    Connect(SocketAddr),
    AddTimeout(T, SystemTime),
}

pub trait EventHandler {
    type Timeout: Send;
    type ApplicationEvent: Send;

    fn handle_event(&mut self, event: Event);
    fn handle_timeout(&mut self, timeout: Self::Timeout);
    fn handle_application_event(&mut self, event: Self::ApplicationEvent);
}

pub trait Channel: Sync + Send + Clone {
    type ApplicationEvent: Send;
    type Timeout: Send;

    fn get_time(&self) -> SystemTime;
    fn address(&self) -> SocketAddr;

    fn post_event(&self, msg: Self::ApplicationEvent) -> Result<()>;
    fn send_to(&mut self, address: &SocketAddr, message: RawMessage);
    fn connect(&mut self, address: &SocketAddr);
    fn add_timeout(&mut self, timeout: Self::Timeout, time: SystemTime);
}

pub trait Reactor<H: EventHandler> {
    type Channel: Channel<ApplicationEvent = H::ApplicationEvent, Timeout = H::Timeout>;

    fn bind(&mut self) -> ::std::io::Result<()>;
    fn run(&mut self) -> ::std::io::Result<()>;
    fn run_once(&mut self, timeout: Option<usize>) -> ::std::io::Result<()>;
    fn get_time(&self) -> SystemTime;
    fn channel(&self) -> Self::Channel;
}

#[derive(Debug)]
pub struct MioAdapter<H: EventHandler> {
    network: Network,
    handler: H,
}

pub struct Events<H: EventHandler> {
    inner: MioAdapter<H>,
    event_loop: EventLoop<H>,
}

#[derive(Debug)]
pub struct MioChannel<E: Send, T: Send> {
    address: SocketAddr,
    inner: mio::Sender<InternalEvent<E, T>>,
}

impl<E: Send, T: Send> MioChannel<E, T> {
    pub fn new(addr: SocketAddr, inner: mio::Sender<InternalEvent<E, T>>) -> MioChannel<E, T> {
        MioChannel {
            address: addr,
            inner: inner.clone(),
        }
    }
}

impl<E: Send, T: Send> Clone for MioChannel<E, T> {
    fn clone(&self) -> MioChannel<E, T> {
        MioChannel {
            address: self.address,
            inner: self.inner.clone(),
        }
    }
}

impl<E: Send, T: Send> Channel for MioChannel<E, T> {
    type ApplicationEvent = E;
    type Timeout = T;

    fn address(&self) -> SocketAddr {
        self.address
    }

    fn get_time(&self) -> SystemTime {
        SystemTime::now()
    }

    fn post_event(&self, event: Self::ApplicationEvent) -> Result<()> {
        let msg = InternalEvent::Application(event);
        self.inner.send(msg).map_err(|e| {
            error!("An error occured: {}", e);
            Error::new(e.to_string())
        })
    }

    fn send_to(&mut self, address: &SocketAddr, message: RawMessage) {
        self.inner
            .send(InternalEvent::Invoke(Invoke::SendTo(*address, message)))
            .log_error("Unable to send to");
    }

    fn connect(&mut self, address: &SocketAddr) {
        self.inner
            .send(InternalEvent::Invoke(Invoke::Connect(*address)))
            .log_error("Unable to connect");
    }

    fn add_timeout(&mut self, timeout: Self::Timeout, time: SystemTime) {
        self.inner
            .send(InternalEvent::Invoke(Invoke::AddTimeout(timeout, time)))
            .log_error("Unable to add timeout");
    }
}

// TODO think about more ergonomic design with ChannelFactory or other solution

impl<H: EventHandler> Events<H> {
    pub fn new(network: Network, handler: H) -> io::Result<Events<H>> {
        let event_loop = EventLoop::<H>::new()?;
        let events = Events {
            inner: MioAdapter {
                network: network,
                handler: handler,
            },
            event_loop: event_loop,
        };
        Ok(events)
    }

    pub fn with_event_loop(network: Network, handler: H, event_loop: EventLoop<H>) -> Events<H> {
        Events {
            inner: MioAdapter {
                network: network,
                handler: handler,
            },
            event_loop: event_loop,
        }
    }

    pub fn handler(&self) -> &H {
        &self.inner.handler
    }

    pub fn handler_mut(&mut self) -> &mut H {
        &mut self.inner.handler
    }
}

impl<H: EventHandler> fmt::Debug for Events<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Events { .. }")
    }
}

impl<H: EventHandler> MioAdapter<H> {
    fn handle_invoke(&mut self, event_loop: &mut EventLoop<H>, method: Invoke<H::Timeout>) {
        match method {
            Invoke::Connect(address) => self.handle_connect(event_loop, &address),
            Invoke::SendTo(address, message) => self.handle_send_to(event_loop, &address, message),
            Invoke::AddTimeout(timeout, time) => self.handle_add_timeout(event_loop, timeout, time),
        }
    }

    fn handle_send_to(
        &mut self,
        event_loop: &mut EventLoop<H>,
        address: &SocketAddr,
        message: RawMessage,
    ) {
        if self.network.is_connected(address) {
            if let Err(e) = self.network.send_to(event_loop, address, message) {
                error!(
                    "{}: An error during send_to occurred {:?}",
                    self.network.address(),
                    e
                );
                self.handler.handle_event(Event::Disconnected(*address));
            }
        } else {
            warn!(
                "{}: Unable to send message to {}, connection does not established",
                self.network.address(),
                address
            );
        }
    }

    fn handle_connect(&mut self, event_loop: &mut EventLoop<H>, address: &SocketAddr) {
        self.network.connect(event_loop, address).log_error(
            format!(
                "Unable to connect with {}",
                address
            ),
        );
    }

    fn handle_add_timeout(
        &mut self,
        event_loop: &mut EventLoop<H>,
        timeout: H::Timeout,
        time: SystemTime,
    ) {
        match time.duration_since(SystemTime::now()) {
            Ok(duration) => {
                event_loop
                    .timeout_ms(Timeout::Node(timeout), num_milliseconds(&duration))
                    .map(|_| ())
                    .map_err(|x| format!("{:?}", x))
                    .log_error("Unable to add timeout to the event loop")
            }
            Err(_) => self.handler.handle_timeout(timeout),
        }
    }
}

impl<H: EventHandler> mio::Handler for MioAdapter<H> {
    type Timeout = Timeout<H::Timeout>;
    type Message = InternalEvent<H::ApplicationEvent, H::Timeout>;

    fn ready(&mut self, event_loop: &mut EventLoop<H>, token: mio::Token, events: mio::EventSet) {
        self.network
            .io(event_loop, &mut self.handler, token, events)
            .log_error(format!("{}", self.network.address()));
    }

    fn notify(&mut self, event_loop: &mut EventLoop<H>, msg: Self::Message) {
        match msg {
            InternalEvent::Node(event) => self.handler.handle_event(event),
            InternalEvent::Invoke(args) => self.handle_invoke(event_loop, args),
            InternalEvent::Application(event) => self.handler.handle_application_event(event),
        }
    }

    fn timeout(&mut self, event_loop: &mut EventLoop<H>, timeout: Self::Timeout) {
        match timeout {
            Timeout::Node(timeout) => {
                self.handler.handle_timeout(timeout);
            }
            Timeout::Internal(timeout) => {
                self.network.handle_timeout(event_loop, timeout);
            }
        }
    }

    // TODO think about interrupted handlers
    // fn interrupted(&mut self, _: &mut EventLoop) {
    //     self.push(Event::Terminate);
    // }

    fn tick(&mut self, event_loop: &mut EventLoop<H>) {
        self.network.tick(event_loop);
    }
}

impl<H: EventHandler> Reactor<H> for Events<H> {
    type Channel = MioChannel<H::ApplicationEvent, H::Timeout>;

    fn bind(&mut self) -> ::std::io::Result<()> {
        self.inner.network.bind(&mut self.event_loop)
    }
    fn run(&mut self) -> ::std::io::Result<()> {
        self.event_loop.run(&mut self.inner)
    }
    fn run_once(&mut self, timeout: Option<usize>) -> ::std::io::Result<()> {
        self.event_loop.run_once(&mut self.inner, timeout)
    }
    fn get_time(&self) -> SystemTime {
        SystemTime::now()
    }
    fn channel(&self) -> MioChannel<H::ApplicationEvent, H::Timeout> {
        MioChannel {
            inner: self.event_loop.channel(),
            address: *self.inner.network.address(),
        }
    }
}

trait LogError {
    fn log_error<S: AsRef<str>>(self, msg: S);
}

impl<E> LogError for result::Result<(), E>
where
    E: fmt::Display,
{
    fn log_error<S: AsRef<str>>(self, msg: S) {
        if let Err(error) = self {
            error!("{}, an error occurred: {}", msg.as_ref(), error);
        }
    }
}

#[derive(Debug)]
pub struct Error {
    message: String,
}

pub type Result<T> = result::Result<T, Error>;

impl Error {
    pub fn new<T: Into<String>>(message: T) -> Error {
        Error { message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Events error: {}", self.message)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        &self.message
    }
}

fn num_milliseconds(duration: &Duration) -> Milliseconds {
    const MILLIS_PER_SEC: u64 = 1000;
    const NANOS_PER_MILLI: u32 = 1000_000;

    let secs_part = duration.as_secs() * MILLIS_PER_SEC;
    let nanos_part = duration.subsec_nanos() / NANOS_PER_MILLI;
    secs_part + nanos_part as Milliseconds
}
