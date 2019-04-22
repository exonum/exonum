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

use bit_vec::BitVec;
use futures::{sync::mpsc, Async, Future, Sink, Stream};

use std::{
    cell::{Ref, RefCell, RefMut},
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque},
    iter::FromIterator,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::{AddAssign, Deref},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use exonum_merkledb::{HashTag, MapProof, ObjectHash, TemporaryDB};

use crate::{
    blockchain::{
        Block, BlockProof, Blockchain, ConsensusConfig, GenesisConfig, Schema, Service,
        SharedNodeState, StoredConfiguration, Transaction, ValidatorKeys,
    },
    crypto::{gen_keypair, gen_keypair_from_seed, Hash, PublicKey, SecretKey, Seed, SEED_LENGTH},
    events::{
        network::NetworkConfiguration, Event, EventHandler, InternalEvent, InternalRequest,
        NetworkEvent, NetworkRequest, TimeoutRequest,
    },
    helpers::{user_agent, Height, Milliseconds, Round, ValidatorId},
    messages::{
        BlockRequest, BlockResponse, Connect, Message, PeersRequest, Precommit, Prevote,
        PrevotesRequest, Propose, ProposeRequest, ProtocolMessage, RawTransaction, Signed,
        SignedMessage, Status, TransactionsRequest, TransactionsResponse,
    },
    node::{
        ApiSender, Configuration, ConnectInfo, ConnectList, ConnectListConfig, ExternalMessage,
        ListenerConfig, NodeHandler, NodeSender, PeerAddress, ServiceConfig, State,
        SystemStateProvider,
    },
    sandbox::{
        config_updater::ConfigUpdateService, sandbox_tests_helper::PROPOSE_TIMEOUT,
        timestamping::TimestampingService,
    },
};

mod config_updater;
mod consensus;
mod old;
mod requests;
mod sandbox_tests_helper;
mod timestamping;

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
    pub sent: VecDeque<(PublicKey, Message)>,
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
                        let protocol_msg =
                            Message::deserialize(msg).expect("Expected valid message.");
                        self.sent.push_back((peer, protocol_msg))
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
                    InternalRequest::Shutdown => unimplemented!(),
                    InternalRequest::VerifyMessage(message) => {
                        let protocol =
                            Message::deserialize(SignedMessage::from_raw_buffer(message).unwrap())
                                .unwrap();
                        self.handler.handle_event(
                            InternalEvent::MessageVerified(Box::new(protocol)).into(),
                        );
                    }
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
    addresses: Vec<ConnectInfo>,
    /// Connect message used during initialization.
    connect: Option<Signed<Connect>>,
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
            panic!("Send unexpected message {:?} to {}", msg, addr);
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
        author: &PublicKey,
        to: &PublicKey,
        height: Height,
        secret_key: &SecretKey,
    ) -> Signed<BlockRequest> {
        Message::concrete(BlockRequest::new(to, height), *author, secret_key)
    }

    /// Creates a `Status` message signed by this validator.
    pub fn create_status(
        &self,
        author: &PublicKey,
        height: Height,
        last_hash: &Hash,
        secret_key: &SecretKey,
    ) -> Signed<Status> {
        Message::concrete(Status::new(height, last_hash), *author, secret_key)
    }

    /// Creates a `BlockResponse` message signed by this validator.
    pub fn create_block_response<I: IntoIterator<Item = Signed<Precommit>>>(
        &self,
        public_key: &PublicKey,
        to: &PublicKey,
        block: Block,
        precommits: I,
        tx_hashes: &[Hash],
        secret_key: &SecretKey,
    ) -> Signed<BlockResponse> {
        Message::concrete(
            BlockResponse::new(
                to,
                block,
                precommits.into_iter().map(Signed::serialize).collect(),
                tx_hashes,
            ),
            *public_key,
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
    ) -> Signed<Connect> {
        Message::concrete(
            Connect::new(&addr, time, user_agent),
            *public_key,
            secret_key,
        )
    }

    /// Creates a `PeersRequest` message signed by this validator.
    pub fn create_peers_request(
        &self,
        public_key: &PublicKey,
        to: &PublicKey,
        secret_key: &SecretKey,
    ) -> Signed<PeersRequest> {
        Message::concrete(PeersRequest::new(to), *public_key, secret_key)
    }

    /// Creates a `Propose` message signed by this validator.
    pub fn create_propose(
        &self,
        validator_id: ValidatorId,
        height: Height,
        round: Round,
        last_hash: &Hash,
        tx_hashes: &[Hash],
        secret_key: &SecretKey,
    ) -> Signed<Propose> {
        Message::concrete(
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
        propose_hash: &Hash,
        block_hash: &Hash,
        system_time: chrono::DateTime<chrono::Utc>,
        secret_key: &SecretKey,
    ) -> Signed<Precommit> {
        Message::concrete(
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
        propose_hash: &Hash,
        locked_round: Round,
        secret_key: &SecretKey,
    ) -> Signed<Prevote> {
        Message::concrete(
            Prevote::new(
                validator_id,
                propose_height,
                propose_round,
                &propose_hash,
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
        from: &PublicKey,
        to: &PublicKey,
        height: Height,
        round: Round,
        propose_hash: &Hash,
        validators: BitVec,
        secret_key: &SecretKey,
    ) -> Signed<PrevotesRequest> {
        Message::concrete(
            PrevotesRequest::new(to, height, round, propose_hash, validators),
            *from,
            secret_key,
        )
    }

    /// Creates a `ProposeRequest` message signed by this validator.
    pub fn create_propose_request(
        &self,
        author: &PublicKey,
        to: &PublicKey,
        height: Height,
        propose_hash: &Hash,
        secret_key: &SecretKey,
    ) -> Signed<ProposeRequest> {
        Message::concrete(
            ProposeRequest::new(to, height, propose_hash),
            *author,
            secret_key,
        )
    }

    /// Creates a `TransactionsRequest` message signed by this validator.
    pub fn create_transactions_request(
        &self,
        author: &PublicKey,
        to: &PublicKey,
        txs: &[Hash],
        secret_key: &SecretKey,
    ) -> Signed<TransactionsRequest> {
        Message::concrete(TransactionsRequest::new(to, txs), *author, secret_key)
    }

    /// Creates a `TransactionsResponse` message signed by this validator.
    pub fn create_transactions_response<I>(
        &self,
        author: &PublicKey,
        to: &PublicKey,
        txs: I,
        secret_key: &SecretKey,
    ) -> Signed<TransactionsResponse>
    where
        I: IntoIterator<Item = Signed<RawTransaction>>,
    {
        Message::concrete(
            TransactionsResponse::new(to, txs.into_iter().map(Signed::serialize).collect()),
            *author,
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
    pub fn connect(&self) -> Option<&Signed<Connect>> {
        self.connect.as_ref()
    }

    pub fn recv<T: ProtocolMessage>(&self, msg: &Signed<T>) {
        self.check_unexpected_message();
        let event = NetworkEvent::MessageReceived(msg.clone().serialize());
        self.inner.borrow_mut().handle_event(event);
    }

    pub fn recv_rebroadcast(&self) {
        self.check_unexpected_message();
        self.inner
            .borrow_mut()
            .handle_event(ExternalMessage::Rebroadcast);
    }

    pub fn process_events(&self) {
        self.inner.borrow_mut().process_events();
    }

    pub fn pop_sent_message(&self) -> Option<(PublicKey, Message)> {
        self.inner.borrow_mut().sent.pop_front()
    }

    pub fn send<T: ProtocolMessage>(&self, key: PublicKey, msg: &Signed<T>) {
        let expected_msg = T::into_protocol(msg.clone());
        self.process_events();
        if let Some((real_addr, real_msg)) = self.pop_sent_message() {
            assert_eq!(expected_msg, real_msg, "Expected to send other message");
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
            let peers_request =
                PeersRequest::try_from(msg).expect("Incorrect message. PeersRequest was expected");

            let id = self.addresses.iter().position(|ref a| a.public_key == addr);
            if let Some(id) = id {
                assert_eq!(&self.public_key(ValidatorId(id as u16)), peers_request.to());
            } else {
                panic!("Sending PeersRequest to unknown peer {:?}", addr);
            }
        } else {
            panic!("Expected to send the PeersRequest message but nothing happened");
        }
    }

    pub fn broadcast<T: ProtocolMessage>(&self, msg: &Signed<T>) {
        self.broadcast_to_addrs(msg, self.addresses.iter().map(|i| &i.public_key).skip(1));
    }

    pub fn try_broadcast<T: ProtocolMessage>(&self, msg: &Signed<T>) -> Result<(), String> {
        self.try_broadcast_to_addrs(msg, self.addresses.iter().map(|i| &i.public_key).skip(1))
    }

    pub fn broadcast_to_addrs<'a, T: ProtocolMessage, I>(&self, msg: &Signed<T>, addresses: I)
    where
        I: IntoIterator<Item = &'a PublicKey>,
    {
        self.try_broadcast_to_addrs(msg, addresses).unwrap();
    }

    pub fn try_broadcast_to_addrs<'a, T: ProtocolMessage, I>(
        &self,
        msg: &Signed<T>,
        addresses: I,
    ) -> Result<(), String>
    where
        I: IntoIterator<Item = &'a PublicKey>,
    {
        let expected_msg = msg.signed_message();

        // If node is excluded from validators, then it still will broadcast messages.
        // So in that case we should not skip addresses and validators count.
        let mut expected_set: HashSet<_> = HashSet::from_iter(addresses);

        for _ in 0..expected_set.len() {
            if let Some((real_addr, real_msg)) = self.pop_sent_message() {
                assert_eq!(
                    expected_msg,
                    real_msg.signed_message(),
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

    pub fn check_broadcast_status(&self, height: Height, block_hash: &Hash) {
        self.broadcast(&self.create_status(
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

    pub fn filter_present_transactions<'a, I>(&self, txs: I) -> Vec<Signed<RawTransaction>>
    where
        I: IntoIterator<Item = &'a Signed<RawTransaction>>,
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
        I: IntoIterator<Item = &'a Signed<RawTransaction>>,
    {
        let height = self.current_height();
        let mut blockchain = self.blockchain_mut();
        let (hashes, recover, patch) = {
            let mut hashes = Vec::new();
            let mut recover = BTreeSet::new();
            let fork = blockchain.fork();
            {
                let mut schema = Schema::new(&fork);
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
            let fork = blockchain.fork();
            {
                let mut schema = Schema::new(&fork);
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
        schema.configs().object_hash()
    }

    pub fn cfg(&self) -> StoredConfiguration {
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
        schema.actual_configuration()
    }

    pub fn majority_count(&self, num_validators: usize) -> usize {
        num_validators * 2 / 3 + 1
    }

    pub fn first_round_timeout(&self) -> Milliseconds {
        self.cfg().consensus.first_round_timeout
    }

    pub fn round_timeout_increase(&self) -> Milliseconds {
        (self.cfg().consensus.first_round_timeout
            * ConsensusConfig::TIMEOUT_LINEAR_INCREASE_PERCENT)
            / 100
    }

    pub fn current_round_timeout(&self) -> Milliseconds {
        let previous_round: u64 = self.current_round().previous().into();
        self.first_round_timeout() + previous_round * self.round_timeout_increase()
    }

    #[allow(clippy::let_and_return)]
    pub fn transactions_hashes(&self) -> Vec<Hash> {
        let snapshot = self.blockchain_ref().snapshot();
        let schema = Schema::new(&snapshot);
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

    pub fn assert_pool_len(&self, expected: u64) {
        let view = self.blockchain_ref().snapshot();
        let schema = Schema::new(&view);
        assert_eq!(expected, schema.transactions_pool_len());
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
                c.pub_addr().parse().expect("Expected resolved address"),
                time.into(),
                c.user_agent(),
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
            .expect("Fail to parse socket address");
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
            &address.to_string(),
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
                address: addr.to_string(),
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
    pub fn from_peers(peers: &HashMap<PublicKey, Signed<Connect>>) -> Self {
        let peers: BTreeMap<PublicKey, PeerAddress> = peers
            .iter()
            .map(|(p, c)| (*p, PeerAddress::new(c.pub_addr().to_owned())))
            .collect();
        ConnectList { peers }
    }
}

pub struct SandboxBuilder {
    initialize: bool,
    services: Vec<Box<dyn Service>>,
    validators_count: u8,
    consensus_config: ConsensusConfig,
}

impl SandboxBuilder {
    pub fn new() -> Self {
        SandboxBuilder {
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
            },
        }
    }

    pub fn do_not_initialize_connections(mut self) -> Self {
        self.initialize = false;
        self
    }

    pub fn with_services(mut self, services: Vec<Box<dyn Service>>) -> Self {
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
        let _ = env_logger::Builder::from_default_env()
            .target(env_logger::Target::Stdout)
            .try_init();

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

fn gen_primitive_socket_addr(idx: u8) -> SocketAddr {
    let addr = Ipv4Addr::new(idx, idx, idx, idx);
    SocketAddr::new(IpAddr::V4(addr), u16::from(idx))
}

/// Constructs an uninitialized instance of a `Sandbox`.
fn sandbox_with_services_uninitialized(
    services: Vec<Box<dyn Service>>,
    consensus: ConsensusConfig,
    validators_count: u8,
) -> Sandbox {
    let validators = (0..validators_count)
        .map(|i| gen_keypair_from_seed(&Seed::new([i; SEED_LENGTH])))
        .collect::<Vec<_>>();

    let service_keys = (0..validators_count)
        .map(|i| gen_keypair_from_seed(&Seed::new([i + validators_count; SEED_LENGTH])))
        .collect::<Vec<_>>();

    let addresses = (1..=validators_count)
        .map(gen_primitive_socket_addr)
        .collect::<Vec<_>>();

    let str_addresses: Vec<String> = addresses.iter().map(ToString::to_string).collect();

    let connect_infos: Vec<_> = validators
        .iter()
        .map(|(p, _)| p)
        .zip(str_addresses.iter())
        .map(|(p, a)| ConnectInfo {
            address: a.clone(),
            public_key: *p,
        })
        .collect();

    let api_channel = mpsc::channel(100);
    let db = TemporaryDB::new();
    let mut blockchain = Blockchain::new(
        db,
        services,
        service_keys[0].0,
        service_keys[0].1.clone(),
        ApiSender::new(api_channel.0.clone()),
    );

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
        ConnectListConfig::from_validator_keys(&genesis.validator_keys, &str_addresses);

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
        &str_addresses[0],
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
        Box::new(TimestampingService::new()),
        Box::new(ConfigUpdateService::new()),
    ])
}

pub fn compute_tx_hash<'a, I>(txs: I) -> Hash
where
    I: IntoIterator<Item = &'a Signed<RawTransaction>>,
{
    let txs = txs.into_iter().map(Signed::hash).collect::<Vec<Hash>>();
    HashTag::hash_list(&txs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::{ExecutionResult, ServiceContext, TransactionContext, TransactionSet};
    use crate::crypto::{gen_keypair_from_seed, Seed};
    use crate::messages::RawTransaction;
    use crate::proto::schema::tests::TxAfterCommit;
    use crate::sandbox::sandbox_tests_helper::{add_one_height, SandboxState};
    use exonum_merkledb::{impl_binary_value_for_message, BinaryValue, Snapshot};
    use protobuf::Message as PbMessage;
    use std::borrow::Cow;

    const SERVICE_ID: u16 = 1;

    #[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
    #[exonum(crate = "crate")]
    enum HandleCommitTransactions {
        TxAfterCommit(TxAfterCommit),
    }

    impl TxAfterCommit {
        pub fn new_with_height(height: Height) -> Signed<RawTransaction> {
            let keypair = gen_keypair_from_seed(&Seed::new([22; 32]));
            let mut payload_tx = TxAfterCommit::new();
            payload_tx.set_height(height.0);
            Message::sign_transaction(payload_tx, SERVICE_ID, keypair.0, &keypair.1)
        }
    }

    impl_binary_value_for_message! { TxAfterCommit }

    impl Transaction for TxAfterCommit {
        fn execute(&self, _: TransactionContext) -> ExecutionResult {
            Ok(())
        }
    }

    struct AfterCommitService;

    impl Service for AfterCommitService {
        fn service_id(&self) -> u16 {
            SERVICE_ID
        }

        fn service_name(&self) -> &str {
            "after_commit"
        }

        fn state_hash(&self, _: &dyn Snapshot) -> Vec<Hash> {
            Vec::new()
        }

        fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
            let tx = HandleCommitTransactions::tx_from_raw(raw)?;
            Ok(tx.into())
        }

        fn after_commit(&self, context: &ServiceContext) {
            let tx = TxAfterCommit::new_with_height(context.height());
            context.broadcast_signed_transaction(tx);
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

        let new_peer_addr = gen_primitive_socket_addr(2);
        // We also need to add public key from this keypair to the ConnectList.
        // Socket address doesn't matter in this case.
        s.add_peer_to_connect_list(new_peer_addr, validator_keys);

        s.recv(&s.create_connect(
            &public,
            new_peer_addr.to_string(),
            s.time().into(),
            &user_agent::get(),
            &secret,
        ));
        s.send(
            public,
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
        let (service, _) = gen_keypair();
        let validator_keys = ValidatorKeys {
            consensus_key: public,
            service_key: service,
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
        s.recv(&s.create_connect(
            &public,
            s.address(ValidatorId(2)),
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

    #[test]
    fn test_sandbox_service_after_commit() {
        let sandbox = SandboxBuilder::new()
            .with_services(vec![
                Box::new(AfterCommitService),
                Box::new(TimestampingService::new()),
            ])
            .build();
        let state = SandboxState::new();
        add_one_height(&sandbox, &state);
        let tx = TxAfterCommit::new_with_height(Height(1));
        sandbox.broadcast(&tx);
    }
}
