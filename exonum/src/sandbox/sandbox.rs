// Copyright 2018 The Exonum Team
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
#![cfg_attr(feature = "cargo-clippy", allow(let_and_return))]

use futures::{self, sync::mpsc, Async, Future, Sink, Stream};

use std::{
    cell::{Ref, RefCell, RefMut},
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque}, iter::FromIterator,
    net::{IpAddr, Ipv4Addr, SocketAddr}, ops::{AddAssign, Deref}, sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::{
    config_updater::ConfigUpdateService, sandbox_tests_helper::{VALIDATOR_0, PROPOSE_TIMEOUT},
    timestamping::TimestampingService,
};
use blockchain::{
    Block, BlockProof, Blockchain, ConsensusConfig, GenesisConfig, Schema, Service,
    SharedNodeState, StoredConfiguration, Transaction, ValidatorKeys,
};
use crypto::{gen_keypair, gen_keypair_from_seed, Hash, PublicKey, SecretKey, Seed, SEED_LENGTH};
use events::{
    network::NetworkConfiguration, Event, EventHandler, InternalEvent, InternalRequest,
    NetworkEvent, NetworkRequest, TimeoutRequest,
};
use helpers::{user_agent, Height, Milliseconds, Round, ValidatorId};
use messages::{Any, Connect, Message, RawMessage, RawTransaction, Status};
use node::ConnectInfo;
use node::{
    ApiSender, Configuration, ConnectList, ConnectListConfig, ExternalMessage, ListenerConfig,
    NodeHandler, NodeSender, ServiceConfig, State, SystemStateProvider,
};
use storage::{MapProof, MemoryDB};

pub type SharedTime = Arc<Mutex<SystemTime>>;

const INITIAL_TIME_IN_SECS: u64 = 1_486_720_340;

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
    pub internal_requests_rx: mpsc::Receiver<InternalRequest>,
    pub api_requests_rx: mpsc::Receiver<ExternalMessage>,
}

impl SandboxInner {
    pub fn process_events(&mut self) {
        self.process_internal_requests();
        self.process_api_requests();
        self.process_network_requests();
        self.process_internal_requests();
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
                    NetworkRequest::DisconnectWithPeer(_) | NetworkRequest::Shutdown => {}
                }
            }
            Ok(())
        });
        network_getter.wait().unwrap();
    }

    fn process_internal_requests(&mut self) {
        let internal_getter = futures::lazy(|| -> Result<(), ()> {
            while let Async::Ready(Some(internal)) = self.internal_requests_rx.poll()? {
                match internal {
                    InternalRequest::Timeout(t) => self.timers.push(t),
                    InternalRequest::JumpToRound(height, round) => self.handler
                        .handle_event(InternalEvent::JumpToRound(height, round).into()),
                    InternalRequest::Shutdown => unimplemented!(),
                }
            }
            Ok(())
        });
        internal_getter.wait().unwrap();
    }
    fn process_api_requests(&mut self) {
        let api_getter = futures::lazy(|| -> Result<(), ()> {
            while let Async::Ready(Some(api)) = self.api_requests_rx.poll()? {
                self.handler.handle_event(api.into());
            }
            Ok(())
        });
        api_getter.wait().unwrap();
    }
}

pub struct Sandbox {
    pub validators_map: HashMap<PublicKey, SecretKey>,
    pub services_map: HashMap<PublicKey, SecretKey>,
    inner: RefCell<SandboxInner>,
    addresses: Vec<SocketAddr>,
    /// Connect message used during initialization.
    connect: Option<Connect>,
}

impl Sandbox {
    pub fn initialize(
        &mut self,
        connect_message_time: SystemTime,
        start_index: usize,
        end_index: usize,
    ) {
        let connect = Connect::new(
            &self.p(VALIDATOR_0),
            self.a(VALIDATOR_0),
            connect_message_time.into(),
            &user_agent::get(),
            self.s(VALIDATOR_0),
        );

        for validator in start_index..end_index {
            let validator = ValidatorId(validator as u16);
            self.recv(&Connect::new(
                &self.p(validator),
                self.a(validator),
                self.time().into(),
                &user_agent::get(),
                self.s(validator),
            ));
            self.send(self.a(validator), &connect);
        }

        self.check_unexpected_message();
        self.connect = Some(connect);
    }

    fn check_unexpected_message(&self) {
        if let Some((addr, msg)) = self.inner.borrow_mut().sent.pop_front() {
            let any_msg = Any::from_raw(msg.clone()).expect("Send incorrect message");
            panic!("Send unexpected message {:?} to {}", any_msg, addr);
        }
    }

    pub fn p(&self, id: ValidatorId) -> PublicKey {
        self.validators()[id.0 as usize]
    }

    pub fn s(&self, id: ValidatorId) -> &SecretKey {
        let p = self.p(id);
        &self.validators_map[&p]
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

    pub fn n_validators(&self) -> usize {
        self.validators().len()
    }

    pub fn time(&self) -> SystemTime {
        let inner = self.inner.borrow();
        let time = *inner.time.lock().unwrap().deref();
        time
    }

    pub fn set_time(&mut self, new_time: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        *inner.time.lock().unwrap() = new_time;
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
        RefMut::map(self.inner.borrow_mut(), |inner| {
            &mut inner.handler.blockchain
        })
    }

    /// Returns connect message used during initialization.
    pub fn connect(&self) -> Option<&Connect> {
        self.connect.as_ref()
    }

    pub fn recv<T: Message>(&self, msg: &T) {
        self.check_unexpected_message();
        // TODO Think about addresses. (ECR-1627)
        let dummy_addr = SocketAddr::from(([127, 0, 0, 1], 12_039));
        let event = NetworkEvent::MessageReceived(dummy_addr, msg.raw().clone());
        self.inner.borrow_mut().handle_event(event);
    }

    pub fn process_events(&self) {
        self.inner.borrow_mut().process_events();
    }

    pub fn send<T: Message>(&self, addr: SocketAddr, msg: &T) {
        self.process_events();
        let any_expected_msg = Any::from_raw(msg.raw().clone()).unwrap();
        let send = self.inner.borrow_mut().sent.pop_front();
        if let Some((real_addr, real_msg)) = send {
            let any_real_msg = Any::from_raw(real_msg.clone()).expect("Send incorrect message");
            if real_addr != addr || any_real_msg != any_expected_msg {
                panic!(
                    "Expected to send the message {:?} to {} instead sending {:?} to {}",
                    any_expected_msg, addr, any_real_msg, real_addr
                )
            }
        } else {
            panic!(
                "Expected to send the message {:?} to {} but nothing happened",
                any_expected_msg, addr
            );
        }
    }

    pub fn broadcast<T: Message>(&self, msg: &T) {
        self.broadcast_to_addrs(msg, self.addresses.iter().skip(1));
    }

    pub fn try_broadcast<T: Message>(&self, msg: &T) -> Result<(), String> {
        self.try_broadcast_to_addrs(msg, self.addresses.iter().skip(1))
    }

    // TODO: Add self-test for broadcasting? (ECR-1627)
    pub fn broadcast_to_addrs<'a, T: Message, I>(&self, msg: &T, addresses: I)
    where
        I: IntoIterator<Item = &'a SocketAddr>,
    {
        self.try_broadcast_to_addrs(msg, addresses).unwrap();
    }

    // TODO: Add self-test for broadcasting? (ECR-1627)
    pub fn try_broadcast_to_addrs<'a, T: Message, I>(
        &self,
        msg: &T,
        addresses: I,
    ) -> Result<(), String>
    where
        I: IntoIterator<Item = &'a SocketAddr>,
    {
        let any_expected_msg = Any::from_raw(msg.raw().clone()).unwrap();

        // If node is excluded from validators, then it still will broadcast messages.
        // So in that case we should not skip addresses and validators count.
        let mut expected_set: HashSet<_> = HashSet::from_iter(addresses);

        for _ in 0..expected_set.len() {
            let send = self.inner.borrow_mut().sent.pop_front();
            if let Some((real_addr, real_msg)) = send {
                let any_real_msg = Any::from_raw(real_msg.clone()).expect("Send incorrect message");
                if any_real_msg != any_expected_msg {
                    return Err(format!(
                        "Expected to broadcast the message {:?} instead sending {:?} to {}",
                        any_expected_msg, any_real_msg, real_addr
                    ));
                }
                if !expected_set.contains(&real_addr) {
                    panic!(
                        "Double send the same message {:?} to {:?} during broadcasting",
                        any_expected_msg, real_addr
                    )
                } else {
                    expected_set.remove(&real_addr);
                }
            } else {
                panic!(
                    "Expected to broadcast the message {:?} but someone don't receive \
                     messages: {:?}",
                    any_expected_msg, expected_set
                );
            }
        }
        Ok(())
    }

    pub fn check_broadcast_status(&self, height: Height, block_hash: &Hash) {
        self.broadcast(&Status::new(
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

    /// Extracts state_hash from the fake block.
    pub fn compute_state_hash<'a, I>(&self, txs: I) -> Hash
    where
        I: IntoIterator<Item = &'a RawTransaction>,
    {
        let height = self.current_height();
        let mut blockchain = self.blockchain_mut();
        let (hashes, recover, patch) = {
            let mut hashes = Vec::new();
            let mut recover = BTreeSet::new();
            let mut fork = blockchain.fork();
            {
                let mut schema = Schema::new(&mut fork);
                for raw in txs {
                    let hash = raw.hash();
                    hashes.push(hash);
                    if schema.transactions().get(&hash).is_none() {
                        recover.insert(hash);
                        schema.add_transaction_into_pool(raw.clone());
                    }
                }
            }

            (hashes, recover, fork.into_patch())
        };
        blockchain.merge(patch).unwrap();

        let fork = {
            let mut fork = blockchain.fork();
            let (_, patch) = blockchain.create_patch(ValidatorId(0), height, &hashes);
            fork.merge(patch);
            fork
        };
        let patch = {
            let mut fork = blockchain.fork();
            {
                let mut schema = Schema::new(&mut fork);
                for hash in recover {
                    schema.reject_transaction(&hash).unwrap();
                }
            }
            fork.into_patch()
        };

        blockchain.merge(patch).unwrap();
        *Schema::new(&fork).last_block().state_hash()
    }

    pub fn get_proof_to_service_table(
        &self,
        service_id: u16,
        table_idx: usize,
    ) -> MapProof<Hash, Hash> {
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
        schema.get_proof_to_service_table(service_id, table_idx)
    }

    pub fn get_configs_merkle_root(&self) -> Hash {
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
        schema.configs().merkle_root()
    }

    pub fn cfg(&self) -> StoredConfiguration {
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
        schema.actual_configuration()
    }

    pub fn majority_count(&self, num_validators: usize) -> usize {
        num_validators * 2 / 3 + 1
    }

    pub fn round_timeout(&self) -> Milliseconds {
        self.cfg().consensus.round_timeout
    }

    pub fn transactions_hashes(&self) -> Vec<Hash> {
        let schema = Schema::new(self.blockchain_ref().snapshot());
        let idx = schema.transactions_pool();
        let vec = idx.iter().collect();
        vec
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

        let actual_height = state.height();
        let actual_round = state.round();
        assert_eq!(actual_height, expected_height);
        assert_eq!(actual_round, expected_round);
    }

    pub fn assert_lock(&self, expected_round: Round, expected_hash: Option<Hash>) {
        let state = self.node_state();

        let actual_round = state.locked_round();
        let actual_hash = state.locked_propose();
        assert_eq!(actual_round, expected_round);
        assert_eq!(actual_hash, expected_hash);
    }

    /// Creates new sandbox with "restarted" node.
    pub fn restart(self) -> Self {
        self.restart_with_time(UNIX_EPOCH + Duration::new(INITIAL_TIME_IN_SECS, 0))
    }

    /// Creates new sandbox with "restarted" node initialized by the given time.
    pub fn restart_with_time(self, time: SystemTime) -> Self {
        let connect = self.connect().map(|c| {
            Connect::new(
                c.pub_key(),
                c.addr(),
                time.into(),
                c.user_agent(),
                self.s(VALIDATOR_0),
            )
        });
        let sandbox = self.restart_uninitialized_with_time(time);
        if let Some(connect) = connect {
            sandbox.broadcast(&connect);
        }

        sandbox
    }

    /// Constructs a new uninitialized instance of a `Sandbox` preserving database and
    /// configuration.
    pub fn restart_uninitialized(self) -> Sandbox {
        self.restart_uninitialized_with_time(UNIX_EPOCH + Duration::new(INITIAL_TIME_IN_SECS, 0))
    }

    /// Constructs a new uninitialized instance of a `Sandbox` preserving database and
    /// configuration.
    pub fn restart_uninitialized_with_time(self, time: SystemTime) -> Sandbox {
        let network_channel = mpsc::channel(100);
        let internal_channel = mpsc::channel(100);
        let api_channel = mpsc::channel(100);

        let address = self.a(VALIDATOR_0);
        let inner = self.inner.borrow();

        let blockchain = inner
            .handler
            .blockchain
            .clone_with_api_sender(ApiSender::new(api_channel.0.clone()));

        let node_sender = NodeSender {
            network_requests: network_channel.0.clone().wait(),
            internal_requests: internal_channel.0.clone().wait(),
            api_requests: api_channel.0.clone().wait(),
        };

        let connect_list = ConnectList::from_peers(inner.handler.state.peers());

        let config = Configuration {
            listener: ListenerConfig {
                address,
                consensus_public_key: *inner.handler.state.consensus_public_key(),
                consensus_secret_key: inner.handler.state.consensus_secret_key().clone(),
                connect_list,
            },
            service: ServiceConfig {
                service_public_key: *inner.handler.state.service_public_key(),
                service_secret_key: inner.handler.state.service_secret_key().clone(),
            },
            network: NetworkConfiguration::default(),
            peer_discovery: Vec::new(),
            mempool: Default::default(),
        };

        let system_state = SandboxSystemStateProvider {
            listen_address: address,
            shared_time: SharedTime::new(Mutex::new(time)),
        };

        let mut handler = NodeHandler::new(
            blockchain,
            address,
            node_sender,
            Box::new(system_state),
            config,
            inner.handler.api_state.clone(),
            None,
        );
        handler.initialize();

        let inner = SandboxInner {
            sent: VecDeque::new(),
            events: VecDeque::new(),
            timers: BinaryHeap::new(),
            internal_requests_rx: internal_channel.1,
            network_requests_rx: network_channel.1,
            api_requests_rx: api_channel.1,
            handler,
            time: Arc::clone(&inner.time),
        };
        let sandbox = Sandbox {
            inner: RefCell::new(inner),
            validators_map: self.validators_map.clone(),
            services_map: self.services_map.clone(),
            addresses: self.addresses.clone(),
            connect: None,
        };
        sandbox.process_events();
        sandbox
    }

    fn node_public_key(&self) -> PublicKey {
        *self.node_state().consensus_public_key()
    }

    fn node_secret_key(&self) -> SecretKey {
        self.node_state().consensus_secret_key().clone()
    }

    fn add_peer_to_connect_list(&self, addr: SocketAddr, validator_keys: ValidatorKeys) {
        let public_key = validator_keys.consensus_key;
        let config = {
            let inner = &self.inner.borrow_mut();
            let state = &inner.handler.state;
            let mut config = state.config().clone();
            config.validator_keys.push(validator_keys);
            config
        };

        self.update_config(config);
        self.inner
            .borrow_mut()
            .handler
            .state
            .add_peer_to_connect_list(ConnectInfo {
                address: addr,
                public_key,
            });
    }

    fn update_config(&self, config: StoredConfiguration) {
        self.inner.borrow_mut().handler.state.update_config(config);
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if !::std::thread::panicking() {
            self.check_unexpected_message();
        }
    }
}

impl ConnectList {
    /// Helper method to populate ConnectList after sandbox node restarts and
    /// we have access only to peers stored in `node::state`.
    #[doc(hidden)]
    pub fn from_peers(peers: &HashMap<PublicKey, Connect>) -> Self {
        let peers: BTreeMap<PublicKey, SocketAddr> =
            peers.iter().map(|(p, c)| (*p, c.addr())).collect();
        ConnectList {
            peers,
            x25519_keys: Vec::new(),
        }
    }
}

fn gen_primitive_socket_addr(idx: u8) -> SocketAddr {
    let addr = Ipv4Addr::new(idx, idx, idx, idx);
    SocketAddr::new(IpAddr::V4(addr), u16::from(idx))
}

/// Constructs an instance of a `Sandbox` and initializes connections.
pub fn sandbox_with_services(services: Vec<Box<dyn Service>>) -> Sandbox {
    let mut sandbox = sandbox_with_services_uninitialized(services);
    let time = sandbox.time();
    let validators_count = sandbox.validators_map.len();
    sandbox.initialize(time, 1, validators_count);
    sandbox
}

/// Constructs an uninitialized instance of a `Sandbox`.
pub fn sandbox_with_services_uninitialized(services: Vec<Box<dyn Service>>) -> Sandbox {
    let validators = vec![
        gen_keypair_from_seed(&Seed::new([12; SEED_LENGTH])),
        gen_keypair_from_seed(&Seed::new([13; SEED_LENGTH])),
        gen_keypair_from_seed(&Seed::new([16; SEED_LENGTH])),
        gen_keypair_from_seed(&Seed::new([19; SEED_LENGTH])),
    ];
    let service_keys = vec![
        gen_keypair_from_seed(&Seed::new([20; SEED_LENGTH])),
        gen_keypair_from_seed(&Seed::new([21; SEED_LENGTH])),
        gen_keypair_from_seed(&Seed::new([22; SEED_LENGTH])),
        gen_keypair_from_seed(&Seed::new([23; SEED_LENGTH])),
    ];

    let addresses: Vec<SocketAddr> = (1..5).map(gen_primitive_socket_addr).collect::<Vec<_>>();

    let api_channel = mpsc::channel(100);
    let db = MemoryDB::new();
    let mut blockchain = Blockchain::new(
        db,
        services,
        service_keys[0].0,
        service_keys[0].1.clone(),
        ApiSender::new(api_channel.0.clone()),
    );

    let consensus = ConsensusConfig {
        round_timeout: 1000,
        status_timeout: 600_000,
        peers_timeout: 600_000,
        txs_block_limit: 1000,
        max_message_len: 1024 * 1024,
        min_propose_timeout: PROPOSE_TIMEOUT,
        max_propose_timeout: PROPOSE_TIMEOUT,
        propose_timeout_threshold: 0,
    };
    let genesis = GenesisConfig::new_with_consensus(
        consensus,
        validators
            .iter()
            .zip(service_keys.iter())
            .map(|x| ValidatorKeys {
                consensus_key: (x.0).0,
                service_key: (x.1).0,
            }),
    );

    let connect_list_config =
        ConnectListConfig::from_validator_keys(&genesis.validator_keys, &addresses);

    blockchain.initialize(genesis).unwrap();

    let config = Configuration {
        listener: ListenerConfig {
            address: addresses[0],
            consensus_public_key: validators[0].0,
            consensus_secret_key: validators[0].1.clone(),
            connect_list: ConnectList::from_config(connect_list_config),
        },
        service: ServiceConfig {
            service_public_key: service_keys[0].0,
            service_secret_key: service_keys[0].1.clone(),
        },
        network: NetworkConfiguration::default(),
        peer_discovery: Vec::new(),
        mempool: Default::default(),
    };

    // TODO: Use factory or other solution like set_handler or run. (ECR-1627)
    let system_state = SandboxSystemStateProvider {
        listen_address: addresses[0],
        shared_time: SharedTime::new(Mutex::new(
            UNIX_EPOCH + Duration::new(INITIAL_TIME_IN_SECS, 0),
        )),
    };
    let shared_time = Arc::clone(&system_state.shared_time);

    let network_channel = mpsc::channel(100);
    let internal_channel = mpsc::channel(100);
    let node_sender = NodeSender {
        network_requests: network_channel.0.clone().wait(),
        internal_requests: internal_channel.0.clone().wait(),
        api_requests: api_channel.0.clone().wait(),
    };

    let mut handler = NodeHandler::new(
        blockchain.clone(),
        addresses[0],
        node_sender,
        Box::new(system_state),
        config.clone(),
        SharedNodeState::new(5000),
        None,
    );
    handler.initialize();

    let inner = SandboxInner {
        sent: VecDeque::new(),
        events: VecDeque::new(),
        timers: BinaryHeap::new(),
        network_requests_rx: network_channel.1,
        api_requests_rx: api_channel.1,
        internal_requests_rx: internal_channel.1,
        handler,
        time: shared_time,
    };
    let sandbox = Sandbox {
        inner: RefCell::new(inner),
        validators_map: HashMap::from_iter(validators.clone()),
        services_map: HashMap::from_iter(service_keys),
        addresses,
        connect: None,
    };

    // General assumption; necessary for correct work of consensus algorithm
    assert!(PROPOSE_TIMEOUT < sandbox.round_timeout());
    sandbox.process_events();
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
    use super::*;
    use blockchain::{ExecutionResult, ServiceContext, TransactionSet};
    use crypto::{gen_keypair_from_seed, Seed, SEED_LENGTH};
    use encoding;
    use messages::RawTransaction;
    use sandbox::sandbox_tests_helper::{
        add_one_height, SandboxState, VALIDATOR_1, VALIDATOR_2, VALIDATOR_3, HEIGHT_ONE, ROUND_ONE,
        ROUND_TWO,
    };
    use storage::{Fork, Snapshot};

    const SERVICE_ID: u16 = 1;

    transactions! {
        HandleCommitTransactions {
            const SERVICE_ID = SERVICE_ID;

            struct TxAfterCommit {
                height: Height,
            }
        }
    }

    impl TxAfterCommit {
        pub fn new_with_height(height: Height) -> TxAfterCommit {
            let keypair = gen_keypair_from_seed(&Seed::new([22; SEED_LENGTH]));
            TxAfterCommit::new(height, &keypair.1)
        }
    }

    impl Transaction for TxAfterCommit {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, _: &mut Fork) -> ExecutionResult {
            Ok(())
        }
    }

    struct AfterCommitService;

    impl Service for AfterCommitService {
        fn service_name(&self) -> &str {
            "after_commit"
        }

        fn service_id(&self) -> u16 {
            SERVICE_ID
        }

        fn state_hash(&self, _: &dyn Snapshot) -> Vec<Hash> {
            Vec::new()
        }

        fn tx_from_raw(
            &self,
            raw: RawTransaction,
        ) -> Result<Box<dyn Transaction>, encoding::Error> {
            let tx = HandleCommitTransactions::tx_from_raw(raw)?;
            Ok(tx.into())
        }

        fn after_commit(&self, context: &ServiceContext) {
            let tx = TxAfterCommit::new_with_height(context.height());
            context.transaction_sender().send(Box::new(tx)).unwrap();
        }
    }

    #[test]
    fn test_sandbox_init() {
        timestamping_sandbox();
    }

    #[test]
    fn test_sandbox_recv_and_send() {
        let s = timestamping_sandbox();
        // As far as all validators have connected to each other during
        // sandbox initialization, we need to use connect-message with unknown
        // keypair.
        let (public, secret) = gen_keypair();
        let (service, _) = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: public,
            service_key: service,
        };
        // We also need to add public key from this keypair to the ConnectList.
        // Socket address doesn't matter in this case.
        s.add_peer_to_connect_list(gen_primitive_socket_addr(1), validator_keys);

        s.recv(&Connect::new(
            &public,
            s.a(VALIDATOR_2),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
        s.send(
            s.a(VALIDATOR_2),
            &Connect::new(
                &s.p(VALIDATOR_0),
                s.a(VALIDATOR_0),
                s.time().into(),
                &user_agent::get(),
                s.s(VALIDATOR_0),
            ),
        );
    }

    #[test]
    fn test_sandbox_assert_status() {
        // TODO: Remove this? (ECR-1627)
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
            &Connect::new(
                &s.p(VALIDATOR_0),
                s.a(VALIDATOR_0),
                s.time().into(),
                &user_agent::get(),
                s.s(VALIDATOR_0),
            ),
        );
    }

    #[test]
    #[should_panic(expected = "Expected to send the message")]
    fn test_sandbox_expected_to_send_another_message() {
        let s = timestamping_sandbox();
        // See comments to `test_sandbox_recv_and_send`.
        let (public, secret) = gen_keypair();
        let (service, _) = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: public,
            service_key: service,
        };
        s.add_peer_to_connect_list(gen_primitive_socket_addr(1), validator_keys);
        s.recv(&Connect::new(
            &public,
            s.a(VALIDATOR_2),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
        s.send(
            s.a(VALIDATOR_1),
            &Connect::new(
                &s.p(VALIDATOR_0),
                s.a(VALIDATOR_0),
                s.time().into(),
                &user_agent::get(),
                s.s(VALIDATOR_0),
            ),
        );
    }

    #[test]
    #[should_panic(expected = "Send unexpected message")]
    fn test_sandbox_unexpected_message_when_drop() {
        let s = timestamping_sandbox();
        // See comments to `test_sandbox_recv_and_send`.
        let (public, secret) = gen_keypair();
        let (service, _) = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: public,
            service_key: service,
        };
        s.add_peer_to_connect_list(gen_primitive_socket_addr(1), validator_keys);
        s.recv(&Connect::new(
            &public,
            s.a(VALIDATOR_2),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
    }

    #[test]
    #[should_panic(expected = "Send unexpected message")]
    fn test_sandbox_unexpected_message_when_handle_another_message() {
        let s = timestamping_sandbox();
        // See comments to `test_sandbox_recv_and_send`.
        let (public, secret) = gen_keypair();
        let (service, _) = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: public,
            service_key: service,
        };
        s.add_peer_to_connect_list(gen_primitive_socket_addr(1), validator_keys);
        s.recv(&Connect::new(
            &public,
            s.a(VALIDATOR_2),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
        s.recv(&Connect::new(
            &public,
            s.a(VALIDATOR_3),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
        panic!("Oops! We don't catch unexpected message");
    }

    #[test]
    #[should_panic(expected = "Send unexpected message")]
    fn test_sandbox_unexpected_message_when_time_changed() {
        let s = timestamping_sandbox();
        // See comments to `test_sandbox_recv_and_send`.
        let (public, secret) = gen_keypair();
        let (service, _) = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: public,
            service_key: service,
        };
        s.add_peer_to_connect_list(gen_primitive_socket_addr(1), validator_keys);
        s.recv(&Connect::new(
            &public,
            s.a(VALIDATOR_2),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
        s.add_time(Duration::from_millis(1000));
        panic!("Oops! We don't catch unexpected message");
    }

    #[test]
    fn test_sandbox_service_after_commit() {
        let sandbox = sandbox_with_services(vec![
            Box::new(AfterCommitService),
            Box::new(TimestampingService::new()),
        ]);
        let state = SandboxState::new();
        add_one_height(&sandbox, &state);
        let tx = TxAfterCommit::new_with_height(Height(1));
        sandbox.broadcast(&tx);
    }
}
