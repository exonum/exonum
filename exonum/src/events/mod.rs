use std::io;
use std::net::SocketAddr;
use std::collections::VecDeque;
use time::{get_time, Timespec};

use mio;

use super::messages::RawMessage;

use super::node::RequestData;
use super::crypto::PublicKey;

mod network;
mod connection;

pub use self::network::{Network, NetworkConfiguration, PeerId, EventSet, Output};

pub type EventsConfiguration = mio::EventLoopConfig;

pub type EventLoop<E> = mio::EventLoop<MioAdapter<E>>;

// FIXME: move this into node module
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum NodeTimeout {
    Status,
    Round(u64, u32),
    Request(RequestData, Option<PublicKey>),
    PeerExchange,
}

#[derive(PartialEq, Eq, Clone)]
pub enum InternalTimeout {
    Reconnect(SocketAddr, u64),
}

#[derive(PartialEq, Eq, Clone)]
pub enum Timeout {
    Node(NodeTimeout),
    Internal(InternalTimeout),
}

pub enum InternalEvent<E: Send> {
    Connected(SocketAddr),
    Disconnected(SocketAddr),
    Error(io::Error),
    External(E),
}

pub enum Event<E: Send> {
    Incoming(RawMessage),
    Internal(InternalEvent<E>),
    Timeout(NodeTimeout),
    Terminate,
}

pub struct Events<E: Send> {
    event_loop: EventLoop<E>,
    queue: MioAdapter<E>,
}

pub struct MioAdapter<E: Send> {
    events: VecDeque<Event<E>>,
    network: Network,
}

pub trait EventHandler {
    type Timeout: Send;
    type ExternalEvent: Send;

    fn handle_message(&mut self, RawMessage);
    fn handle_event(&mut self, event: InternalEvent<Self::ExternalEvent>);
    fn handle_timeout(&mut self, timeout: Self::Timeout);
}

impl<E: Send> MioAdapter<E> {
    fn new(network: Network) -> MioAdapter<E> {
        MioAdapter {
            // FIXME: configurable capacity?
            events: VecDeque::new(),
            network: network,
        }
    }

    fn push(&mut self, event: Event<E>) {
        self.events.push_back(event)
    }

    fn pop(&mut self) -> Option<Event<E>> {
        self.events.pop_front()
    }
}

impl<E: Send> mio::Handler for MioAdapter<E> {
    type Timeout = Timeout;
    type Message = InternalEvent<E>;

    fn ready(&mut self, event_loop: &mut EventLoop<E>, token: mio::Token, events: mio::EventSet) {
        loop {
            match self.network.io(event_loop, token, events) {
                Ok(Some(output)) => {
                    let event = match output {
                        Output::Data(buf) => Event::Incoming(RawMessage::new(buf)),
                        Output::Connected(addr) => Event::Internal(InternalEvent::Connected(addr)),
                        Output::Disconnected(addr) => {
                            Event::Internal(InternalEvent::Disconnected(addr))
                        }
                    };
                    self.push(event);
                }
                Ok(None) => break,
                Err(e) => {
                    error!("{}: An error occured {:?}", self.network.address(), e);
                    break;
                }
            }
        }
    }

    fn notify(&mut self, _: &mut EventLoop<E>, msg: Self::Message) {
        self.push(Event::Internal(msg));
    }

    fn timeout(&mut self, event_loop: &mut EventLoop<E>, timeout: Self::Timeout) {
        match timeout {
            Timeout::Node(timeout) => {
                self.push(Event::Timeout(timeout));
            }
            Timeout::Internal(timeout) => {
                self.network.handle_timeout(event_loop, timeout);
            }
        }

    }

    fn interrupted(&mut self, _: &mut EventLoop<E>) {
        self.push(Event::Terminate);
    }

    fn tick(&mut self, event_loop: &mut EventLoop<E>) {
        self.network.tick(event_loop);
    }
}

pub trait Sender<Message: Send>: Send {
    fn send(&self, msg: Message) -> ::std::io::Result<()>;
}

pub trait Reactor<E: Send> {
    fn get_time(&self) -> Timespec;
    fn poll(&mut self) -> Event<E>;
    fn bind(&mut self) -> ::std::io::Result<()>;
    fn send_to(&mut self, address: &SocketAddr, message: RawMessage);
    fn connect(&mut self, address: &SocketAddr);
    fn address(&self) -> SocketAddr;
    fn add_timeout(&mut self, timeout: NodeTimeout, time: Timespec);
    fn channel(&self) -> Box<Sender<InternalEvent<E>>>;
}

impl<E: Send> Events<E> {
    pub fn with_config(config: EventsConfiguration, network: Network) -> io::Result<Events<E>> {
        // TODO: using EventLoopConfig + capacity of queue
        Ok(Events {
            event_loop: EventLoop::configured(config)?,
            queue: MioAdapter::new(network),
        })
    }
}

type MioSender<E> = mio::Sender<InternalEvent<E>>;

impl<E: Send + 'static> Sender<InternalEvent<E>> for MioSender<E> {
    fn send(&self, msg: InternalEvent<E>) -> io::Result<()> {
        let r = self.send(msg);
        r.map_err(|_| io::Error::new(io::ErrorKind::Other, "Unable to send message to reactor"))
    }
}

impl<E: Send + 'static> Reactor<E> for Events<E> {
    fn get_time(&self) -> Timespec {
        get_time()
    }

    fn poll(&mut self) -> Event<E> {
        loop {
            if let Some(event) = self.queue.pop() {
                return event;
            }
            if let Err(err) = self.event_loop.run_once(&mut self.queue, None) {
                self.queue.push(Event::Internal(InternalEvent::Error(err)))
            }
        }
    }

    fn bind(&mut self) -> ::std::io::Result<()> {
        self.queue.network.bind(&mut self.event_loop)
    }

    fn send_to(&mut self, address: &SocketAddr, message: RawMessage) {
        if self.queue.network.is_connected(address) {
            if let Err(e) = self.queue.network.send_to(&mut self.event_loop, address, message) {
                error!("{}: An error during send_to occured {:?}",
                       self.queue.network.address(),
                       e);
                self.queue.push(Event::Internal(InternalEvent::Disconnected(*address)));
            }
        } else {
            warn!("{}: Unable to send message to {}, connection does not established",
                  self.queue.network.address(),
                  address);
        }
    }

    fn connect(&mut self, address: &SocketAddr) {
        if let Err(e) = self.queue.network.connect(&mut self.event_loop, address) {
            error!("{}: An error during connect occured {:?}",
                   self.queue.network.address(),
                   e);
        }
    }

    fn address(&self) -> SocketAddr {
        *self.queue.network.address()
    }

    fn add_timeout(&mut self, timeout: NodeTimeout, time: Timespec) {
        let ms = (time - self.get_time()).num_milliseconds();
        if ms < 0 {
            self.queue.push(Event::Timeout(timeout))
        } else {
            // FIXME: remove unwrap here
            // TODO: use mio::Timeout
            self.event_loop.timeout_ms(Timeout::Node(timeout), ms as u64).unwrap();
        }
    }

    fn channel(&self) -> Box<Sender<InternalEvent<E>>> {
        Box::new(self.event_loop.channel())
    }
}

#[cfg(test)]
mod tests {
    use std::{time, thread};
    use std::net::SocketAddr;

    use time::Duration;

    use super::{Events, Reactor, EventsConfiguration, Event, NodeTimeout, InternalEvent};
    use super::{Network, NetworkConfiguration};

    use ::messages::{MessageWriter, RawMessage};
    use ::crypto::gen_keypair;

    impl<E: Send + 'static> Events<E> {
        fn with_addr(addr: SocketAddr) -> Events<E> {
            let network = Network::with_config(NetworkConfiguration {
                listen_address: addr,
                max_incoming_connections: 128,
                max_outgoing_connections: 128,
                tcp_nodelay: true,
                tcp_keep_alive: None,
                tcp_reconnect_timeout: 1000,
                tcp_reconnect_timeout_max: 600000,
            });
            Events::with_config(EventsConfiguration::new(), network).unwrap()
        }

        fn wait_for_msg(&mut self, timeout: Duration) -> Option<RawMessage> {
            let time = self.get_time() + timeout;
            self.add_timeout(NodeTimeout::Status, time);
            loop {
                match self.poll() {
                    Event::Incoming(msg) => return Some(msg),
                    Event::Timeout(_) => return None,
                    Event::Internal(InternalEvent::Error(_)) => return None,
                    _ => {}
                }
            }
        }

        fn wait_for_bind(&mut self, addr: &SocketAddr) {
            self.bind().unwrap();
            thread::sleep(time::Duration::from_millis(1000));

            // TODO timeout
            self.connect(addr);
            loop {
                match self.poll() {
                    Event::Internal(InternalEvent::Connected(_)) => return,
                    _ => {}
                }
            }
        }

        fn process_events(&mut self, timeout: Duration) {
            let time = self.get_time() + timeout;
            self.add_timeout(NodeTimeout::Status, time);
            loop {
                match self.poll() {
                    Event::Timeout(_) => break,
                    Event::Internal(InternalEvent::Error(_)) => return,
                    _ => {}
                }
            }
        }
    }

    fn gen_message(id: u16, len: usize) -> RawMessage {
        let writer = MessageWriter::new(id, len);
        RawMessage::new(writer.sign(&gen_keypair().1))
    }

    #[test]
    fn big_message() {
        let addrs: [SocketAddr; 2] = ["127.0.0.1:8200".parse().unwrap(),
                                      "127.0.0.1:8201".parse().unwrap()];

        let m1 = gen_message(15, 1000000);
        let m2 = gen_message(16, 400);

        let t1;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            t1 = thread::spawn(move || {
                let mut e = Events::<u32>::with_addr(addrs[0].clone());
                e.wait_for_bind(&addrs[1]);

                e.send_to(&addrs[1], m1);
                assert_eq!(e.wait_for_msg(Duration::milliseconds(1000)), Some(m2));
                e.process_events(Duration::milliseconds(10000));
            });
        }

        let t2;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            t2 = thread::spawn(move || {
                let mut e = Events::<u32>::with_addr(addrs[1].clone());
                e.wait_for_bind(&addrs[0]);

                e.send_to(&addrs[0], m2);
                assert_eq!(e.wait_for_msg(Duration::milliseconds(30000)), Some(m1));
            });
        }

        t2.join().unwrap();
        t1.join().unwrap();
    }

    #[test]
    fn reconnect() {
        let addrs: [SocketAddr; 2] = ["127.0.0.1:9000".parse().unwrap(),
                                      "127.0.0.1:9001".parse().unwrap()];

        let m1 = gen_message(15, 250);
        let m2 = gen_message(16, 400);
        let m3 = gen_message(17, 600);

        let t1;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            let m3 = m3.clone();
            t1 = thread::spawn(move || {
                {
                    let mut e = Events::<u32>::with_addr(addrs[0].clone());
                    e.wait_for_bind(&addrs[1]);

                    println!("t1: connection opened");
                    println!("t1: send m1 to t2");
                    e.send_to(&addrs[1], m1);
                    println!("t1: wait for m2");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)), Some(m2));
                    println!("t1: received m2 from t2");
                    e.process_events(Duration::milliseconds(100));
                    drop(e);
                }
                println!("t1: connection closed");
                {
                    let mut e = Events::<u32>::with_addr(addrs[0].clone());
                    e.wait_for_bind(&addrs[1]);

                    println!("t1: connection reopened");
                    println!("t1: send m3 to t2");
                    e.send_to(&addrs[1], m3.clone());
                    println!("t1: wait for m3");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)), Some(m3));
                    e.process_events(Duration::milliseconds(100));
                    println!("t1: received m3 from t2");
                }
                println!("t1: connection closed");
            });
        }

        let t2;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            let m3 = m3.clone();
            t2 = thread::spawn(move || {
                {
                    let mut e = Events::<u32>::with_addr(addrs[1].clone());
                    e.wait_for_bind(&addrs[0]);

                    println!("t2: connection opened");
                    println!("t2: send m2 to t1");
                    e.send_to(&addrs[0], m2);
                    println!("t2: wait for m1");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)), Some(m1));
                    println!("t2: received m1 from t1");
                    println!("t2: wait for m3");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)),
                               Some(m3.clone()));
                    println!("t2: received m3 from t1");
                    e.process_events(Duration::milliseconds(100));
                    drop(e);
                }
                println!("t2: connection closed");
                {
                    println!("t2: connection reopened");
                    let mut e = Events::<u32>::with_addr(addrs[1].clone());
                    e.wait_for_bind(&addrs[0]);

                    println!("t2: send m3 to t1");
                    e.send_to(&addrs[0], m3.clone());
                    e.process_events(Duration::milliseconds(100));
                }
                println!("t2: connection closed");
            });
        }

        t2.join().unwrap();
        t1.join().unwrap();
    }
}
