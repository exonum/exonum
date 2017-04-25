use std::collections::{VecDeque, BinaryHeap, HashSet, HashMap};
use std::iter::FromIterator;
use std::cell::{RefCell, Ref};
use std::sync::{Arc, Mutex};
use std::net::{SocketAddr, Ipv4Addr, IpAddr};
use std::ops::Drop;
use std::time::{SystemTime, Duration, UNIX_EPOCH};

use exonum::node::{ValidatorId, NodeHandler, Configuration, NodeTimeout, ExternalMessage,
                   ListenerConfig};
use exonum::node::state::{Round, Height, TxPool};
use exonum::blockchain::{Blockchain, ConsensusConfig, GenesisConfig, Block, StoredConfiguration,
                         Schema, Transaction, Service};
use exonum::storage::{Map, MemoryDB, Error as StorageError, RootProofNode, Fork};
use exonum::messages::{Any, Message, RawMessage, Connect, RawTransaction, BlockProof};
use exonum::events::{Reactor, Event, EventsConfiguration, NetworkConfiguration, InternalEvent,
                     EventHandler, Channel, Result as EventsResult, Milliseconds};
use exonum::crypto::{Hash, PublicKey, SecretKey, Seed, gen_keypair_from_seed};
#[cfg(test)]
use exonum::crypto::gen_keypair;

use timestamping::TimestampingService;

type SandboxEvent = InternalEvent<ExternalMessage, NodeTimeout>;

#[derive(PartialEq, Eq)]
pub struct TimerPair(pub SystemTime, pub NodeTimeout);

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
    pub time: SystemTime,
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
        self.inner
            .lock()
            .unwrap()
            .sended
            .push_back((address.clone(), message));
    }
}

impl Channel for SandboxChannel {
    type ApplicationEvent = ExternalMessage;
    type Timeout = NodeTimeout;

    fn address(&self) -> SocketAddr {
        self.inner.lock().unwrap().address
    }

    fn get_time(&self) -> SystemTime {
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

    fn add_timeout(&mut self, timeout: Self::Timeout, time: SystemTime) {
        let pair = TimerPair(time, timeout);
        self.inner.lock().unwrap().timers.push(pair);
    }
}

pub struct SandboxReactor {
    inner: Arc<Mutex<SandboxInner>>,
    handler: NodeHandler<SandboxChannel>,
}

impl Reactor<NodeHandler<SandboxChannel>> for SandboxReactor {
    type Channel = SandboxChannel;

    fn bind(&mut self) -> ::std::io::Result<()> {
        Ok(())
    }
    fn run(&mut self) -> ::std::io::Result<()> {
        unreachable!();
    }
    fn run_once(&mut self, _: Option<usize>) -> ::std::io::Result<()> {
        loop {
            let result = self.inner.lock().unwrap().events.pop_front();
            if let Some(event) = result {
                match event {
                    InternalEvent::Node(event) => {
                        self.handler.handle_event(event);
                    }
                    InternalEvent::Application(event) => {
                        self.handler.handle_application_event(event);
                    }
                    InternalEvent::Invoke(_) => {}
                }
            } else {
                break;
            }
        }
        Ok(())
    }
    fn get_time(&self) -> SystemTime {
        self.inner.lock().unwrap().time
    }
    fn channel(&self) -> SandboxChannel {
        SandboxChannel { inner: self.inner.clone() }
    }
}

impl SandboxReactor {
    pub fn is_leader(&self) -> bool {
        self.handler.state().is_leader()
    }

    pub fn leader(&self, round: Round) -> ValidatorId {
        self.handler.state().leader(round)
    }

    pub fn is_validator(&self) -> bool {
        self.handler.state().is_validator()
    }

    pub fn last_block(&self) -> Result<Block, StorageError> {
        self.handler.blockchain.last_block()
    }

    pub fn last_hash(&self) -> Result<Hash, StorageError> {
        self.handler.blockchain.last_hash()
    }

    pub fn actual_config(&self) -> Result<StoredConfiguration, StorageError> {
        let view = self.handler.blockchain.view();
        let schema = Schema::new(&view);
        schema.get_actual_configuration()
    }

    pub fn following_config(&self) -> Result<Option<StoredConfiguration>, StorageError> {
        let view = self.handler.blockchain.view();
        let schema = Schema::new(&view);
        schema.get_following_configuration()
    }

    pub fn handle_message(&mut self, msg: RawMessage) {
        let event = Event::Incoming(msg);
        self.handler.handle_event(event);
    }

    pub fn handle_timeout(&mut self, timeout: NodeTimeout) {
        self.handler.handle_timeout(timeout);
    }
}

pub struct Sandbox {
    inner: Arc<Mutex<SandboxInner>>,
    reactor: RefCell<SandboxReactor>,
    // pub validators: Vec<(PublicKey, SecretKey)>,
    pub validators_map: HashMap<PublicKey, SecretKey>,
    addresses: Vec<SocketAddr>,
}

impl Sandbox {
    pub fn initialize(&self,
                      connect_message_time: SystemTime,
                      start_index: usize,
                      end_index: usize) {
        let connect = Connect::new(&self.p(0), self.a(0), connect_message_time, self.s(0));

        for validator in start_index..end_index {
            self.recv(Connect::new(&self.p(validator),
                                   self.a(validator),
                                   self.time(),
                                   self.s(validator)));
            self.send(self.a(validator), connect.clone());
        }

        self.check_unexpected_message()
    }

    pub fn set_validators_map(&mut self,
                              new_addresses_len: u8,
                              validators: Vec<(PublicKey, SecretKey)>) {
        self.addresses = (1..(new_addresses_len + 1) as u8)
            .map(gen_primitive_socket_addr)
            .collect::<Vec<_>>();
        self.validators_map.extend(validators);
    }

    fn check_unexpected_message(&self) {
        let sended = self.inner.lock().unwrap().sended.pop_front();
        if let Some((addr, msg)) = sended {
            let any_msg = Any::from_raw(msg.clone()).expect("Send incorrect message");
            panic!("Send unexpected message {:?} to {}", any_msg, addr);
        }
    }

    pub fn tx_from_raw(&self, raw: RawTransaction) -> Option<Box<Transaction>> {
        let reactor = self.reactor.borrow_mut();
        reactor.handler.blockchain.tx_from_raw(raw)
    }

    pub fn p(&self, id: usize) -> PublicKey {
        self.validators()[id]
        // &self.validators[id].0
    }

    pub fn s(&self, id: usize) -> &SecretKey {
        let p = self.p(id);
        &self.validators_map[&p]
        // &self.validators[id].1
    }

    pub fn a(&self, id: usize) -> SocketAddr {
        self.addresses[id].clone()
    }

    pub fn validators(&self) -> Vec<PublicKey> {
        let conf = self.cfg();
        conf.validators.clone()
    }

    pub fn n_validators(&self) -> usize {
        self.validators().len()
    }

    pub fn time(&self) -> SystemTime {
        self.inner.lock().unwrap().time
    }

    pub fn blockchain_ref(&self) -> Ref<Blockchain> {
        Ref::map(self.reactor.borrow(), |reactor| &reactor.handler.blockchain)
    }

    pub fn recv<T: Message>(&self, msg: T) {
        self.check_unexpected_message();
        let mut reactor = self.reactor.borrow_mut();
        reactor.handle_message(msg.raw().clone());
        reactor.run_once(None).unwrap();
    }

    pub fn send<T: Message>(&self, addr: SocketAddr, msg: T) {
        let any_expected_msg = Any::from_raw(msg.raw().clone()).unwrap();
        let sended = self.inner.lock().unwrap().sended.pop_front();
        if let Some((real_addr, real_msg)) = sended {
            let any_real_msg = Any::from_raw(real_msg.clone()).expect("Send incorrect message");
            if real_addr != addr || any_real_msg != any_expected_msg {
                panic!("Expected to send the message {:?} to {} instead sending {:?} to {}",
                       any_expected_msg,
                       addr,
                       any_real_msg,
                       real_addr)
            }
        } else {
            panic!("Expected to send the message {:?} to {} but nothing happened",
                   any_expected_msg,
                   addr);
        }
    }

    // TODO: add self-test for broadcasting?
    pub fn broadcast<T: Message>(&self, msg: T) {
        let any_expected_msg = Any::from_raw(msg.raw().clone()).unwrap();
        let mut set: HashSet<SocketAddr> =
            HashSet::from_iter(self.addresses.iter().skip(1).cloned());
        for _ in 0..self.n_validators() - 1 {
            let sended = self.inner.lock().unwrap().sended.pop_front();
            if let Some((real_addr, real_msg)) = sended {
                let any_real_msg = Any::from_raw(real_msg.clone()).expect("Send incorrect message");
                if any_real_msg != any_expected_msg {
                    panic!("Expected to broadcast the message {:?} instead sending {:?} to {}",
                           any_expected_msg,
                           any_real_msg,
                           real_addr)
                }
                if !set.contains(&real_addr) {
                    panic!("Double send the same message {:?} to {:?} during broadcasting",
                           any_expected_msg,
                           real_addr)
                } else {
                    set.remove(&real_addr);
                }
            } else {
                panic!("Expected to broadcast the message {:?} but someone don't recieve \
                        messages: {:?}",
                       any_expected_msg,
                       set);
            }
        }
    }

    pub fn add_time(&self, duration: Duration) {
        self.check_unexpected_message();
        let now = {
            let mut inner = self.inner.lock().unwrap();
            inner.time = inner.time + duration;
            inner.time
        };
        // handle timeouts if occurs
        loop {
            let timeout = {
                let ref mut timers = self.inner.lock().unwrap().timers;
                if let Some(TimerPair(time, timeout)) = timers.pop() {
                    if time > now {
                        timers.push(TimerPair(time, timeout));
                        break;
                    } else {
                        timeout
                    }
                } else {
                    break;
                }
            };
            let mut reactor = self.reactor.borrow_mut();
            reactor.handle_timeout(timeout);
            reactor.run_once(None).unwrap();
        }
    }

    pub fn is_leader(&self) -> bool {
        let reactor = self.reactor.borrow();
        reactor.is_leader()
    }

    pub fn leader(&self, round: Round) -> ValidatorId {
        let reactor = self.reactor.borrow();
        reactor.leader(round)
    }

    pub fn is_validator(&self) -> bool {
        let reactor = self.reactor.borrow();
        reactor.is_validator()
    }

    pub fn last_block(&self) -> Block {
        let reactor = self.reactor.borrow();
        reactor.last_block().unwrap()
    }

    pub fn last_hash(&self) -> Hash {
        let reactor = self.reactor.borrow();
        reactor.last_hash().unwrap()
    }

    pub fn last_state_hash(&self) -> Hash {
        let reactor = self.reactor.borrow();
        *reactor.last_block().unwrap().state_hash()
    }

    pub fn filter_present_transactions<'a, I>(&self, txs: I) -> Vec<RawMessage>
        where I: IntoIterator<Item = &'a RawMessage>
    {
        let mut unique_set: HashSet<Hash> = HashSet::new();
        let view = self.reactor.borrow().handler.blockchain.view();
        let schema = Schema::new(&view);
        let schema_transactions = schema.transactions();
        let res: Vec<RawTransaction> = txs.into_iter()
            .filter(|elem| {
                let hash_elem = elem.hash();
                if unique_set.contains(&hash_elem) {
                    return false;
                }
                unique_set.insert(hash_elem);
                if schema_transactions.get(&hash_elem).unwrap().is_some() {
                    return false;
                }
                true
            })
            .map(|elem| elem.clone())
            .collect::<Vec<_>>();
        res
    }
    /// Extract state_hash from fake block
    pub fn compute_state_hash<'a, I>(&self, txs: I) -> Hash
        where I: IntoIterator<Item = &'a RawTransaction>
    {
        let ref blockchain = self.reactor.borrow().handler.blockchain;
        let (hashes, tx_pool) = {
            let mut pool = TxPool::new();
            let mut hashes = Vec::new();
            for raw in txs {
                let tx = blockchain.tx_from_raw(raw.clone()).unwrap();
                let hash = tx.hash();
                hashes.push(hash);
                pool.insert(hash, tx);
            }
            (hashes, pool)
        };

        let view = {
            let db = blockchain.view();
            let (_, patch) = blockchain
                .create_patch(self.current_height(),
                              self.current_round(),
                              &hashes,
                              &tx_pool)
                .unwrap();
            db.merge(&patch);
            db
        };
        Schema::new(&view)
            .last_block()
            .unwrap()
            .unwrap()
            .state_hash()
            .clone()
    }

    pub fn get_proof_to_service_table(&self,
                                      service_id: u16,
                                      table_idx: usize)
                                      -> Result<RootProofNode<Hash>, StorageError> {
        let view = self.reactor.borrow().handler.blockchain.view();
        let schema = Schema::new(&view);
        schema.get_proof_to_service_table(service_id, table_idx)
    }

    pub fn get_configs_root_hash(&self) -> Result<Hash, StorageError> {
        let view = self.reactor.borrow().handler.blockchain.view();
        let schema = Schema::new(&view);
        schema.configs().root_hash()
    }

    pub fn cfg(&self) -> StoredConfiguration {
        let reactor = self.reactor.borrow();
        reactor.actual_config().unwrap()
    }

    pub fn following_cfg(&self) -> Option<StoredConfiguration> {
        let reactor = self.reactor.borrow();
        reactor.following_config().unwrap()
    }

    pub fn propose_timeout(&self) -> Milliseconds {
        self.cfg().consensus.propose_timeout
    }

    pub fn majority_count(&self, num_validators: usize) -> usize {
        num_validators * 2 / 3 + 1
    }

    pub fn round_timeout(&self) -> Milliseconds {
        self.cfg().consensus.round_timeout
    }

    pub fn transactions_hashes(&self) -> Vec<Hash> {
        self.reactor
            .borrow()
            .handler
            .state()
            .transactions()
            .keys()
            .cloned()
            .collect()
    }

    pub fn current_round(&self) -> Round {
        self.reactor.borrow().handler.state().round()
    }

    pub fn block_and_precommits(&self, height: u64) -> Result<Option<BlockProof>, StorageError> {
        let view = self.reactor.borrow().handler.blockchain.view();
        let schema = Schema::new(&view);
        schema.block_and_precommits(height)
    }

    pub fn current_height(&self) -> Height {
        self.reactor.borrow().handler.state().height()
    }

    pub fn current_leader(&self) -> Round {
        self.reactor
            .borrow()
            .handler
            .state()
            .leader(self.current_round())
    }

    pub fn assert_state(&self, height: Height, round: Round) {
        let reactor = self.reactor.borrow();
        let ref state = reactor.handler.state();

        let achual_height = state.height();
        let actual_round = state.round();
        assert!(achual_height == height,
                "Incorrect height, actual={}, expected={}",
                achual_height,
                height);
        assert!(actual_round == round,
                "Incorrect round, actual={}, expected={}",
                actual_round,
                round);
    }

    pub fn assert_lock(&self, round: Round, hash: Option<Hash>) {
        let reactor = self.reactor.borrow();
        let state = reactor.handler.state();

        let actual_round = state.locked_round();
        let actual_hash = state.locked_propose();
        assert!(actual_round == round,
                "Incorrect round, actual={}, expected={}",
                actual_round,
                round);
        assert!(actual_hash == hash,
                "Incorrect hash, actual={:?}, expected={:?}",
                actual_hash,
                hash);
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if !::std::thread::panicking() {
            self.check_unexpected_message();
        }
    }
}

fn gen_primitive_socket_addr(idx: u8) -> SocketAddr {
    let addr = Ipv4Addr::new(idx, idx, idx, idx);
    SocketAddr::new(IpAddr::V4(addr), idx as u16)
}

pub fn sandbox_with_services(services: Vec<Box<Service>>) -> Sandbox {
    let validators = vec![gen_keypair_from_seed(&Seed::new([12; 32])),
                          gen_keypair_from_seed(&Seed::new([13; 32])),
                          gen_keypair_from_seed(&Seed::new([16; 32])),
                          gen_keypair_from_seed(&Seed::new([19; 32]))];
    let addresses: Vec<SocketAddr> = (1..5).map(gen_primitive_socket_addr).collect::<Vec<_>>();

    let db = MemoryDB::new();
    let blockchain = Blockchain::new(db, services);

    let consensus = ConsensusConfig {
        round_timeout: 1000,
        status_timeout: 600000,
        peers_timeout: 600000,
        propose_timeout: 200,
        txs_block_limit: 1000,
    };
    let genesis = GenesisConfig::new_with_consensus(consensus, validators.iter().map(|x| x.0));
    blockchain.create_genesis_block(genesis).unwrap();

    let config = Configuration {
        listener: ListenerConfig {
            address: addresses[0].clone(),
            public_key: validators[0].0.clone(),
            secret_key: validators[0].1.clone(),
        },
        network: NetworkConfiguration::default(),
        events: EventsConfiguration::new(),
        peer_discovery: Vec::new(),
    };

    // TODO use factory or other solution like set_handler or run

    let inner = Arc::new(Mutex::new(SandboxInner {
                                        address: addresses[0].clone(),
                                        time: UNIX_EPOCH + Duration::new(1486720340, 0),
                                        sended: VecDeque::new(),
                                        events: VecDeque::new(),
                                        timers: BinaryHeap::new(),
                                    }));

    let channel = SandboxChannel { inner: inner.clone() };
    let node = NodeHandler::new(blockchain.clone(), channel, config.clone());

    let mut reactor = SandboxReactor {
        inner: inner.clone(),
        handler: node,
    };
    let mut validators_map = HashMap::new();
    validators_map.extend(validators.clone());
    reactor.handler.initialize();
    let sandbox = Sandbox {
        inner: inner.clone(),
        reactor: RefCell::new(reactor),
        validators_map: validators_map,
        addresses: addresses,
    };

    sandbox.initialize(sandbox.time(), 1, validators.len());
    // General assumption; necessary for correct work of consensus algorithm
    assert!(sandbox.propose_timeout() < sandbox.round_timeout());
    sandbox
}

pub fn timestamping_sandbox() -> Sandbox {
    sandbox_with_services(vec![Box::new(TimestampingService::new())])
}

#[test]
fn test_sandbox_init() {
    timestamping_sandbox();
}

#[test]
fn test_sandbox_recv_and_send() {
    let s = timestamping_sandbox();
    let (public, secret) = gen_keypair();
    s.recv(Connect::new(&public, s.a(2), s.time(), &secret));
    s.send(s.a(2), Connect::new(&s.p(0), s.a(0), s.time(), s.s(0)));
}

#[test]
fn test_sandbox_assert_status() {
    // TODO: remove this?
    let s = timestamping_sandbox();
    s.assert_state(1, 1);
    s.add_time(Duration::from_millis(999));
    s.assert_state(1, 1);
    s.add_time(Duration::from_millis(1));
    s.assert_state(1, 2);
}

#[test]
#[should_panic(expected = "Expected to send the message")]
fn test_sandbox_expected_to_send_but_nothing_happened() {
    let s = timestamping_sandbox();
    s.send(s.a(1), Connect::new(&s.p(0), s.a(0), s.time(), s.s(0)));
}

#[test]
#[should_panic(expected = "Expected to send the message")]
fn test_sandbox_expected_to_send_another_message() {
    let s = timestamping_sandbox();
    let (public, secret) = gen_keypair();
    s.recv(Connect::new(&public, s.a(2), s.time(), &secret));
    s.send(s.a(1), Connect::new(&s.p(0), s.a(0), s.time(), s.s(0)));
}

#[test]
#[should_panic(expected = "Send unexpected message")]
fn test_sandbox_unexpected_message_when_drop() {
    let s = timestamping_sandbox();
    let (public, secret) = gen_keypair();
    s.recv(Connect::new(&public, s.a(2), s.time(), &secret));
}

#[test]
#[should_panic(expected = "Send unexpected message")]
fn test_sandbox_unexpected_message_when_handle_another_message() {
    let s = timestamping_sandbox();
    let (public, secret) = gen_keypair();
    s.recv(Connect::new(&public, s.a(2), s.time(), &secret));
    s.recv(Connect::new(&public, s.a(3), s.time(), &secret));
    panic!("Oops! We don't catch unexpected message");
}

#[test]
#[should_panic(expected = "Send unexpected message")]
fn test_sandbox_unexpected_message_when_time_changed() {
    let s = timestamping_sandbox();
    let (public, secret) = gen_keypair();
    s.recv(Connect::new(&public, s.a(2), s.time(), &secret));
    s.add_time(Duration::from_millis(1000));
    panic!("Oops! We don't catch unexpected message");
}
