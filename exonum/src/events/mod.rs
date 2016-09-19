use std::io;
use std::fmt::Display;
use std::net::SocketAddr;
use time::{get_time, Timespec};

use mio;

use super::messages::RawMessage;

mod network;
mod connection;

pub use self::network::{Network, NetworkConfiguration, PeerId, EventSet};

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

    fn post_event(&self, msg: Self::ApplicationEvent);
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
pub struct MioChannel<E: Send, T: Send> {
    address: SocketAddr,
    inner: mio::Sender<InternalEvent<E, T>>,
}

impl<E: Send, T: Send> MioChannel<E, T> {
    pub fn new(addr: SocketAddr, inner: mio::Sender<InternalEvent<E, T>>) -> MioChannel<E, T> {
        MioChannel {
            address: addr,
            inner: inner,
        }
    }
}

// TODO remove unwrap
impl<E: Send, T: Send> Channel for MioChannel<E, T> {
    type ApplicationEvent = E;
    type Timeout = T;

    fn address(&self) -> SocketAddr {
        self.address
    }

    fn get_time(&self) -> Timespec {
        get_time()
    }

    fn post_event(&self, event: Self::ApplicationEvent) {
        let msg = InternalEvent::Application(event);
        self.inner
            .send(msg)
            .log_error("Unable to post event");
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

    fn add_timeout(&mut self, timeout: Self::Timeout, time: Timespec) {
        self.inner
            .send(InternalEvent::Invoke(Invoke::AddTimeout(timeout, time)))
            .log_error("Unable to add timeout");
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
        self.network
            .connect(event_loop, address)
            .log_error(format!("Unable to connect with {}", address));
    }

    fn handle_add_timeout(&mut self,
                          event_loop: &mut EventLoop<H>,
                          timeout: H::Timeout,
                          time: Timespec) {
        let ms = (time - get_time()).num_milliseconds();
        if ms < 0 {
            self.handler.handle_timeout(timeout);
        } else {
            // TODO: use mio::Timeout
            event_loop.timeout_ms(Timeout::Node(timeout), ms as u64)
                .map(|_| ())
                .map_err(|x| format!("{:?}", x))
                .log_error("Unable to add timeout to event loop");
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

    // TODO rewrite it
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
    fn get_time(&self) -> Timespec {
        get_time()
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

impl<E> LogError for Result<(), E>
    where E: Display
{
    fn log_error<S: AsRef<str>>(self, msg: S) {
        if let Err(error) = self {
            error!("{}, an error occured: {}", msg.as_ref(), error);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::thread;
    use std::net::SocketAddr;
    use std::collections::VecDeque;

    use time::{get_time, Duration};
    use env_logger;

    use super::{Events, Reactor, Event, InternalEvent, Channel};
    use super::{Network, NetworkConfiguration, EventHandler};

    use ::messages::{MessageWriter, RawMessage};
    use ::crypto::gen_keypair;

    pub type TestEvent = InternalEvent<(), u32>;

    pub struct TestHandler {
        events: VecDeque<TestEvent>,
        messages: VecDeque<RawMessage>,
    }

    pub trait TestPoller {
        fn event(&mut self) -> Option<TestEvent>;
        fn message(&mut self) -> Option<RawMessage>;
    }

    impl TestHandler {
        pub fn new() -> TestHandler {
            TestHandler {
                events: VecDeque::new(),
                messages: VecDeque::new(),
            }
        }
    }

    impl TestPoller for TestHandler {
        fn event(&mut self) -> Option<TestEvent> {
            self.events.pop_front()
        }
        fn message(&mut self) -> Option<RawMessage> {
            self.messages.pop_front()
        }
    }

    impl EventHandler for TestHandler {
        type Timeout = ();
        type ApplicationEvent = ();

        fn handle_event(&mut self, event: Event) {
            info!("handle event: {:?}", event);
            match event {
                Event::Incoming(raw) => self.messages.push_back(raw),
                _ => {
                    self.events.push_back(InternalEvent::Node(event));
                }
            }
        }
        fn handle_timeout(&mut self, _: Self::Timeout) {}
        fn handle_application_event(&mut self, event: Self::ApplicationEvent) {
            self.events.push_back(InternalEvent::Application(event));
        }
    }

    pub struct TestEvents(pub Events<TestHandler>);

    impl TestEvents {
        pub fn with_addr(addr: SocketAddr) -> TestEvents {
            let network = Network::with_config(NetworkConfiguration {
                listen_address: addr,
                max_incoming_connections: 128,
                max_outgoing_connections: 128,
                tcp_nodelay: true,
                tcp_keep_alive: Some(1),
                tcp_reconnect_timeout: 1000,
                tcp_reconnect_timeout_max: 600000,
            });
            let handler = TestHandler::new();

            TestEvents(Events::new(network, handler).unwrap())
        }

        pub fn wait_for_bind(&mut self, addr: &SocketAddr) -> Option<()> {
            self.0.bind().unwrap();
            let r = self.wait_for_connect(addr);
            r
        }

        pub fn wait_for_connect(&mut self, addr: &SocketAddr) -> Option<()> {
            self.0.channel().connect(addr);

            let start = get_time();
            loop {
                self.process_events().unwrap();

                if start + Duration::milliseconds(10000) < get_time() {
                    return None;
                }
                while let Some(e) = self.0.inner.handler.event() {
                    match e {
                        InternalEvent::Node(Event::Connected(_)) => return Some(()),
                        _ => {}
                    }
                }
            }
        }

        pub fn wait_for_msg(&mut self, duration: Duration) -> Option<RawMessage> {
            let start = get_time();
            loop {
                self.process_events().unwrap();

                if start + duration < get_time() {
                    return None;
                }

                while let Some(e) = self.0.inner.handler.event() {
                    match e {
                        InternalEvent::Node(Event::Error(e)) => {
                            error!("An error during wait occured {:?}", e);
                        }
                        _ => {}
                    }
                }

                if let Some(msg) = self.0.inner.handler.message() {
                    return Some(msg);
                }
            }
        }

        pub fn wait_for_disconnect(&mut self) -> Option<()> {
            let start = get_time();
            loop {
                self.process_events().unwrap();

                if start + Duration::milliseconds(1000) < get_time() {
                    return None;
                }
                while let Some(e) = self.0.inner.handler.event() {
                    match e {
                        InternalEvent::Node(Event::Disconnected(_)) => return Some(()),
                        _ => {}
                    }
                }
            }
        }

        pub fn send_to(&mut self, addr: &SocketAddr, msg: RawMessage) {
            self.0.channel().send_to(addr, msg);
            self.process_events().unwrap();
        }

        pub fn process_events(&mut self) -> io::Result<()> {
            self.0.run_once(Some(100))
        }
    }

    pub fn gen_message(id: u16, len: usize) -> RawMessage {
        let writer = MessageWriter::new(id, len);
        RawMessage::new(writer.sign(&gen_keypair().1))
    }

    #[test]
    fn big_message() {
        let _ = env_logger::init();
        let addrs: [SocketAddr; 2] = ["127.0.0.1:7200".parse().unwrap(),
                                      "127.0.0.1:7201".parse().unwrap()];

        let m1 = gen_message(15, 1000000);
        let m2 = gen_message(16, 400);

        let mut e1 = TestEvents::with_addr(addrs[0].clone());
        let mut e2 = TestEvents::with_addr(addrs[1].clone());
        e1.0.bind().unwrap();
        e2.0.bind().unwrap();

        let t1;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            t1 = thread::spawn(move || {
                let mut e = e1;
                e.wait_for_connect(&addrs[1]);

                e.send_to(&addrs[1], m1);
                assert_eq!(e.wait_for_msg(Duration::milliseconds(10000)), Some(m2));
                e.wait_for_disconnect().unwrap();
            });
        }

        let t2;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            t2 = thread::spawn(move || {
                let mut e = e2;
                e.wait_for_connect(&addrs[0]);

                e.send_to(&addrs[0], m2);
                assert_eq!(e.wait_for_msg(Duration::milliseconds(10000)), Some(m1));
            });
        }

        t2.join().unwrap();
        t1.join().unwrap();
    }

    #[test]
    fn reconnect() {
        let _ = env_logger::init();
        let addrs: [SocketAddr; 2] = ["127.0.0.1:9100".parse().unwrap(),
                                      "127.0.0.1:9101".parse().unwrap()];

        let m1 = gen_message(15, 250);
        let m2 = gen_message(16, 400);
        let m3 = gen_message(17, 600);

        let mut e1 = TestEvents::with_addr(addrs[0].clone());
        let mut e2 = TestEvents::with_addr(addrs[1].clone());
        e1.0.bind().unwrap();
        e2.0.bind().unwrap();

        let t1;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            let m3 = m3.clone();
            t1 = thread::spawn(move || {
                {
                    let mut e = e1;
                    e.wait_for_connect(&addrs[1]).unwrap();

                    debug!("t1: connection opened");
                    debug!("t1: send m1 to t2");
                    e.send_to(&addrs[1], m1);
                    debug!("t1: wait for m2");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)), Some(m2));
                    debug!("t1: received m2 from t2");
                }
                debug!("t1: connection closed");
                {
                    let mut e = TestEvents::with_addr(addrs[0].clone());
                    e.wait_for_bind(&addrs[1]).unwrap();

                    debug!("t1: connection reopened");
                    debug!("t1: send m3 to t2");
                    e.send_to(&addrs[1], m3.clone());
                    debug!("t1: wait for m3");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)), Some(m3));
                    debug!("t1: received m3 from t2");
                    e.process_events().unwrap();
                }
                debug!("t1: finished");
            });
        }

        let t2;
        {
            let m1 = m1.clone();
            let m2 = m2.clone();
            let m3 = m3.clone();
            t2 = thread::spawn(move || {
                {
                    let mut e = e2;
                    e.wait_for_connect(&addrs[0]).unwrap();

                    debug!("t2: connection opened");
                    debug!("t2: send m2 to t1");
                    e.send_to(&addrs[0], m2);
                    debug!("t2: wait for m1");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)), Some(m1));
                    debug!("t2: received m1 from t1");
                    debug!("t2: wait for m3");
                    assert_eq!(e.wait_for_msg(Duration::milliseconds(5000)),
                               Some(m3.clone()));
                    debug!("t2: received m3 from t1");
                }
                debug!("t2: connection closed");
                {
                    debug!("t2: connection reopened");
                    let mut e = TestEvents::with_addr(addrs[1].clone());
                    e.wait_for_bind(&addrs[0]).unwrap();

                    debug!("t2: send m3 to t1");
                    e.send_to(&addrs[0], m3.clone());
                    e.wait_for_disconnect().unwrap();
                }
                debug!("t2: finished");
            });
        }

        t2.join().unwrap();
        t1.join().unwrap();
    }
}

#[cfg(feature = "long_benchmarks")]
#[cfg(test)]
mod benches {
    use std::thread;
    use std::net::SocketAddr;

    use time::{get_time, Duration};

    use super::{Network, NetworkConfiguration, Events, Reactor};
    use super::tests::{gen_message, TestEvents, TestPoller, TestHandler};

    use test::Bencher;

    struct BenchConfig {
        times: usize,
        len: usize,
        tcp_nodelay: bool,
    }

    impl TestEvents {
        fn with_cfg(cfg: &BenchConfig, addr: SocketAddr) -> TestEvents {
            let network = Network::with_config(NetworkConfiguration {
                listen_address: addr,
                max_incoming_connections: 128,
                max_outgoing_connections: 128,
                tcp_nodelay: cfg.tcp_nodelay,
                tcp_keep_alive: Some(1),
                tcp_reconnect_timeout: 1000,
                tcp_reconnect_timeout_max: 600000,
            });
            let handler = TestHandler::new();

            TestEvents(Events::new(network, handler).unwrap())
        }

        fn wait_for_messages(&mut self,
                             mut count: usize,
                             duration: Duration)
                             -> Result<(), String> {
            let start = get_time();
            loop {
                self.0.run_once(Some(100)).unwrap();

                if start + duration < get_time() {
                    return Err(format!("Timeout exceeded, {} messages is not received", count));
                }

                if let Some(_) = self.0.inner.handler.message() {
                    count = count - 1;
                    if count == 0 {
                        return Ok(());
                    }
                }
            }
        }
    }

    fn bench_network(b: &mut Bencher, addrs: [SocketAddr; 2], cfg: BenchConfig) {
        b.iter(|| {
            let mut e1 = TestEvents::with_cfg(&cfg, addrs[0]);
            let mut e2 = TestEvents::with_cfg(&cfg, addrs[1]);
            e1.0.bind().unwrap();
            e2.0.bind().unwrap();

            let timeout = Duration::seconds(30);
            let len = cfg.len;
            let times = cfg.times;
            let t1 = thread::spawn(move || {
                e1.wait_for_connect(&addrs[1]).unwrap();
                for _ in 0..times {
                    let msg = gen_message(0, len);
                    e1.send_to(&addrs[1], msg);
                    e1.wait_for_messages(1, timeout).unwrap();
                }
                e1.wait_for_disconnect().unwrap();
            });
            let t2 = thread::spawn(move || {
                e2.wait_for_connect(&addrs[0]).unwrap();
                for _ in 0..times {
                    let msg = gen_message(1, len);
                    e2.send_to(&addrs[0], msg);
                    e2.wait_for_messages(1, timeout).unwrap();
                }
            });
            t1.join().unwrap();
            t2.join().unwrap();
        })
    }

    #[bench]
    fn bench_msg_short_100(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100,
            times: 100,
        };
        let addrs = ["127.0.0.1:6990".parse().unwrap(), "127.0.0.1:6991".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_short_1000(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100,
            times: 1000,
        };
        let addrs = ["127.0.0.1:9792".parse().unwrap(), "127.0.0.1:9793".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_short_10000(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100,
            times: 10000,
        };
        let addrs = ["127.0.0.1:9982".parse().unwrap(), "127.0.0.1:9983".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_short_100_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100,
            times: 100,
        };
        let addrs = ["127.0.0.1:4990".parse().unwrap(), "127.0.0.1:4991".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_short_10000_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100,
            times: 10000,
        };
        let addrs = ["127.0.0.1:5990".parse().unwrap(), "127.0.0.1:5991".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_long_10(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100000,
            times: 10,
        };
        let addrs = ["127.0.0.1:9984".parse().unwrap(), "127.0.0.1:9985".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_long_100(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100000,
            times: 100,
        };
        let addrs = ["127.0.0.1:9946".parse().unwrap(), "127.0.0.1:9947".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_long_1000(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: false,
            len: 100000,
            times: 1000,
        };
        let addrs = ["127.0.0.1:9918".parse().unwrap(), "127.0.0.1:9919".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }

    #[bench]
    fn bench_msg_long_1000_nodelay(b: &mut Bencher) {
        let cfg = BenchConfig {
            tcp_nodelay: true,
            len: 100000,
            times: 1000,
        };
        let addrs = ["127.0.0.1:9198".parse().unwrap(), "127.0.0.1:9199".parse().unwrap()];
        bench_network(b, addrs, cfg);
    }
}
