use std::io;
use std::net::SocketAddr;
use time::{get_time, Timespec};

use mio;

use super::messages::RawMessage;

mod network;
mod connection;

pub use self::network::{Network, NetworkConfiguration, PeerId, EventSet, Output};

pub type EventsConfiguration = mio::EventLoopConfig;

pub type EventLoop<H> = mio::EventLoop<MioAdapter<H>>;

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
    AddTimeout(T, Timespec),
}

pub trait EventHandler {
    type Timeout: Send;
    type ApplicationEvent: Send;

    fn handle_event(&mut self, event: Event);
    fn handle_timeout(&mut self, timeout: Self::Timeout);
    fn handle_application_event(&mut self, event: Self::ApplicationEvent);
}

pub trait Channel: Send {
    type ApplicationEvent: Send;
    type Timeout: Send;

    fn get_time(&self) -> Timespec;
    fn address(&self) -> SocketAddr;

    fn post_event(&self, msg: Self::ApplicationEvent) -> ::std::io::Result<()>;
    fn send_to(&mut self, address: &SocketAddr, message: RawMessage);
    fn connect(&mut self, address: &SocketAddr);
    fn add_timeout(&mut self, timeout: Self::Timeout, time: Timespec);
}

pub trait Reactor<H: EventHandler> {
    type Channel: Channel<ApplicationEvent = H::ApplicationEvent, Timeout = H::Timeout>;

    fn bind(&mut self) -> ::std::io::Result<()>;
    fn run(&mut self) -> ::std::io::Result<()>;
    fn run_once(&mut self, timeout: Option<usize>) -> ::std::io::Result<()>;
    fn get_time(&self) -> Timespec;
    fn channel(&self) -> Self::Channel;
}

pub struct MioAdapter<H: EventHandler> {
    network: Network,
    handler: H,
}

pub struct Events<H: EventHandler> {
    inner: MioAdapter<H>,
    event_loop: EventLoop<H>,
}

#[derive(Clone)]
pub struct MioChannel<H: EventHandler> {
    address: SocketAddr,
    inner: mio::Sender<InternalEvent<H::ApplicationEvent, H::Timeout>>,
}

// TODO remove unwrap
impl<H: EventHandler> Channel for MioChannel<H> {
    type ApplicationEvent = H::ApplicationEvent;
    type Timeout = H::Timeout;

    fn address(&self) -> SocketAddr {
        self.address
    }

    fn get_time(&self) -> Timespec {
        get_time()
    }

    fn post_event(&self, event: Self::ApplicationEvent) -> io::Result<()> {
        let msg = InternalEvent::Application(event);
        let r = self.inner.send(msg);
        r.map_err(|_| io::Error::new(io::ErrorKind::Other, "Unable to send message to reactor"))
    }

    fn send_to(&mut self, address: &SocketAddr, message: RawMessage) {
        self.inner
            .send(InternalEvent::Invoke(Invoke::SendTo(*address, message)))
            .unwrap();
    }

    fn connect(&mut self, address: &SocketAddr) {
        self.inner
            .send(InternalEvent::Invoke(Invoke::Connect(*address)))
            .unwrap();
    }

    fn add_timeout(&mut self, timeout: Self::Timeout, time: Timespec) {
        self.inner
            .send(InternalEvent::Invoke(Invoke::AddTimeout(timeout, time)))
            .unwrap();
    }
}

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
}

impl<H: EventHandler> MioAdapter<H> {
    fn handle_invoke(&mut self, event_loop: &mut EventLoop<H>, method: Invoke<H::Timeout>) {
        match method {
            Invoke::Connect(address) => self.handle_connect(event_loop, &address),
            Invoke::SendTo(address, message) => self.handle_send_to(event_loop, &address, message),
            Invoke::AddTimeout(timeout, time) => self.handle_add_timeout(event_loop, timeout, time),
        }
    }

    fn handle_send_to(&mut self,
                      event_loop: &mut EventLoop<H>,
                      address: &SocketAddr,
                      message: RawMessage) {
        if self.network.is_connected(address) {
            if let Err(e) = self.network.send_to(event_loop, address, message) {
                error!("{}: An error during send_to occured {:?}",
                       self.network.address(),
                       e);
                self.handler.handle_event(Event::Disconnected(*address));
            }
        } else {
            warn!("{}: Unable to send message to {}, connection does not established",
                  self.network.address(),
                  address);
        }
    }

    fn handle_connect(&mut self, event_loop: &mut EventLoop<H>, address: &SocketAddr) {
        if let Err(e) = self.network.connect(event_loop, address) {
            error!("{}: An error during connect occured {:?}",
                   self.network.address(),
                   e);
        }
    }

    fn handle_add_timeout(&mut self,
                          event_loop: &mut EventLoop<H>,
                          timeout: H::Timeout,
                          time: Timespec) {
        let ms = (time - get_time()).num_milliseconds();
        if ms < 0 {
            self.handler.handle_timeout(timeout);
        } else {
            // FIXME: remove unwrap here
            // TODO: use mio::Timeout
            event_loop.timeout_ms(Timeout::Node(timeout), ms as u64).unwrap();
        }
    }
}

impl<H: EventHandler> mio::Handler for MioAdapter<H> {
    type Timeout = Timeout<H::Timeout>;
    type Message = InternalEvent<H::ApplicationEvent, H::Timeout>;

    fn ready(&mut self, event_loop: &mut EventLoop<H>, token: mio::Token, events: mio::EventSet) {
        loop {
            match self.network.io(event_loop, token, events) {
                Ok(Some(output)) => {
                    let event = match output {
                        Output::Data(buf) => Event::Incoming(RawMessage::new(buf)),
                        Output::Connected(addr) => Event::Connected(addr),
                        Output::Disconnected(addr) => Event::Disconnected(addr),
                    };
                    self.handler.handle_event(event);
                }
                Ok(None) => break,
                Err(e) => {
                    error!("{}: An error occured {:?}", self.network.address(), e);
                    break;
                }
            }
        }
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

    // TODO rewrite it
    // fn interrupted(&mut self, _: &mut EventLoop) {
    //     self.push(Event::Terminate);
    // }

    fn tick(&mut self, event_loop: &mut EventLoop<H>) {
        self.network.tick(event_loop);
    }
}

impl<H: EventHandler> Reactor<H> for Events<H> {
    type Channel = MioChannel<H>;

    fn bind(&mut self) -> ::std::io::Result<()> {
        self.inner.network.bind(&mut self.event_loop)
    }
    fn run(&mut self) -> ::std::io::Result<()> {
        self.event_loop.run(&mut self.inner)
    }
    fn run_once(&mut self, timeout: Option<usize>) -> ::std::io::Result<()> {
        self.event_loop.run_once(&mut self.inner, timeout)
    }
    fn get_time(&self) -> Timespec {
        get_time()
    }
    fn channel(&self) -> MioChannel<H> {
        MioChannel {
            inner: self.event_loop.channel(),
            address: *self.inner.network.address(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{time, thread};
    use std::net::SocketAddr;
    use std::collections::VecDeque;

    use time::{get_time, Duration};
    use test::Bencher;

    use super::{Events, Reactor, Event, InternalEvent, Channel};
    use super::{Network, NetworkConfiguration, EventHandler};

    use ::messages::{MessageWriter, RawMessage};
    use ::crypto::gen_keypair;

    pub type TestEvent = InternalEvent<(), u32>;

    pub struct TestHandler {
        events: VecDeque<TestEvent>,
    }

    pub trait TestPoller {
        fn poll(&mut self) -> Option<TestEvent>;
    }

    impl TestHandler {
        pub fn new() -> TestHandler {
            TestHandler { events: VecDeque::new() }
        }
    }

    impl TestPoller for TestHandler {
        fn poll(&mut self) -> Option<TestEvent> {
            self.events.pop_front()
        }
    }

    impl EventHandler for TestHandler {
        type Timeout = ();
        type ApplicationEvent = ();

        fn handle_event(&mut self, event: Event) {
            self.events.push_back(InternalEvent::Node(event));
        }
        fn handle_timeout(&mut self, _: Self::Timeout) {}
        fn handle_application_event(&mut self, event: Self::ApplicationEvent) {
            self.events.push_back(InternalEvent::Application(event));
        }
    }

    pub struct TestEvents(Events<TestHandler>);

    impl TestEvents {
        fn with_addr(addr: SocketAddr) -> TestEvents {
            let network = Network::with_config(NetworkConfiguration {
                listen_address: addr,
                max_incoming_connections: 128,
                max_outgoing_connections: 128,
                tcp_nodelay: true,
                tcp_keep_alive: None,
                tcp_reconnect_timeout: 1000,
                tcp_reconnect_timeout_max: 600000,
            });
            let handler = TestHandler::new();

            TestEvents(Events::new(network, handler).unwrap())
        }

        fn bind(&mut self) -> ::io::Result<()> {
            self.0.bind()
        }

        fn wait_for_bind(&mut self, addr: &SocketAddr) {
            self.0.bind().unwrap();
            thread::sleep(time::Duration::from_millis(1000));
            self.wait_for_connect(addr);
        }

        fn wait_for_connect(&mut self, addr: &SocketAddr) {
            self.0.channel().connect(addr);

            let start = get_time();
            loop {
                self.0.run_once(Some(100)).unwrap();

                if start + Duration::milliseconds(100) < get_time() {
                    return;
                }
                while let Some(e) = self.0.inner.handler.poll() {
                    match e {
                        InternalEvent::Node(Event::Connected(_)) => return,
                        _ => {}
                    }
                }
            }
        }

        fn wait_for_msg(&mut self, duration: Duration) -> Option<RawMessage> {
            let start = get_time();
            loop {
                self.0.run_once(Some(100)).unwrap();

                if start + duration < get_time() {
                    return None;
                }
                while let Some(e) = self.0.inner.handler.poll() {
                    match e {
                        InternalEvent::Node(Event::Incoming(msg)) => return Some(msg),
                        InternalEvent::Node(Event::Error(_)) => return None,
                        _ => {}
                    }
                }
            }
        }

        fn wait_for_messages(&mut self, usize count) -> Option<()> {
            
        }

        fn process_events(&mut self, duration: Duration) {
            let start = get_time();
            loop {
                self.0.run_once(Some(100)).unwrap();

                if start + duration < get_time() {
                    return;
                }
            }
        }

        fn send_to(&mut self, addr: &SocketAddr, msg: RawMessage) {
            self.0.channel().send_to(addr, msg);
            self.0.run_once(None).unwrap();
        }
    }

    fn gen_message(id: u16, len: usize) -> RawMessage {
        let writer = MessageWriter::new(id, len);
        RawMessage::new(writer.sign(&gen_keypair().1))
    }

    struct BenchConfig {
        times: usize,
        len: usize,
        tcp_nodelay: bool
    }

    fn bench_network(b: &mut Bencher, addrs: [SocketAddr; 2], cfg: BenchConfig) {
        b.iter(|| {
            let mut e1 = TestEvents::with_addr(addrs[0], &cfg);
            let mut e2 = TestEvents::with_addr(addrs[1], &cfg);
            e1.bind().unwrap();
            e2.bind().unwrap();

            let timeout = Duration::seconds(30);
            let len = cfg.len;
            let times = cfg.times;
            let t1 = thread::spawn(move || {
                e1.wait_for_connect(&addrs[1]);
                for _ in 0..times {
                    let msg = Events::<u32>::gen_message(0, len);
                    e1.send_to(&addrs[1], msg);
                    e1.wait_for_messages(1, timeout).unwrap();
                }
                e1.process_events(Duration::milliseconds(0));
                drop(e1);
            });
            let t2 = thread::spawn(move || {
                e2.wait_for_connect(&addrs[0]);
                for _ in 0..times {
                    let msg = Events::<u32>::gen_message(1, len);
                    e2.send_to(&addrs[0], msg);
                    e2.wait_for_messages(1, timeout).unwrap();
                }
                e2.process_events(Duration::milliseconds(0));
                drop(e2);
            });
            t1.join().unwrap();
            t2.join().unwrap();
        })
    }

    #[test]
    fn big_message() {
        let addrs: [SocketAddr; 2] = ["127.0.0.1:7200".parse().unwrap(),
                                      "127.0.0.1:7201".parse().unwrap()];

        let m1 = gen_message(15, 1000000);
        let m2 = gen_message(16, 400);

        let t1;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            t1 = thread::spawn(move || {
                let mut e = TestEvents::with_addr(addrs[0].clone());
                e.wait_for_bind(&addrs[1]);

                e.send_to(&addrs[1], m1);
                assert_eq!(e.wait_for_msg(Duration::milliseconds(1000)), Some(m2));
                e.process_events(Duration::milliseconds(5000));
            });
        }

        let t2;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            t2 = thread::spawn(move || {
                let mut e = TestEvents::with_addr(addrs[1].clone());
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
        let addrs: [SocketAddr; 2] = ["127.0.0.1:9100".parse().unwrap(),
                                      "127.0.0.1:9101".parse().unwrap()];

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
                    let mut e = TestEvents::with_addr(addrs[0].clone());
                    e.wait_for_bind(&addrs[1]);

                    info!("t1: connection opened");
                    info!("t1: send m1 to t2");
                    e.send_to(&addrs[1], m1);
                    info!("t1: wait for m2");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)), Some(m2));
                    info!("t1: received m2 from t2");
                    e.process_events(Duration::milliseconds(500));
                    drop(e);
                }
                info!("t1: connection closed");
                {
                    let mut e = TestEvents::with_addr(addrs[0].clone());
                    e.wait_for_bind(&addrs[1]);

                    info!("t1: connection reopened");
                    info!("t1: send m3 to t2");
                    e.send_to(&addrs[1], m3.clone());
                    info!("t1: wait for m3");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)), Some(m3));
                    e.process_events(Duration::milliseconds(500));
                    info!("t1: received m3 from t2");
                }
                info!("t1: connection closed");
            });
        }

        let t2;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            let m3 = m3.clone();
            t2 = thread::spawn(move || {
                {
                    let mut e = TestEvents::with_addr(addrs[1].clone());
                    e.wait_for_bind(&addrs[0]);

                    info!("t2: connection opened");
                    info!("t2: send m2 to t1");
                    e.send_to(&addrs[0], m2);
                    info!("t2: wait for m1");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)), Some(m1));
                    info!("t2: received m1 from t1");
                    info!("t2: wait for m3");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)),
                               Some(m3.clone()));
                    info!("t2: received m3 from t1");
                    e.process_events(Duration::milliseconds(500));
                    drop(e);
                }
                info!("t2: connection closed");
                {
                    info!("t2: connection reopened");
                    let mut e = TestEvents::with_addr(addrs[1].clone());
                    e.wait_for_bind(&addrs[0]);

                    info!("t2: send m3 to t1");
                    e.send_to(&addrs[0], m3.clone());
                    e.process_events(Duration::milliseconds(500));
                }
                info!("t2: connection closed");
            });
        }

        t2.join().unwrap();
        t1.join().unwrap();
    }

}
