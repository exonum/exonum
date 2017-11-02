// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


// Workaround: Clippy does not correctly handle borrowing checking rules for returned types.
#![cfg_attr(feature="cargo-clippy", allow(let_and_return))]

use futures::{self, Async, Future, Stream};
use futures::sync::mpsc;

use std::ops::{AddAssign, Deref};
use std::sync::{Arc, Mutex};
use std::cell::{Ref, RefCell, RefMut};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet, VecDeque};
use std::iter::FromIterator;

use exonum::node::{Configuration, ListenerConfig, NodeHandler, ServiceConfig, State,
                   SystemStateProvider, NodeSender};
use exonum::blockchain::{Block, BlockProof, Blockchain, ConsensusConfig, GenesisConfig, Schema,
                         Service, SharedNodeState, StoredConfiguration, TimeoutAdjusterConfig,
                         Transaction, ValidatorKeys};
use exonum::storage::{MapProof, MemoryDB};
use exonum::messages::{Any, Connect, Message, RawMessage, RawTransaction, Status};
use exonum::crypto::{gen_keypair_from_seed, Hash, PublicKey, SecretKey, Seed};
#[cfg(test)]
use exonum::crypto::gen_keypair;
use exonum::helpers::{Height, Milliseconds, Round, ValidatorId};
use exonum::events::{Event, EventHandler, NetworkEvent, NetworkRequest, TimeoutRequest};
use exonum::events::network::NetworkConfiguration;

use timestamping::TimestampingService;
use config_updater::ConfigUpdateService;
use sandbox_tests_helper::VALIDATOR_0;

pub type SharedTime = Arc<Mutex<SystemTime>>;

#[derive(Debug)]
pub struct SandboxSystemStateProvider {
    listen_address: SocketAddr,
    shared_time: SharedTime,
}

impl SystemStateProvider for SandboxSystemStateProvider {
    fn current_time(&self) -> SystemTime {
        *self.shared_time.lock().unwrap()
    }

    fn listen_address(&self) -> SocketAddr {
        self.listen_address
    }
}

#[derive(Debug)]
pub struct SandboxInner {
    pub time: SharedTime,
    pub handler: NodeHandler,
    pub sent: VecDeque<(SocketAddr, RawMessage)>,
    pub events: VecDeque<Event>,
    pub timers: BinaryHeap<TimeoutRequest>,
    pub network_requests_rx: mpsc::Receiver<NetworkRequest>,
    pub timeout_requests_rx: mpsc::Receiver<TimeoutRequest>,
}

impl SandboxInner {
    pub fn process_events(&mut self) {
        self.process_network_requests();
        self.process_timeout_requests();
    }

    pub fn handle_event<E: Into<Event>>(&mut self, e: E) {
        self.handler.handle_event(e.into());
        self.process_events();
    }

    fn process_network_requests(&mut self) {
        let network_getter = futures::lazy(|| -> Result<(), ()> {
            while let Async::Ready(Some(network)) = self.network_requests_rx.poll()? {
                match network {
                    NetworkRequest::SendMessage(peer, msg) => self.sent.push_back((peer, msg)),
                    NetworkRequest::DisconnectWithPeer(_) |
                    NetworkRequest::Shutdown => {}
                }
            }
            Ok(())
        });
        network_getter.wait().unwrap();
    }

    fn process_timeout_requests(&mut self) {
        let timeouts_getter = futures::lazy(|| -> Result<(), ()> {
            while let Async::Ready(Some(timeout)) = self.timeout_requests_rx.poll()? {
                self.timers.push(timeout);
            }
            Ok(())
        });
        timeouts_getter.wait().unwrap();
    }
}

pub struct Sandbox {
    pub validators_map: HashMap<PublicKey, SecretKey>,
    pub services_map: HashMap<PublicKey, SecretKey>,
    inner: RefCell<SandboxInner>,
    addresses: Vec<SocketAddr>,
}

impl Sandbox {
    pub fn initialize(
        &self,
        connect_message_time: SystemTime,
        start_index: usize,
        end_index: usize,
    ) {
        let connect = Connect::new(
            &self.p(VALIDATOR_0),
            self.a(VALIDATOR_0),
            connect_message_time,
            self.s(VALIDATOR_0),
        );

        for validator in start_index..end_index {
            let validator = ValidatorId(validator as u16);
            self.recv(Connect::new(
                &self.p(validator),
                self.a(validator),
                self.time(),
                self.s(validator),
            ));
            self.send(self.a(validator), connect.clone());
        }

        self.check_unexpected_message()
    }

    pub fn set_validators_map(
        &mut self,
        new_addresses_len: u8,
        validators: Vec<(PublicKey, SecretKey)>,
        services: Vec<(PublicKey, SecretKey)>,
    ) {
        self.addresses = (1..(new_addresses_len + 1) as u8)
            .map(gen_primitive_socket_addr)
            .collect::<Vec<_>>();
        self.validators_map.extend(validators);
        self.services_map.extend(services);
    }

    fn check_unexpected_message(&self) {
        if let Some((addr, msg)) = self.inner.borrow_mut().sent.pop_front() {
            let any_msg = Any::from_raw(msg.clone()).expect("Send incorrect message");
            panic!("Send unexpected message {:?} to {}", any_msg, addr);
        }
    }

    pub fn tx_from_raw(&self, raw: RawTransaction) -> Option<Box<Transaction>> {
        self.blockchain_ref().tx_from_raw(raw)
    }

    pub fn p(&self, id: ValidatorId) -> PublicKey {
        self.validators()[id.0 as usize]
    }

    pub fn s(&self, id: ValidatorId) -> &SecretKey {
        let p = self.p(id);
        &self.validators_map[&p]
    }

    pub fn service_public_key(&self, id: ValidatorId) -> PublicKey {
        let id: usize = id.into();
        self.nodes_keys()[id].service_key
    }

    pub fn service_secret_key(&self, id: ValidatorId) -> &SecretKey {
        let public_key = self.service_public_key(id);
        &self.services_map[&public_key]
    }

    pub fn a(&self, id: ValidatorId) -> SocketAddr {
        let id: usize = id.into();
        self.addresses[id]
    }

    pub fn validators(&self) -> Vec<PublicKey> {
        self.cfg()
            .validator_keys
            .iter()
            .map(|x| x.consensus_key)
            .collect()
    }

    pub fn nodes_keys(&self) -> Vec<ValidatorKeys> {
        self.cfg().validator_keys
    }

    pub fn n_validators(&self) -> usize {
        self.validators().len()
    }

    pub fn time(&self) -> SystemTime {
        let inner = self.inner.borrow();
        let time = *inner.time.lock().unwrap().deref();
        time
    }

    pub fn node_handler(&self) -> Ref<NodeHandler> {
        Ref::map(self.inner.borrow(), |inner| &inner.handler)
    }

    pub fn node_handler_mut(&self) -> RefMut<NodeHandler> {
        RefMut::map(self.inner.borrow_mut(), |inner| &mut inner.handler)
    }

    pub fn node_state(&self) -> Ref<State> {
        Ref::map(self.inner.borrow(), |inner| inner.handler.state())
    }

    pub fn blockchain_ref(&self) -> Ref<Blockchain> {
        Ref::map(self.inner.borrow(), |inner| &inner.handler.blockchain)
    }

    pub fn blockchain_mut(&self) -> RefMut<Blockchain> {
        RefMut::map(
            self.inner.borrow_mut(),
            |inner| &mut inner.handler.blockchain,
        )
    }

    pub fn recv<T: Message>(&self, msg: T) {
        self.check_unexpected_message();
        // TODO Think about addresses.
        let dummy_addr = SocketAddr::from(([127, 0, 0, 1], 12_039));
        let event = NetworkEvent::MessageReceived(dummy_addr, msg.raw().clone());
        self.inner.borrow_mut().handle_event(event);
    }

    pub fn send<T: Message>(&self, addr: SocketAddr, msg: T) {
        let any_expected_msg = Any::from_raw(msg.raw().clone()).unwrap();
        let sended = self.inner.borrow_mut().sent.pop_front();
        if let Some((real_addr, real_msg)) = sended {
            let any_real_msg = Any::from_raw(real_msg.clone()).expect("Send incorrect message");
            if real_addr != addr || any_real_msg != any_expected_msg {
                panic!(
                    "Expected to send the message {:?} to {} instead sending {:?} to {}",
                    any_expected_msg,
                    addr,
                    any_real_msg,
                    real_addr
                )
            }
        } else {
            panic!(
                "Expected to send the message {:?} to {} but nothing happened",
                any_expected_msg,
                addr
            );
        }
    }

    pub fn broadcast<T: Message>(&self, msg: T) {
        self.broadcast_to_addrs(msg, self.addresses.iter().skip(1));
    }

    // TODO: add self-test for broadcasting?
    pub fn broadcast_to_addrs<'a, T: Message, I>(&self, msg: T, addresses: I)
    where
        I: IntoIterator<Item = &'a SocketAddr>,
    {
        let any_expected_msg = Any::from_raw(msg.raw().clone()).unwrap();

        // If node is excluded from validators, then it still will broadcast messages.
        // So in that case we should not skip addresses and validators count.
        let mut expected_set: HashSet<_> = HashSet::from_iter(addresses);

        for _ in 0..expected_set.len() {
            let sended = self.inner.borrow_mut().sent.pop_front();
            if let Some((real_addr, real_msg)) = sended {
                let any_real_msg = Any::from_raw(real_msg.clone()).expect("Send incorrect message");
                if any_real_msg != any_expected_msg {
                    panic!(
                        "Expected to broadcast the message {:?} instead sending {:?} to {}",
                        any_expected_msg,
                        any_real_msg,
                        real_addr
                    )
                }
                if !expected_set.contains(&real_addr) {
                    panic!(
                        "Double send the same message {:?} to {:?} during broadcasting",
                        any_expected_msg,
                        real_addr
                    )
                } else {
                    expected_set.remove(&real_addr);
                }
            } else {
                panic!(
                    "Expected to broadcast the message {:?} but someone don't recieve \
                     messages: {:?}",
                    any_expected_msg,
                    expected_set
                );
            }
        }
    }

    pub fn check_broadcast_status(&self, height: Height, block_hash: &Hash) {
        self.broadcast(Status::new(
            &self.node_public_key(),
            height,
            block_hash,
            &self.node_secret_key(),
        ));
    }

    pub fn add_time(&self, duration: Duration) {
        self.check_unexpected_message();
        let now = {
            let inner = self.inner.borrow_mut();
            let mut time = inner.time.lock().unwrap();
            time.add_assign(duration);
            *time.deref()
        };
        // handle timeouts if occurs
        loop {
            let timeout = {
                let timers = &mut self.inner.borrow_mut().timers;
                if let Some(TimeoutRequest(time, timeout)) = timers.pop() {
                    if time > now {
                        timers.push(TimeoutRequest(time, timeout));
                        break;
                    } else {
                        timeout
                    }
                } else {
                    break;
                }
            };
            self.inner.borrow_mut().handle_event(timeout);
        }
    }

    pub fn is_leader(&self) -> bool {
        self.node_state().is_leader()
    }

    pub fn leader(&self, round: Round) -> ValidatorId {
        self.node_state().leader(round)
    }

    pub fn is_validator(&self) -> bool {
        self.node_state().is_validator()
    }

    pub fn last_block(&self) -> Block {
        self.blockchain_ref().last_block()
    }

    pub fn last_hash(&self) -> Hash {
        self.blockchain_ref().last_hash()
    }

    pub fn last_state_hash(&self) -> Hash {
        *self.last_block().state_hash()
    }

    pub fn filter_present_transactions<'a, I>(&self, txs: I) -> Vec<RawMessage>
    where
        I: IntoIterator<Item = &'a RawMessage>,
    {
        let mut unique_set: HashSet<Hash> = HashSet::new();
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
        let schema_transactions = schema.transactions();
        txs.into_iter()
            .filter(|elem| {
                let hash_elem = elem.hash();
                if unique_set.contains(&hash_elem) {
                    return false;
                }
                unique_set.insert(hash_elem);
                if schema_transactions.contains(&hash_elem) {
                    return false;
                }
                true
            })
            .cloned()
            .collect()
    }

    /// Extract state_hash from fake block
    pub fn compute_state_hash<'a, I>(&self, txs: I) -> Hash
    where
        I: IntoIterator<Item = &'a RawTransaction>,
    {
        let blockchain = &self.blockchain_ref();
        let (hashes, tx_pool) = {
            let mut pool = BTreeMap::new();
            let mut hashes = Vec::new();
            for raw in txs {
                let tx = blockchain.tx_from_raw(raw.clone()).unwrap();
                let hash = tx.hash();
                hashes.push(hash);
                pool.insert(hash, tx);
            }
            (hashes, pool)
        };

        let fork = {
            let mut fork = blockchain.fork();
            let (_, patch) =
                blockchain.create_patch(ValidatorId(0), self.current_height(), &hashes, &tx_pool);
            fork.merge(patch);
            fork
        };
        *Schema::new(&fork).last_block().unwrap().state_hash()
    }

    pub fn get_proof_to_service_table(&self, service_id: u16, table_idx: usize) -> MapProof<Hash> {
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
        schema.get_proof_to_service_table(service_id, table_idx)
    }

    pub fn get_configs_root_hash(&self) -> Hash {
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
        schema.configs().root_hash()
    }

    pub fn cfg(&self) -> StoredConfiguration {
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
        schema.actual_configuration()
    }

    pub fn following_cfg(&self) -> Option<StoredConfiguration> {
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
        schema.following_configuration()
    }

    pub fn propose_timeout(&self) -> Milliseconds {
        match self.cfg().consensus.timeout_adjuster {
            TimeoutAdjusterConfig::Constant { timeout } => timeout,
            _ => panic!("Unexpected timeout adjuster config type"),
        }
    }

    pub fn majority_count(&self, num_validators: usize) -> usize {
        num_validators * 2 / 3 + 1
    }

    pub fn round_timeout(&self) -> Milliseconds {
        self.cfg().consensus.round_timeout
    }

    pub fn transactions_hashes(&self) -> Vec<Hash> {
        let node_state = self.node_state();
        let rlock = node_state.transactions().read().expect(
            "Expected read lock",
        );
        rlock.keys().cloned().collect()
    }

    pub fn current_round(&self) -> Round {
        self.node_state().round()
    }

    pub fn block_and_precommits(&self, height: Height) -> Option<BlockProof> {
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
        schema.block_and_precommits(height)
    }

    pub fn current_height(&self) -> Height {
        self.node_state().height()
    }

    pub fn current_leader(&self) -> ValidatorId {
        self.node_state().leader(self.current_round())
    }

    pub fn assert_state(&self, expected_height: Height, expected_round: Round) {
        let state = self.node_state();

        let achual_height = state.height();
        let actual_round = state.round();
        assert_eq!(achual_height, expected_height);
        assert_eq!(actual_round, expected_round);
    }

    pub fn assert_lock(&self, expected_round: Round, expected_hash: Option<Hash>) {
        let state = self.node_state();

        let actual_round = state.locked_round();
        let actual_hash = state.locked_propose();
        assert_eq!(actual_round, expected_round);
        assert_eq!(actual_hash, expected_hash);
    }

    fn node_public_key(&self) -> PublicKey {
        *self.node_state().consensus_public_key()
    }

    fn node_secret_key(&self) -> SecretKey {
        self.node_state().consensus_secret_key().clone()
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
    SocketAddr::new(IpAddr::V4(addr), u16::from(idx))
}

pub fn sandbox_with_services(services: Vec<Box<Service>>) -> Sandbox {
    let validators = vec![
        gen_keypair_from_seed(&Seed::new([12; 32])),
        gen_keypair_from_seed(&Seed::new([13; 32])),
        gen_keypair_from_seed(&Seed::new([16; 32])),
        gen_keypair_from_seed(&Seed::new([19; 32])),
    ];
    let service_keys = vec![
        gen_keypair_from_seed(&Seed::new([20; 32])),
        gen_keypair_from_seed(&Seed::new([21; 32])),
        gen_keypair_from_seed(&Seed::new([22; 32])),
        gen_keypair_from_seed(&Seed::new([23; 32])),
    ];

    let addresses: Vec<SocketAddr> = (1..5).map(gen_primitive_socket_addr).collect::<Vec<_>>();

    let db = Box::new(MemoryDB::new());
    let mut blockchain = Blockchain::new(db, services);

    let consensus = ConsensusConfig {
        round_timeout: 1000,
        status_timeout: 600_000,
        peers_timeout: 600_000,
        txs_block_limit: 1000,
        timeout_adjuster: TimeoutAdjusterConfig::Constant { timeout: 200 },
    };
    let genesis = GenesisConfig::new_with_consensus(
        consensus,
        validators.iter().zip(service_keys.iter()).map(|x| {
            ValidatorKeys {
                consensus_key: (x.0).0,
                service_key: (x.1).0,
            }
        }),
    );
    blockchain.create_genesis_block(genesis).unwrap();

    let config = Configuration {
        listener: ListenerConfig {
            address: addresses[0],
            consensus_public_key: validators[0].0,
            consensus_secret_key: validators[0].1.clone(),
            whitelist: Default::default(),
        },
        service: ServiceConfig {
            service_public_key: service_keys[0].0,
            service_secret_key: service_keys[0].1.clone(),
        },
        network: NetworkConfiguration::default(),
        peer_discovery: Vec::new(),
        mempool: Default::default(),
    };

    // TODO use factory or other solution like set_handler or run
    let system_state = SandboxSystemStateProvider {
        listen_address: addresses[0],
        shared_time: SharedTime::new(Mutex::new(UNIX_EPOCH + Duration::new(1_486_720_340, 0))),
    };
    let shared_time = Arc::clone(&system_state.shared_time);

    let network_channel = mpsc::channel(100);
    let timeout_channel = mpsc::channel(100);
    let node_sender = NodeSender {
        network_requests: network_channel.0.clone(),
        timeout_requests: timeout_channel.0.clone(),
    };

    let mut handler = NodeHandler::new(
        blockchain.clone(),
        addresses[0],
        node_sender,
        Box::new(system_state),
        config.clone(),
        SharedNodeState::new(5000),
    );
    handler.initialize();

    let inner = SandboxInner {
        sent: VecDeque::new(),
        events: VecDeque::new(),
        timers: BinaryHeap::new(),
        timeout_requests_rx: timeout_channel.1,
        network_requests_rx: network_channel.1,
        handler,
        time: shared_time,
    };
    let sandbox = Sandbox {
        inner: RefCell::new(inner),
        validators_map: HashMap::from_iter(validators.clone()),
        services_map: HashMap::from_iter(service_keys),
        addresses: addresses,
    };

    sandbox.initialize(sandbox.time(), 1, validators.len());
    // General assumption; necessary for correct work of consensus algorithm
    assert!(sandbox.propose_timeout() < sandbox.round_timeout());
    sandbox
}

pub fn timestamping_sandbox() -> Sandbox {
    sandbox_with_services(vec![
        Box::new(TimestampingService::new()),
        Box::new(ConfigUpdateService::new()),
    ])
}

#[cfg(test)]
mod tests {
    use sandbox_tests_helper::{VALIDATOR_1, VALIDATOR_2, VALIDATOR_3, HEIGHT_ONE, ROUND_ONE,
                               ROUND_TWO};
    use super::*;

    #[test]
    fn test_sandbox_init() {
        timestamping_sandbox();
    }

    #[test]
    fn test_sandbox_recv_and_send() {
        let s = timestamping_sandbox();
        let (public, secret) = gen_keypair();
        s.recv(Connect::new(&public, s.a(VALIDATOR_2), s.time(), &secret));
        s.send(
            s.a(VALIDATOR_2),
            Connect::new(
                &s.p(VALIDATOR_0),
                s.a(VALIDATOR_0),
                s.time(),
                s.s(VALIDATOR_0),
            ),
        );
    }

    #[test]
    fn test_sandbox_assert_status() {
        // TODO: remove this?
        let s = timestamping_sandbox();
        s.assert_state(HEIGHT_ONE, ROUND_ONE);
        s.add_time(Duration::from_millis(999));
        s.assert_state(HEIGHT_ONE, ROUND_ONE);
        s.add_time(Duration::from_millis(1));
        s.assert_state(HEIGHT_ONE, ROUND_TWO);
    }

    #[test]
    #[should_panic(expected = "Expected to send the message")]
    fn test_sandbox_expected_to_send_but_nothing_happened() {
        let s = timestamping_sandbox();
        s.send(
            s.a(VALIDATOR_1),
            Connect::new(
                &s.p(VALIDATOR_0),
                s.a(VALIDATOR_0),
                s.time(),
                s.s(VALIDATOR_0),
            ),
        );
    }

    #[test]
    #[should_panic(expected = "Expected to send the message")]
    fn test_sandbox_expected_to_send_another_message() {
        let s = timestamping_sandbox();
        let (public, secret) = gen_keypair();
        s.recv(Connect::new(&public, s.a(VALIDATOR_2), s.time(), &secret));
        s.send(
            s.a(VALIDATOR_1),
            Connect::new(
                &s.p(VALIDATOR_0),
                s.a(VALIDATOR_0),
                s.time(),
                s.s(VALIDATOR_0),
            ),
        );
    }

    #[test]
    #[should_panic(expected = "Send unexpected message")]
    fn test_sandbox_unexpected_message_when_drop() {
        let s = timestamping_sandbox();
        let (public, secret) = gen_keypair();
        s.recv(Connect::new(&public, s.a(VALIDATOR_2), s.time(), &secret));
    }

    #[test]
    #[should_panic(expected = "Send unexpected message")]
    fn test_sandbox_unexpected_message_when_handle_another_message() {
        let s = timestamping_sandbox();
        let (public, secret) = gen_keypair();
        s.recv(Connect::new(&public, s.a(VALIDATOR_2), s.time(), &secret));
        s.recv(Connect::new(&public, s.a(VALIDATOR_3), s.time(), &secret));
        panic!("Oops! We don't catch unexpected message");
    }

    #[test]
    #[should_panic(expected = "Send unexpected message")]
    fn test_sandbox_unexpected_message_when_time_changed() {
        let s = timestamping_sandbox();
        let (public, secret) = gen_keypair();
        s.recv(Connect::new(&public, s.a(VALIDATOR_2), s.time(), &secret));
        s.add_time(Duration::from_millis(1000));
        panic!("Oops! We don't catch unexpected message");
    }
}
