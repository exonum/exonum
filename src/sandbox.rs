use std::collections::{VecDeque, BinaryHeap};
use std::cell::RefCell;
use std::sync::Arc;
use std::io;
use std::net::SocketAddr;

use time::Timespec;

use super::messages::{Message, RawMessage};
use super::events::{Reactor, Event, Timeout};
use super::network::{PeerId, EventSet};
use super::crypto::{hash, Hash, Seed, PublicKey, SecretKey, gen_keypair_from_seed};

// TODO: Add Debug implementation for all messages type
// TODO: Check that send queue is empty when drop sandbox

struct SandboxInner {
    address: SocketAddr,
    time: Timespec,
    sended: VecDeque<(SocketAddr, RawMessage)>,
    timers: BinaryHeap<(Timespec, Timeout)>,
}

pub struct Sandbox {
    inner: Arc<RefCell<SandboxInner>>,
    validators: Vec<(PublicKey, SecretKey)>,
}

pub struct SandboxReactor {
    inner: Arc<RefCell<SandboxInner>>
}

impl Sandbox {
    pub fn new() -> Sandbox {
        let inner = SandboxInner {
            address: "127.0.0.1:7000".parse().unwrap(),
            time: Timespec { sec: 0, nsec: 0},
            sended: VecDeque::new(),
            timers: BinaryHeap::new(),
        };
        Sandbox {
            inner: Arc::new(RefCell::new(inner)),
            validators: vec![
                gen_keypair_from_seed(&Seed::from_slice(&vec![0; 32]).unwrap()),
                gen_keypair_from_seed(&Seed::from_slice(&vec![1; 32]).unwrap()),
                gen_keypair_from_seed(&Seed::from_slice(&vec![2; 32]).unwrap()),
                gen_keypair_from_seed(&Seed::from_slice(&vec![3; 32]).unwrap()),
            ],
        }
    }
}

impl Reactor for SandboxReactor {
    fn get_time(&self) -> Timespec {
        self.inner.borrow().time
    }

    fn poll(&mut self) -> Event {
        unreachable!();
    }

    fn io(&mut self, id: PeerId, set: EventSet) -> io::Result<()> {
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
        // TODO: check that time > current time
        self.inner.borrow_mut().timers.push((time, timeout));
    }
}

impl Sandbox {
    pub fn v0(&self) -> &SecretKey {
        &self.validators[0].1
    }

    pub fn v1(&self) -> &SecretKey {
        &self.validators[1].1
    }

    pub fn time(&self) -> Timespec {
        self.inner.borrow().time
    }

    pub fn last_hash(&self) -> Hash {
        // FIXME: temporary hack
        hash(&[])
    }

    pub fn send<T: Message>(&self, msg: T) {
        // self.inner.borrow_mut()
    }

    pub fn recv<T: Message>(&self, msg: T) {

    }

    pub fn set_time(&mut self, sec: i64, nsec: i32) {
        self.inner.borrow_mut().time = Timespec {sec: sec, nsec: nsec};
    }
}
