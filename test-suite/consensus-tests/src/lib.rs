// Copyright 2019 The Exonum Team
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

#![warn(missing_debug_implementations, unsafe_code, bare_trait_objects)]

pub mod config_updater;
pub mod sandbox_tests_helper;
pub mod timestamping;

mod guarded_queue;
mod proto;

use bit_vec::BitVec;
use exonum::{
    api::node::SharedNodeState,
    blockchain::{
        contains_transaction, Block, BlockProof, Blockchain, BlockchainBuilder, BlockchainMut,
        ConsensusConfig, InstanceCollection, Schema, ValidatorKeys,
    },
    crypto::{gen_keypair_from_seed, Hash, PublicKey, SecretKey, Seed, SEED_LENGTH},
    events::{
        network::NetworkConfiguration, Event, EventHandler, InternalEvent, InternalRequest,
        NetworkEvent, NetworkRequest, TimeoutRequest,
    },
    helpers::{
        create_rust_runtime_and_genesis_config, user_agent, Height, Milliseconds, Round,
        ValidatorId,
    },
    messages::{
        AnyTx, BlockRequest, BlockResponse, Connect, ExonumMessage, Message, PeersRequest,
        PoolTransactionsRequest, Precommit, Prevote, PrevotesRequest, Propose, ProposeRequest,
        SignedMessage, Status, TransactionsRequest, TransactionsResponse, Verified,
    },
    node::{
        ApiSender, Configuration, ConnectInfo, ConnectList, ConnectListConfig, ExternalMessage,
        ListenerConfig, NodeHandler, NodeSender, ServiceConfig, State, SystemStateProvider,
    },
    runtime::{ForkExt, SnapshotExt},
};
use exonum_keys::Keys;
use exonum_merkledb::{
    BinaryValue, Fork, MapProof, ObjectHash, Snapshot, SystemSchema, TemporaryDB,
};
use futures::{sync::mpsc, Async, Future, Sink, Stream};

use std::{
    cell::{Ref, RefCell, RefMut},
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque},
    convert::TryFrom,
    fmt::Debug,
    iter::FromIterator,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::{AddAssign, Deref, DerefMut},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
    config_updater::ConfigUpdaterService,
    guarded_queue::GuardedQueue,
    sandbox_tests_helper::{BlockBuilder, PROPOSE_TIMEOUT},
    timestamping::TimestampingService,
};

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
    pub sent: GuardedQueue,
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
                    NetworkRequest::SendMessage(peer, msg) => {
                        let msg = Message::from_signed(msg).expect("Expected valid message.");
                        self.sent.push_back((peer, msg))
                    }
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

                    InternalRequest::JumpToRound(height, round) => self
                        .handler
                        .handle_event(InternalEvent::JumpToRound(height, round).into()),

                    InternalRequest::VerifyMessage(raw) => {
                        let msg = SignedMessage::from_bytes(raw.into())
                            .and_then(SignedMessage::into_verified::<ExonumMessage>)
                            .map(Message::from)
                            .unwrap();

                        self.handler
                            .handle_event(InternalEvent::MessageVerified(Box::new(msg)).into())
                    }

                    InternalRequest::Shutdown => unreachable!(),
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

#[derive(Debug)]
pub struct Sandbox {
    pub validators_map: HashMap<PublicKey, SecretKey>,
    pub services_map: HashMap<PublicKey, SecretKey>,
    inner: RefCell<SandboxInner>,
    addresses: Vec<ConnectInfo>,
    /// Connect message used during initialization.
    connect: Option<Verified<Connect>>,
}

impl Sandbox {
    pub fn initialize(
        &mut self,
        connect_message_time: SystemTime,
        start_index: usize,
        end_index: usize,
    ) {
        let connect = self.create_connect(
            &self.public_key(ValidatorId(0)),
            self.address(ValidatorId(0)),
            connect_message_time.into(),
            &user_agent::get(),
            self.secret_key(ValidatorId(0)),
        );

        for validator in start_index..end_index {
            let validator = ValidatorId(validator as u16);
            self.recv(&self.create_connect(
                &self.public_key(validator),
                self.address(validator),
                self.time().into(),
                &user_agent::get(),
                self.secret_key(validator),
            ));
            self.send(self.public_key(validator), &connect);
        }

        self.check_unexpected_message();
        self.connect = Some(connect);
    }

    fn check_unexpected_message(&self) {
        if let Some((addr, msg)) = self.pop_sent_message() {
            panic!("Sent unexpected message {:?} to {}", msg, addr);
        }
    }

    pub fn public_key(&self, id: ValidatorId) -> PublicKey {
        self.validators()[id.0 as usize]
    }

    pub fn secret_key(&self, id: ValidatorId) -> &SecretKey {
        let p = self.public_key(id);
        &self.validators_map[&p]
    }

    pub fn address(&self, id: ValidatorId) -> String {
        let id: usize = id.into();
        self.addresses[id].address.clone()
    }

    /// Creates a `BlockRequest` message signed by this validator.
    pub fn create_block_request(
        &self,
        author: PublicKey,
        to: PublicKey,
        height: Height,
        secret_key: &SecretKey,
    ) -> Verified<BlockRequest> {
        Verified::from_value(BlockRequest::new(to, height), author, secret_key)
    }

    /// Creates a `Status` message signed by this validator.
    pub fn create_status(
        &self,
        author: PublicKey,
        height: Height,
        last_hash: Hash,
        pool_size: u64,
        secret_key: &SecretKey,
    ) -> Verified<Status> {
        Verified::from_value(
            Status::new(height, last_hash, pool_size),
            author,
            secret_key,
        )
    }

    /// Creates a `BlockResponse` message signed by this validator.
    pub fn create_block_response(
        &self,
        public_key: PublicKey,
        to: PublicKey,
        block: Block,
        precommits: impl IntoIterator<Item = Verified<Precommit>>,
        tx_hashes: impl IntoIterator<Item = Hash>,
        secret_key: &SecretKey,
    ) -> Verified<BlockResponse> {
        Verified::from_value(
            BlockResponse::new(
                to,
                block,
                precommits.into_iter().map(Verified::into_bytes),
                tx_hashes,
            ),
            public_key,
            secret_key,
        )
    }

    /// Creates a `Connect` message signed by this validator.
    pub fn create_connect(
        &self,
        public_key: &PublicKey,
        addr: String,
        time: chrono::DateTime<::chrono::Utc>,
        user_agent: &str,
        secret_key: &SecretKey,
    ) -> Verified<Connect> {
        Verified::from_value(
            Connect::new(&addr, time, user_agent),
            *public_key,
            secret_key,
        )
    }

    /// Creates a `PeersRequest` message signed by this validator.
    pub fn create_peers_request(
        &self,
        public_key: PublicKey,
        to: PublicKey,
        secret_key: &SecretKey,
    ) -> Verified<PeersRequest> {
        Verified::from_value(PeersRequest::new(to), public_key, secret_key)
    }

    /// Creates a `PoolTransactionsRequest` message signed by this validator.
    pub fn create_pool_transactions_request(
        &self,
        public_key: PublicKey,
        to: PublicKey,
        secret_key: &SecretKey,
    ) -> Verified<PoolTransactionsRequest> {
        Verified::from_value(PoolTransactionsRequest::new(to), public_key, secret_key)
    }

    /// Creates a `Propose` message signed by this validator.
    pub fn create_propose(
        &self,
        validator_id: ValidatorId,
        height: Height,
        round: Round,
        last_hash: Hash,
        tx_hashes: impl IntoIterator<Item = Hash>,
        secret_key: &SecretKey,
    ) -> Verified<Propose> {
        Verified::from_value(
            Propose::new(validator_id, height, round, last_hash, tx_hashes),
            self.public_key(validator_id),
            secret_key,
        )
    }

    /// Creates a `Precommit` message signed by this validator.
    #[allow(clippy::too_many_arguments)]
    pub fn create_precommit(
        &self,
        validator_id: ValidatorId,
        propose_height: Height,
        propose_round: Round,
        propose_hash: Hash,
        block_hash: Hash,
        system_time: chrono::DateTime<chrono::Utc>,
        secret_key: &SecretKey,
    ) -> Verified<Precommit> {
        Verified::from_value(
            Precommit::new(
                validator_id,
                propose_height,
                propose_round,
                propose_hash,
                block_hash,
                system_time,
            ),
            self.public_key(validator_id),
            secret_key,
        )
    }

    /// Creates a `Precommit` message signed by this validator.
    pub fn create_prevote(
        &self,
        validator_id: ValidatorId,
        propose_height: Height,
        propose_round: Round,
        propose_hash: Hash,
        locked_round: Round,
        secret_key: &SecretKey,
    ) -> Verified<Prevote> {
        Verified::from_value(
            Prevote::new(
                validator_id,
                propose_height,
                propose_round,
                propose_hash,
                locked_round,
            ),
            self.public_key(validator_id),
            secret_key,
        )
    }

    /// Creates a `PrevoteRequest` message signed by this validator.
    #[allow(clippy::too_many_arguments)]
    pub fn create_prevote_request(
        &self,
        from: PublicKey,
        to: PublicKey,
        height: Height,
        round: Round,
        propose_hash: Hash,
        validators: BitVec,
        secret_key: &SecretKey,
    ) -> Verified<PrevotesRequest> {
        Verified::from_value(
            PrevotesRequest::new(to, height, round, propose_hash, validators),
            from,
            secret_key,
        )
    }

    /// Creates a `ProposeRequest` message signed by this validator.
    pub fn create_propose_request(
        &self,
        author: PublicKey,
        to: PublicKey,
        height: Height,
        propose_hash: Hash,
        secret_key: &SecretKey,
    ) -> Verified<ProposeRequest> {
        Verified::from_value(
            ProposeRequest::new(to, height, propose_hash),
            author,
            secret_key,
        )
    }

    /// Creates a `TransactionsRequest` message signed by this validator.
    pub fn create_transactions_request(
        &self,
        author: PublicKey,
        to: PublicKey,
        txs: impl IntoIterator<Item = Hash>,
        secret_key: &SecretKey,
    ) -> Verified<TransactionsRequest> {
        Verified::from_value(TransactionsRequest::new(to, txs), author, secret_key)
    }

    /// Creates a `TransactionsResponse` message signed by this validator.
    pub fn create_transactions_response(
        &self,
        author: PublicKey,
        to: PublicKey,
        txs: impl IntoIterator<Item = Verified<AnyTx>>,
        secret_key: &SecretKey,
    ) -> Verified<TransactionsResponse> {
        Verified::from_value(
            TransactionsResponse::new(to, txs.into_iter().map(Verified::into_bytes)),
            author,
            secret_key,
        )
    }

    pub fn validators(&self) -> Vec<PublicKey> {
        self.cfg()
            .validator_keys
            .iter()
            .map(|x| x.consensus_key)
            .collect()
    }

    #[allow(clippy::let_and_return)]
    pub fn time(&self) -> SystemTime {
        let inner = self.inner.borrow();
        let time = *inner.time.lock().unwrap().deref();
        time
    }

    pub fn set_time(&mut self, new_time: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        *inner.time.lock().unwrap() = new_time;
    }

    pub fn node_handler_mut(&self) -> RefMut<'_, NodeHandler> {
        RefMut::map(self.inner.borrow_mut(), |inner| &mut inner.handler)
    }

    pub fn node_state(&self) -> Ref<'_, State> {
        Ref::map(self.inner.borrow(), |inner| inner.handler.state())
    }

    pub fn blockchain(&self) -> Blockchain {
        self.inner.borrow().handler.blockchain.as_ref().clone()
    }

    pub fn blockchain_mut<'s>(&'s self) -> impl DerefMut<Target = BlockchainMut> + 's {
        RefMut::map(self.inner.borrow_mut(), |inner| {
            &mut inner.handler.blockchain
        })
    }

    /// Returns connect message used during initialization.
    pub fn connect(&self) -> Option<&Verified<Connect>> {
        self.connect.as_ref()
    }

    pub fn recv<T: TryFrom<SignedMessage>>(&self, msg: &Verified<T>) {
        self.check_unexpected_message();
        let event = NetworkEvent::MessageReceived(msg.as_raw().to_bytes());
        self.inner.borrow_mut().handle_event(event);
    }

    pub fn process_events(&self) {
        self.inner.borrow_mut().process_events();
    }

    pub fn pop_sent_message(&self) -> Option<(PublicKey, Message)> {
        self.inner.borrow_mut().sent.pop_front()
    }

    pub fn send<T>(&self, key: PublicKey, expected_msg: &Verified<T>)
    where
        T: TryFrom<SignedMessage> + Debug,
    {
        self.process_events();
        if let Some((real_addr, real_msg)) = self.pop_sent_message() {
            assert_eq!(
                expected_msg.as_raw(),
                real_msg.as_raw(),
                "Expected to send other message"
            );
            assert_eq!(
                key, real_addr,
                "Expected to send message to other recipient"
            );
        } else {
            panic!(
                "Expected to send the message {:?} to {} but nothing happened",
                expected_msg, key
            );
        }
    }

    pub fn send_peers_request(&self) {
        self.process_events();

        if let Some((addr, msg)) = self.pop_sent_message() {
            let peers_request = Verified::<PeersRequest>::try_from(msg)
                .expect("Incorrect message. PeersRequest was expected");

            let id = self.addresses.iter().position(|ref a| a.public_key == addr);
            if let Some(id) = id {
                assert_eq!(
                    &self.public_key(ValidatorId(id as u16)),
                    peers_request.payload().to()
                );
            } else {
                panic!("Sending PeersRequest to unknown peer {:?}", addr);
            }
        } else {
            panic!("Expected to send the PeersRequest message but nothing happened");
        }
    }

    pub fn broadcast<T: TryFrom<SignedMessage>>(&self, msg: &Verified<T>) {
        self.broadcast_to_addrs(msg, self.addresses.iter().map(|i| &i.public_key).skip(1));
    }

    pub fn try_broadcast<T: TryFrom<SignedMessage>>(
        &self,
        msg: &Verified<T>,
    ) -> Result<(), String> {
        self.try_broadcast_to_addrs(msg, self.addresses.iter().map(|i| &i.public_key).skip(1))
    }

    pub fn broadcast_to_addrs<'a, T: TryFrom<SignedMessage>, I>(
        &self,
        msg: &Verified<T>,
        addresses: I,
    ) where
        I: IntoIterator<Item = &'a PublicKey>,
    {
        self.try_broadcast_to_addrs(msg, addresses).unwrap();
    }

    pub fn try_broadcast_to_addrs<'a, T: TryFrom<SignedMessage>, I>(
        &self,
        msg: &Verified<T>,
        addresses: I,
    ) -> Result<(), String>
    where
        I: IntoIterator<Item = &'a PublicKey>,
    {
        let expected_msg = msg.as_raw();

        // If node is excluded from validators, then it still will broadcast messages.
        // So in that case we should not skip addresses and validators count.
        let mut expected_set: HashSet<_> = HashSet::from_iter(addresses);

        for _ in 0..expected_set.len() {
            if let Some((real_addr, real_msg)) = self.pop_sent_message() {
                assert_eq!(
                    expected_msg,
                    real_msg.as_raw(),
                    "Expected to broadcast other message"
                );
                if !expected_set.contains(&real_addr) {
                    panic!(
                        "Double send the same message {:?} to {:?} during broadcasting",
                        expected_msg, real_addr
                    )
                } else {
                    expected_set.remove(&real_addr);
                }
            } else {
                panic!(
                    "Expected to broadcast the message {:?} but someone don't receive \
                     messages: {:?}",
                    expected_msg, expected_set
                );
            }
        }
        Ok(())
    }

    pub fn check_broadcast_status(&self, height: Height, block_hash: Hash) {
        self.broadcast(&self.create_status(
            self.node_public_key(),
            height,
            block_hash,
            0,
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
        self.blockchain().last_block()
    }

    pub fn last_hash(&self) -> Hash {
        self.blockchain().last_hash()
    }

    pub fn last_state_hash(&self) -> Hash {
        self.last_block().state_hash
    }

    pub fn filter_present_transactions<'a, I>(&self, txs: I) -> Vec<Verified<AnyTx>>
    where
        I: IntoIterator<Item = &'a Verified<AnyTx>>,
    {
        let mut unique_set: HashSet<Hash> = HashSet::new();
        let snapshot = self.blockchain().snapshot();
        let schema = snapshot.for_core();
        let schema_transactions = schema.transactions();
        txs.into_iter()
            .filter(|elem| {
                let hash_elem = elem.object_hash();
                if unique_set.contains(&hash_elem) {
                    return false;
                }
                unique_set.insert(hash_elem);
                if contains_transaction(
                    &hash_elem,
                    &schema_transactions,
                    self.node_state().tx_cache(),
                ) {
                    return false;
                }
                true
            })
            .cloned()
            .collect()
    }

    /// Extracts `state_hash` and `error_hash` from the fake block.
    ///
    /// **NB.** This method does not correctly process transactions that mutate the `Dispatcher`,
    /// e.g., starting new services.
    pub fn compute_block_hashes(&self, txs: &[Verified<AnyTx>]) -> (Hash, Hash) {
        let height = self.current_height();
        let mut blockchain = self.blockchain_mut();

        let mut hashes = vec![];
        let mut recover = BTreeSet::new();
        let fork = blockchain.fork();
        let mut schema = fork.for_core_writeable();
        for raw in txs {
            let hash = raw.object_hash();
            hashes.push(hash);
            if schema.transactions().get(&hash).is_none() {
                recover.insert(hash);
                schema.add_transaction_into_pool(raw.clone());
            }
        }
        blockchain.merge(fork.into_patch()).unwrap();

        let (_, patch) =
            blockchain.create_patch(ValidatorId(0).into(), height, &hashes, &mut BTreeMap::new());

        let fork = blockchain.fork();
        let mut schema = fork.for_core_writeable();
        for hash in recover {
            reject_transaction(&mut schema, &hash).unwrap();
        }
        blockchain.merge(fork.into_patch()).unwrap();

        let block = (&patch as &dyn Snapshot).for_core().last_block();
        (block.state_hash, block.error_hash)
    }

    pub fn create_block(&self, txs: &[Verified<AnyTx>]) -> Block {
        let tx_hashes: Vec<_> = txs.iter().map(ObjectHash::object_hash).collect();
        let (state_hash, error_hash) = self.compute_block_hashes(txs);
        BlockBuilder::new(self)
            .with_txs_hashes(&tx_hashes)
            .with_state_hash(&state_hash)
            .with_error_hash(&error_hash)
            .build()
    }

    pub fn get_proof_to_index(&self, index_name: &str) -> MapProof<String, Hash> {
        let snapshot = self.blockchain().snapshot();
        SystemSchema::new(&snapshot)
            .state_aggregator()
            .get_proof(index_name.to_owned())
    }

    pub fn get_configs_merkle_root(&self) -> Hash {
        let snapshot = self.blockchain().snapshot();
        let schema = snapshot.for_core();
        schema.consensus_config().object_hash()
    }

    pub fn cfg(&self) -> ConsensusConfig {
        let snapshot = self.blockchain().snapshot();
        let schema = snapshot.for_core();
        schema.consensus_config()
    }

    pub fn majority_count(&self, num_validators: usize) -> usize {
        num_validators * 2 / 3 + 1
    }

    pub fn first_round_timeout(&self) -> Milliseconds {
        self.cfg().first_round_timeout
    }

    pub fn round_timeout_increase(&self) -> Milliseconds {
        (self.cfg().first_round_timeout * ConsensusConfig::TIMEOUT_LINEAR_INCREASE_PERCENT) / 100
    }

    pub fn current_round_timeout(&self) -> Milliseconds {
        let previous_round: u64 = self.current_round().previous().into();
        self.first_round_timeout() + previous_round * self.round_timeout_increase()
    }

    pub fn transactions_hashes(&self) -> Vec<Hash> {
        let snapshot = self.blockchain().snapshot();
        let schema = snapshot.for_core();
        let idx = schema.transactions_pool();

        let mut vec: Vec<Hash> = idx.iter().collect();
        vec.extend(self.node_state().tx_cache().keys().cloned());
        vec
    }

    pub fn current_round(&self) -> Round {
        self.node_state().round()
    }

    pub fn block_and_precommits(&self, height: Height) -> Option<BlockProof> {
        let snapshot = self.blockchain().snapshot();
        let schema = snapshot.for_core();
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

    pub fn assert_pool_len(&self, expected: u64) {
        let snapshot = self.blockchain().snapshot();
        let schema = snapshot.for_core();
        assert_eq!(expected, schema.transactions_pool_len());
    }

    pub fn assert_tx_cache_len(&self, expected: u64) {
        assert_eq!(expected, self.node_state().tx_cache_len() as u64);
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
            self.create_connect(
                &c.author(),
                c.payload().host.parse().expect("Expected resolved address"),
                time.into(),
                c.payload().user_agent(),
                self.secret_key(ValidatorId(0)),
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

        let address: SocketAddr = self
            .address(ValidatorId(0))
            .parse()
            .expect("Failed to parse socket address");
        let inner = self.inner.into_inner();

        let node_sender = NodeSender {
            network_requests: network_channel.0.clone().wait(),
            internal_requests: internal_channel.0.clone().wait(),
            api_requests: api_channel.0.clone().wait(),
        };
        let connect_list = ConnectList::from_peers(
            inner
                .handler
                .state
                .peers()
                .iter()
                .map(|(public_key, connect)| (*public_key, connect.clone())),
        );

        let keys = Keys::from_keys(
            inner.handler.state.consensus_public_key(),
            inner.handler.state.consensus_secret_key().clone(),
            inner.handler.state.service_public_key(),
            inner.handler.state.service_secret_key().clone(),
        );

        let config = Configuration {
            listener: ListenerConfig {
                address,
                connect_list,
            },
            service: ServiceConfig {
                service_public_key: inner.handler.state.service_public_key(),
                service_secret_key: inner.handler.state.service_secret_key().clone(),
            },
            network: NetworkConfiguration::default(),
            peer_discovery: Vec::new(),
            mempool: Default::default(),
            keys,
        };

        let system_state = SandboxSystemStateProvider {
            listen_address: address,
            shared_time: SharedTime::new(Mutex::new(time)),
        };

        let blockchain = inner.handler.blockchain;
        let mut handler = NodeHandler::new(
            blockchain,
            &address.to_string(),
            node_sender,
            Box::new(system_state),
            config,
            inner.handler.api_state.clone(),
            None,
        );
        handler.initialize();

        let inner = SandboxInner {
            sent: GuardedQueue::default(),
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
            validators_map: self.validators_map,
            services_map: self.services_map,
            addresses: self.addresses,
            connect: None,
        };
        sandbox.process_events();
        sandbox
    }

    fn node_public_key(&self) -> PublicKey {
        self.node_state().consensus_public_key()
    }

    fn node_secret_key(&self) -> SecretKey {
        self.node_state().consensus_secret_key().clone()
    }
}

#[derive(Debug)]
pub struct SandboxBuilder {
    initialize: bool,
    services: Vec<InstanceCollection>,
    validators_count: u8,
    consensus_config: ConsensusConfig,
}

impl Default for SandboxBuilder {
    fn default() -> Self {
        Self {
            initialize: true,
            services: Vec::new(),
            validators_count: 4,
            consensus_config: ConsensusConfig {
                first_round_timeout: 1000,
                status_timeout: 600_000,
                peers_timeout: 600_000,
                txs_block_limit: 1000,
                max_message_len: 1024 * 1024,
                min_propose_timeout: PROPOSE_TIMEOUT,
                max_propose_timeout: PROPOSE_TIMEOUT,
                propose_timeout_threshold: std::u32::MAX,
                validator_keys: Vec::default(),
            },
        }
    }
}

impl SandboxBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn do_not_initialize_connections(mut self) -> Self {
        self.initialize = false;
        self
    }

    pub fn with_services(mut self, services: Vec<InstanceCollection>) -> Self {
        self.services = services;
        self
    }

    pub fn with_consensus<F: FnOnce(&mut ConsensusConfig)>(mut self, update: F) -> Self {
        update(&mut self.consensus_config);
        self
    }

    pub fn with_validators(mut self, n: u8) -> Self {
        self.validators_count = n;
        self
    }

    pub fn build(self) -> Sandbox {
        let mut sandbox = sandbox_with_services_uninitialized(
            self.services,
            self.consensus_config,
            self.validators_count,
        );

        sandbox.inner.borrow_mut().sent.clear(); // To clear initial connect messages.
        if self.initialize {
            let time = sandbox.time();
            sandbox.initialize(time, 1, self.validators_count as usize);
        }
        sandbox
    }
}

fn reject_transaction(schema: &mut Schema<&Fork>, hash: &Hash) -> Result<(), ()> {
    let contains = schema.transactions_pool().contains(hash);
    schema.transactions_pool().remove(hash);
    schema.transactions().remove(hash);

    if contains {
        let x = schema.transactions_pool_len_index().get().unwrap();
        schema.transactions_pool_len_index().set(x - 1);
        Ok(())
    } else {
        Err(())
    }
}

fn gen_primitive_socket_addr(idx: u8) -> SocketAddr {
    let addr = Ipv4Addr::new(idx, idx, idx, idx);
    SocketAddr::new(IpAddr::V4(addr), u16::from(idx))
}

/// Constructs an uninitialized instance of a `Sandbox`.
fn sandbox_with_services_uninitialized(
    services: Vec<InstanceCollection>,
    consensus: ConsensusConfig,
    validators_count: u8,
) -> Sandbox {
    let keys: (Vec<_>) = (0..validators_count)
        .map(|i| {
            (
                gen_keypair_from_seed(&Seed::new([i; SEED_LENGTH])),
                gen_keypair_from_seed(&Seed::new([i + validators_count; SEED_LENGTH])),
            )
        })
        .map(|(v, s)| Keys::from_keys(v.0, v.1, s.0, s.1))
        .collect();

    let validators = keys
        .iter()
        .map(|keys| (keys.consensus_pk(), keys.consensus_sk().clone()))
        .collect::<Vec<_>>();

    let service_keys = keys
        .iter()
        .map(|keys| (keys.service_pk(), keys.service_sk().clone()))
        .collect::<Vec<_>>();

    let addresses = (1..=validators_count)
        .map(gen_primitive_socket_addr)
        .collect::<Vec<_>>();

    let str_addresses: Vec<String> = addresses.iter().map(ToString::to_string).collect();

    let connect_infos: Vec<_> = keys
        .iter()
        .zip(str_addresses.iter())
        .map(|(keys, a)| ConnectInfo {
            address: a.clone(),
            public_key: keys.consensus_pk(),
        })
        .collect();

    let genesis = ConsensusConfig {
        validator_keys: keys
            .iter()
            .map(|keys| ValidatorKeys {
                consensus_key: keys.consensus_pk(),
                service_key: keys.service_pk(),
            })
            .collect(),
        ..consensus
    };

    let connect_list_config =
        ConnectListConfig::from_validator_keys(&genesis.validator_keys, &str_addresses);

    let api_channel = mpsc::channel(100);
    let blockchain = Blockchain::new(
        TemporaryDB::new(),
        service_keys[0].clone(),
        ApiSender(api_channel.0.clone()),
    );

    let (rust_runtime, genesis_config) =
        create_rust_runtime_and_genesis_config(mpsc::channel(1).0, genesis, services);

    let blockchain = BlockchainBuilder::new(blockchain, genesis_config)
        .with_runtime(rust_runtime)
        .build()
        .unwrap();

    let config = Configuration {
        listener: ListenerConfig {
            address: addresses[0],
            connect_list: ConnectList::from_config(connect_list_config),
        },
        service: ServiceConfig {
            service_public_key: service_keys[0].0,
            service_secret_key: service_keys[0].1.clone(),
        },
        network: NetworkConfiguration::default(),
        peer_discovery: Vec::new(),
        mempool: Default::default(),
        keys: keys[0].clone(),
    };

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
    let api_state = SharedNodeState::new(5000);

    let mut handler = NodeHandler::new(
        blockchain,
        &str_addresses[0],
        node_sender,
        Box::new(system_state),
        config.clone(),
        api_state,
        None,
    );
    handler.initialize();

    let inner = SandboxInner {
        sent: GuardedQueue::default(),
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
        addresses: connect_infos,
        connect: None,
    };

    // General assumption; necessary for correct work of consensus algorithm
    assert!(PROPOSE_TIMEOUT < sandbox.first_round_timeout());
    sandbox.process_events();
    sandbox
}

pub fn timestamping_sandbox() -> Sandbox {
    timestamping_sandbox_builder().build()
}

pub fn timestamping_sandbox_builder() -> SandboxBuilder {
    SandboxBuilder::new().with_services(vec![
        InstanceCollection::new(TimestampingService).with_instance(
            TimestampingService::ID,
            "timestamping",
            (),
        ),
        InstanceCollection::new(ConfigUpdaterService).with_instance(
            ConfigUpdaterService::ID,
            "config-updater",
            (),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use exonum_crypto::gen_keypair;

    use super::*;

    impl Sandbox {
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
                    address: addr.to_string(),
                    public_key,
                });
        }

        fn update_config(&self, config: ConsensusConfig) {
            self.inner.borrow_mut().handler.state.update_config(config);
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
        let consensus = gen_keypair();
        let service = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: consensus.0,
            service_key: service.0,
        };

        let new_peer_addr = gen_primitive_socket_addr(2);
        // We also need to add public key from this keypair to the ConnectList.
        // Socket address doesn't matter in this case.
        s.add_peer_to_connect_list(new_peer_addr, validator_keys);

        s.recv(&s.create_connect(
            &consensus.0,
            new_peer_addr.to_string(),
            s.time().into(),
            &user_agent::get(),
            &consensus.1,
        ));
        s.send(
            consensus.0,
            &s.create_connect(
                &s.public_key(ValidatorId(0)),
                s.address(ValidatorId(0)),
                s.time().into(),
                &user_agent::get(),
                s.secret_key(ValidatorId(0)),
            ),
        );
    }

    #[test]
    fn test_sandbox_assert_status() {
        let s = timestamping_sandbox();
        s.assert_state(Height(1), Round(1));
        s.add_time(Duration::from_millis(999));
        s.assert_state(Height(1), Round(1));
        s.add_time(Duration::from_millis(1));
        s.assert_state(Height(1), Round(2));
    }

    #[test]
    #[should_panic(expected = "Expected to send the message")]
    fn test_sandbox_expected_to_send_but_nothing_happened() {
        let s = timestamping_sandbox();
        s.send(
            s.public_key(ValidatorId(1)),
            &s.create_connect(
                &s.public_key(ValidatorId(0)),
                s.address(ValidatorId(0)),
                s.time().into(),
                &user_agent::get(),
                s.secret_key(ValidatorId(0)),
            ),
        );
    }

    #[test]
    #[should_panic(expected = "Expected to send message to other recipient")]
    fn test_sandbox_expected_to_send_another_message() {
        let s = timestamping_sandbox();
        // See comments to `test_sandbox_recv_and_send`.
        let (public, secret) = gen_keypair();
        let (service_key, _) = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: public,
            service_key,
        };
        s.add_peer_to_connect_list(gen_primitive_socket_addr(1), validator_keys);
        s.recv(&s.create_connect(
            &public,
            s.address(ValidatorId(2)),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
        s.send(
            s.public_key(ValidatorId(1)),
            &s.create_connect(
                &s.public_key(ValidatorId(0)),
                s.address(ValidatorId(0)),
                s.time().into(),
                &user_agent::get(),
                s.secret_key(ValidatorId(0)),
            ),
        );
    }

    #[test]
    #[should_panic(expected = "Sent unexpected message")]
    fn test_sandbox_unexpected_message_when_drop() {
        let s = timestamping_sandbox();
        // See comments to `test_sandbox_recv_and_send`.
        let (public, secret) = gen_keypair();
        let (service_key, _) = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: public,
            service_key,
        };
        s.add_peer_to_connect_list(gen_primitive_socket_addr(1), validator_keys);
        s.recv(&s.create_connect(
            &public,
            s.address(ValidatorId(2)),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
    }

    #[test]
    #[should_panic(expected = "Sent unexpected message")]
    fn test_sandbox_unexpected_message_when_handle_another_message() {
        let s = timestamping_sandbox();
        // See comments to `test_sandbox_recv_and_send`.
        let (public, secret) = gen_keypair();
        let (service_key, _) = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: public,
            service_key,
        };
        s.add_peer_to_connect_list(gen_primitive_socket_addr(1), validator_keys);
        s.recv(&s.create_connect(
            &public,
            s.address(ValidatorId(2)),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
        s.recv(&s.create_connect(
            &public,
            s.address(ValidatorId(3)),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
        panic!("Oops! We don't catch unexpected message");
    }

    #[test]
    #[should_panic(expected = "Sent unexpected message")]
    fn test_sandbox_unexpected_message_when_time_changed() {
        let s = timestamping_sandbox();
        // See comments to `test_sandbox_recv_and_send`.
        let (public, secret) = gen_keypair();
        let (service_key, _) = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: public,
            service_key,
        };
        s.add_peer_to_connect_list(gen_primitive_socket_addr(1), validator_keys);
        s.recv(&s.create_connect(
            &public,
            s.address(ValidatorId(2)),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
        s.add_time(Duration::from_millis(1000));
        panic!("Oops! We don't catch unexpected message");
    }
}
