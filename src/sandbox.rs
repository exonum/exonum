use std::collections::{VecDeque, BinaryHeap};
use std::cell::RefCell;
use std::sync::Arc;
use std::io;
use std::net::SocketAddr;
use std::ops::Drop;

use time::Timespec;

use super::node::{Node, NodeContext, State};
use super::storage::{Storage, MemoryDB};
use super::messages::{Any, Message, RawMessage, Connect};
use super::events::{Reactor, Event, Timeout};
use super::network::{PeerId, EventSet};
use super::tx_generator::TxGenerator;
use super::crypto::{hash, Hash, Seed, PublicKey, SecretKey, gen_keypair_from_seed};

struct SandboxInner {
    address: SocketAddr,
    time: Timespec,
    sended: VecDeque<(SocketAddr, RawMessage)>,
    timers: BinaryHeap<(Timespec, Timeout)>,
}

pub struct Sandbox {
    inner: Arc<RefCell<SandboxInner>>,
    node: RefCell<Node>,
    validators: Vec<(PublicKey, SecretKey)>,
    addresses: Vec<SocketAddr>,
}

pub struct SandboxReactor {
    inner: Arc<RefCell<SandboxInner>>
}

impl Sandbox {
    pub fn new() -> Sandbox {
        let validators = vec![
            gen_keypair_from_seed(&Seed::from_slice(&vec![0; 32]).unwrap()),
            gen_keypair_from_seed(&Seed::from_slice(&vec![1; 32]).unwrap()),
            gen_keypair_from_seed(&Seed::from_slice(&vec![2; 32]).unwrap()),
            gen_keypair_from_seed(&Seed::from_slice(&vec![3; 32]).unwrap()),
        ];

        let addresses = vec![
            "1.1.1.1:1".parse().unwrap(),
            "2.2.2.2:2".parse().unwrap(),
            "3.3.3.3:3".parse().unwrap(),
            "4.4.4.4:4".parse().unwrap(),
        ] : Vec<SocketAddr>;

        let inner = Arc::new(RefCell::new(SandboxInner {
            address: addresses[0].clone(),
            time: Timespec { sec: 0, nsec: 0},
            sended: VecDeque::new(),
            timers: BinaryHeap::new(),
        }));

        let state = State::new(0, validators.iter().map(|&(p, _)| p.clone()).collect());

        let context = NodeContext {
            public_key: validators[0].0.clone(),
            secret_key: validators[0].1.clone(),
            state: state,
            events: Box::new(SandboxReactor {
                inner: inner.clone(),
            }) as Box<Reactor>,
            storage: Storage::new(MemoryDB::new()),
            round_timeout: 1000,
            peer_discovery: Vec::new(),
            tx_generator: TxGenerator::new(),
        };

        let node = Node::with_context(context);

        let sandbox = Sandbox {
            inner: inner,
            node: RefCell::new(node),
            validators: validators,
            addresses: addresses,
        };

        sandbox.initialize();

        return sandbox;
    }
}

impl Reactor for SandboxReactor {
    fn get_time(&self) -> Timespec {
        self.inner.borrow().time
    }

    fn poll(&mut self) -> Event {
        unreachable!();
    }

    fn io(&mut self, _: PeerId, _: EventSet) -> io::Result<()> {
        unreachable!();
    }

    fn bind(&mut self) -> ::std::io::Result<()> {
        Ok(())
    }

    fn send_to(&mut self,
                   address: &SocketAddr,
                   message: RawMessage) -> io::Result<()> {
        self.inner.borrow_mut().sended.push_back((address.clone(), message));
        Ok(())
    }

    fn address(&self) -> SocketAddr {
        self.inner.borrow().address
    }

    fn add_timeout(&mut self, timeout: Timeout, time: Timespec) {
        // assert!(time < self.inner.borrow().time, "Tring to add timeout for the past");
        self.inner.borrow_mut().timers.push((time, timeout));
    }
}

impl Sandbox {
    fn initialize(&self) {
        self.node.borrow_mut().initialize();

        let connect = Connect::new(self.p(0), self.a(0), self.time(), self.s(0));

        self.recv(Connect::new(self.p(1), self.a(1), self.time(), self.s(1)));
        self.send(self.a(1), connect.clone());

        self.recv(Connect::new(self.p(2), self.a(2), self.time(), self.s(2)));
        self.send(self.a(2), connect.clone());

        self.recv(Connect::new(self.p(3), self.a(3), self.time(), self.s(3)));
        self.send(self.a(3), connect.clone());

        self.check_unexpected_message()
    }

    fn check_unexpected_message(&self) {
        let sended = self.inner.borrow_mut().sended.pop_front();
        if let Some((addr, msg)) = sended {
            let any_msg = Any::from_raw(msg.clone())
                              .expect("Send incorrect message");
            panic!("Send unexpected message");
            // panic!("Send unexpected message {:?} to {}", any_msg, addr);
        }
    }

    pub fn p(&self, id: usize) -> &PublicKey {
        &self.validators[id].0
    }

    pub fn s(&self, id: usize) -> &SecretKey {
        &self.validators[id].1
    }

    pub fn a(&self, id: usize) -> SocketAddr {
        self.addresses[id].clone()
    }

    pub fn time(&self) -> Timespec {
        self.inner.borrow().time
    }

    pub fn last_hash(&self) -> Hash {
        // FIXME: temporary hack
        hash(&[])
    }

    pub fn recv<T: Message>(&self, msg: T) {
        self.check_unexpected_message();
        self.node.borrow_mut().handle_message(msg.raw().clone());
    }

    pub fn send<T: Message>(&self, addr: SocketAddr, msg: T) {
        let any_expected_msg = Any::from_raw(msg.raw().clone()).unwrap();
        let sended = self.inner.borrow_mut().sended.pop_front();
        if let Some((real_addr, real_msg)) = sended {
            let any_real_msg = Any::from_raw(real_msg.clone())
                                    .expect("Send incorrect message");
            if real_addr != addr || any_real_msg != any_expected_msg {
                panic!("Expected to send the message {:?} to {} instead sending {:?} to {}",
                       any_expected_msg, addr, any_real_msg, real_addr)
            }
        } else {
            panic!("Expected to send the message {:?} to {} but nothing happened",
                   any_expected_msg, addr);
        }
    }

    pub fn set_time(&self, sec: i64, nsec: i32) {
        self.check_unexpected_message();
        // set time
        let now = Timespec {sec: sec, nsec: nsec};
        self.inner.borrow_mut().time = now;
        // handle timeouts if occurs
        loop {
            let timeout = {
                let ref mut timers = self.inner.borrow_mut().timers;
                if let Some((time, timeout)) = timers.pop() {
                    if time > now {
                        timers.push((time, timeout));
                        break;
                    } else {
                        timeout
                    }
                } else {
                    break;
                }
            };
            self.node.borrow_mut().handle_timeout(timeout);
        }
    }

    pub fn assert_round(&self, round: u32) {
        let actual_round = self.node.borrow().context().state.round();
        assert!(actual_round == round,
                "Incorrect round, actual={}, expected={}", actual_round, round);
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        self.check_unexpected_message();
    }
}

#[test]
fn test_sandbox_init() {
    Sandbox::new();
}

#[test]
fn test_sandbox_recv_and_send() {
    let s = Sandbox::new();
    let (public, secret) = gen_keypair_from_seed(&Seed::from_slice(&vec![17; 32]).unwrap());
    s.recv(Connect::new(&public, s.a(2), s.time(), &secret));
    s.send(s.a(2), Connect::new(s.p(0), s.a(0), s.time(), s.s(0)));
}

#[test]
fn test_sandbox_assert_round() {
    let s = Sandbox::new();
    s.assert_round(1);
    s.set_time(0, 999_999_999);
    s.assert_round(1);
    s.set_time(1, 0);
    s.assert_round(2);
}

#[test]
#[should_panic(expected = "Expected to send the message")]
fn test_sandbox_expected_to_send_but_nothing_happened() {
    let s = Sandbox::new();
    s.send(s.a(1), Connect::new(s.p(0), s.a(0), s.time(), s.s(0)));
}

#[test]
#[should_panic(expected = "Expected to send the message")]
fn test_sandbox_expected_to_send_another_message() {
    let s = Sandbox::new();
    let (public, secret) = gen_keypair_from_seed(&Seed::from_slice(&vec![17; 32]).unwrap());
    s.recv(Connect::new(&public, s.a(2), s.time(), &secret));
    s.send(s.a(1), Connect::new(s.p(0), s.a(0), s.time(), s.s(0)));
}

#[test]
#[should_panic(expected = "Send unexpected message")]
fn test_sandbox_unexpected_message_when_drop() {
    let s = Sandbox::new();
    let (public, secret) = gen_keypair_from_seed(&Seed::from_slice(&vec![17; 32]).unwrap());
    s.recv(Connect::new(&public, s.a(2), s.time(), &secret));
}

#[test]
#[should_panic(expected = "Send unexpected message")]
fn test_sandbox_unexpected_message_when_handle_another_message() {
    let s = Sandbox::new();
    let (public, secret) = gen_keypair_from_seed(&Seed::from_slice(&vec![17; 32]).unwrap());
    s.recv(Connect::new(&public, s.a(2), s.time(), &secret));
    s.recv(Connect::new(&public, s.a(3), s.time(), &secret));
    panic!("Oops! We don't catch unexpected message");
}

#[test]
#[should_panic(expected = "Send unexpected message")]
fn test_sandbox_unexpected_message_when_time_changed() {
    let s = Sandbox::new();
    let (public, secret) = gen_keypair_from_seed(&Seed::from_slice(&vec![17; 32]).unwrap());
    s.recv(Connect::new(&public, s.a(2), s.time(), &secret));
    s.set_time(1, 0);
    panic!("Oops! We don't catch unexpected message");
}
