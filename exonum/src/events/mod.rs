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

pub type EventLoop = mio::EventLoop<MioAdapter>;

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

pub struct InternalMessage;

pub enum Event {
    Incoming(RawMessage),
    Internal(InternalMessage),
    Connected(SocketAddr),
    Disconnected(SocketAddr),
    Timeout(NodeTimeout),
    Error(io::Error),
    Terminate,
}

pub struct Events {
    event_loop: EventLoop,
    queue: MioAdapter,
}

pub struct MioAdapter {
    events: VecDeque<Event>,
    network: Network,
}

impl MioAdapter {
    fn new(network: Network) -> MioAdapter {
        MioAdapter {
            // FIXME: configurable capacity?
            events: VecDeque::new(),
            network: network,
        }
    }

    fn push(&mut self, event: Event) {
        self.events.push_back(event)
    }

    fn pop(&mut self) -> Option<Event> {
        self.events.pop_front()
    }
}

impl mio::Handler for MioAdapter {
    type Timeout = Timeout;
    type Message = InternalMessage;

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) {
        loop {
            match self.network.io(event_loop, token, events) {
                Ok(Some(output)) => {
                    let event = match output {
                        Output::Data(buf) => Event::Incoming(RawMessage::new(buf)),
                        Output::Connected(addr) => Event::Connected(addr),
                        Output::Disconnected(addr) => Event::Disconnected(addr),
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

    fn notify(&mut self, _: &mut EventLoop, msg: Self::Message) {
        self.push(Event::Internal(msg));
    }

    fn timeout(&mut self, event_loop: &mut EventLoop, timeout: Self::Timeout) {
        match timeout {
            Timeout::Node(timeout) => {
                self.push(Event::Timeout(timeout));
            }
            Timeout::Internal(timeout) => {
                self.network.handle_timeout(event_loop, timeout);
            }
        }

    }

    fn interrupted(&mut self, _: &mut EventLoop) {
        self.push(Event::Terminate);
    }

    fn tick(&mut self, event_loop: &mut EventLoop) {
        self.network.tick(event_loop);
    }
}

pub trait Reactor {
    fn get_time(&self) -> Timespec;
    fn poll(&mut self) -> Event;
    fn bind(&mut self) -> ::std::io::Result<()>;
    fn send_to(&mut self, address: &SocketAddr, message: RawMessage);
    fn connect(&mut self, address: &SocketAddr);
    fn address(&self) -> SocketAddr;
    fn add_timeout(&mut self, timeout: NodeTimeout, time: Timespec);
}

impl Events {
    pub fn with_config(config: EventsConfiguration, network: Network) -> io::Result<Events> {
        // TODO: using EventLoopConfig + capacity of queue
        Ok(Events {
            event_loop: EventLoop::configured(config)?,
            queue: MioAdapter::new(network),
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

    fn bind(&mut self) -> ::std::io::Result<()> {
        self.queue.network.bind(&mut self.event_loop)
    }

    fn send_to(&mut self, address: &SocketAddr, message: RawMessage) {
        self.queue.network.send_to(&mut self.event_loop, address, message)
    }

    fn connect(&mut self, address: &SocketAddr) {
        if let Err(e) = self.queue.network.connect(&mut self.event_loop, address) {
            error!("{}: An error occured {:?}", self.queue.network.address(), e);
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
}

#[cfg(test)]
mod tests {
    use std::{time, thread};
    use std::net::SocketAddr;

    use time::Duration;

    use super::{Events, Reactor, EventsConfiguration, Event, NodeTimeout};
    use super::{Network, NetworkConfiguration};

    use ::messages::{MessageWriter, RawMessage};
    use ::crypto::gen_keypair;

    impl Events {
        fn with_addr(addr: SocketAddr) -> Events {
            let network = Network::with_config(NetworkConfiguration {
                listen_address: addr,
                max_connections: 256,
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
                    Event::Error(_) => return None,
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
                    Event::Connected(_) => return,
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
                    Event::Error(_) => return,
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
                let mut e = Events::with_addr(addrs[0].clone());
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
                let mut e = Events::with_addr(addrs[1].clone());
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
                    let mut e = Events::with_addr(addrs[0].clone());
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
                    let mut e = Events::with_addr(addrs[0].clone());
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
                    let mut e = Events::with_addr(addrs[1].clone());
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
                    let mut e = Events::with_addr(addrs[1].clone());
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
