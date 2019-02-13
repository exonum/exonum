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

//! State of the `NodeHandler`.

use bit_vec::BitVec;
use serde_json::Value;

use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap, HashSet},
    ops::Deref,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime},
};

use crate::blockchain::{ConsensusConfig, StoredConfiguration, ValidatorKeys};
use crate::crypto::{Hash, PublicKey, SecretKey};
use crate::events::network::ConnectedPeerAddr;
use crate::helpers::{Height, Milliseconds, Round, ValidatorId};
use crate::messages::{
    BlockResponse, Connect, Consensus as ConsensusMessage, Precommit, Prevote, Propose,
    RawTransaction, Signed,
};
use crate::node::{
    connect_list::{ConnectList, PeerAddress},
    ConnectInfo,
};
use crate::storage::{KeySetIndex, MapIndex, Patch, Snapshot};

// TODO: Move request timeouts into node configuration. (ECR-171)

/// Timeout value for the `ProposeRequest` message.
pub const PROPOSE_REQUEST_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `TransactionsRequest` message.
pub const TRANSACTIONS_REQUEST_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `PrevotesRequest` message.
pub const PREVOTES_REQUEST_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `BlockRequest` message.
pub const BLOCK_REQUEST_TIMEOUT: Milliseconds = 100;

/// State of the `NodeHandler`.
#[derive(Debug)]
pub struct State {
    validator_state: Option<ValidatorState>,
    our_connect_message: Signed<Connect>,

    consensus_public_key: PublicKey,
    consensus_secret_key: SecretKey,
    service_public_key: PublicKey,
    service_secret_key: SecretKey,

    config: StoredConfiguration,
    connect_list: SharedConnectList,

    peers: HashMap<PublicKey, Signed<Connect>>,
    connections: HashMap<PublicKey, ConnectedPeerAddr>,
    height_start_time: SystemTime,
    height: Height,

    round: Round,
    locked_round: Round,
    locked_propose: Option<Hash>,
    last_hash: Hash,

    // Messages.
    proposes: HashMap<Hash, ProposeState>,
    blocks: HashMap<Hash, BlockState>,
    prevotes: HashMap<(Round, Hash), Votes<Signed<Prevote>>>,
    precommits: HashMap<(Round, Hash), Votes<Signed<Precommit>>>,

    queued: Vec<ConsensusMessage>,

    unknown_txs: HashMap<Hash, Vec<Hash>>,
    unknown_proposes_with_precommits: HashMap<Hash, Vec<(Round, Hash)>>,

    // Our requests state.
    requests: HashMap<RequestData, RequestState>,

    // Maximum of node height in consensus messages.
    nodes_max_height: BTreeMap<PublicKey, Height>,

    validators_rounds: BTreeMap<ValidatorId, Round>,

    incomplete_block: Option<IncompleteBlock>,
}

/// State of a validator-node.
#[derive(Debug, Clone)]
pub struct ValidatorState {
    id: ValidatorId,
    our_prevotes: HashMap<Round, Signed<Prevote>>,
    our_precommits: HashMap<Round, Signed<Precommit>>,
}

/// `RequestData` represents a request for some data to other nodes. Each enum variant will be
/// translated to the corresponding request-message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestData {
    /// Represents `ProposeRequest` message.
    Propose(Hash),
    /// Represents `TransactionsRequest` message for `Propose`.
    ProposeTransactions(Hash),
    /// Represents `TransactionsRequest` message for `BlockResponse`.
    BlockTransactions,
    /// Represents `PrevotesRequest` message.
    Prevotes(Round, Hash),
    /// Represents `BlockRequest` message.
    Block(Height),
}

#[derive(Debug)]
struct RequestState {
    // Number of attempts made.
    retries: u16,
    // Nodes that have the required information.
    known_nodes: HashSet<PublicKey>,
}

/// `ProposeState` represents the state of some propose and is used for tracking of unknown
#[derive(Debug)]
/// transactions.
pub struct ProposeState {
    propose: Signed<Propose>,
    unknown_txs: HashSet<Hash>,
    block_hash: Option<Hash>,
    // Whether the message has been saved to the consensus messages' cache or not.
    is_saved: bool,
}

/// State of a block.
#[derive(Clone, Debug)]
pub struct BlockState {
    hash: Hash,
    // Changes that should be made for block committing.
    patch: Patch,
    txs: Vec<Hash>,
    proposer_id: ValidatorId,
}

/// Incomplete block.
#[derive(Clone, Debug)]
pub struct IncompleteBlock {
    msg: Signed<BlockResponse>,
    unknown_txs: HashSet<Hash>,
}

/// `VoteMessage` trait represents voting messages such as `Precommit` and `Prevote`.
pub trait VoteMessage: Clone {
    /// Return validator if of the message.
    fn validator(&self) -> ValidatorId;
}

impl VoteMessage for Signed<Precommit> {
    fn validator(&self) -> ValidatorId {
        self.deref().validator()
    }
}

impl VoteMessage for Signed<Prevote> {
    fn validator(&self) -> ValidatorId {
        self.deref().validator()
    }
}

/// Contains voting messages alongside with there validator ids.
#[derive(Debug)]
pub struct Votes<T: VoteMessage> {
    messages: Vec<T>,
    validators: BitVec,
    count: usize,
}

impl ValidatorState {
    /// Creates new `ValidatorState` with given validator id.
    pub fn new(id: ValidatorId) -> Self {
        Self {
            id,
            our_precommits: HashMap::new(),
            our_prevotes: HashMap::new(),
        }
    }

    /// Returns validator id.
    pub fn id(&self) -> ValidatorId {
        self.id
    }

    /// Sets new validator id.
    pub fn set_validator_id(&mut self, id: ValidatorId) {
        self.id = id;
    }

    /// Checks if the node has pre-vote for the specified round.
    pub fn have_prevote(&self, round: Round) -> bool {
        self.our_prevotes.get(&round).is_some()
    }

    /// Clears pre-commits and pre-votes.
    pub fn clear(&mut self) {
        self.our_precommits.clear();
        self.our_prevotes.clear();
    }
}

impl<T> Votes<T>
where
    T: VoteMessage,
{
    /// Creates a new `Votes` instance with a specified validators number.
    pub fn new(validators_len: usize) -> Self {
        Self {
            messages: Vec::new(),
            validators: BitVec::from_elem(validators_len, false),
            count: 0,
        }
    }

    /// Inserts a new message if it hasn't been inserted yet.
    pub fn insert(&mut self, message: T) {
        let voter: usize = message.validator().into();
        if !self.validators[voter] {
            self.count += 1;
            self.validators.set(voter, true);
            self.messages.push(message);
        }
    }

    /// Returns validators.
    pub fn validators(&self) -> &BitVec {
        &self.validators
    }

    /// Returns number of contained messages.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Returns messages.
    pub fn messages(&self) -> &Vec<T> {
        &self.messages
    }
}

impl RequestData {
    /// Returns timeout value of the data request.
    pub fn timeout(&self) -> Duration {
        let ms = match *self {
            RequestData::Propose(..) => PROPOSE_REQUEST_TIMEOUT,
            RequestData::ProposeTransactions(..) | RequestData::BlockTransactions => {
                TRANSACTIONS_REQUEST_TIMEOUT
            }
            RequestData::Prevotes(..) => PREVOTES_REQUEST_TIMEOUT,
            RequestData::Block(..) => BLOCK_REQUEST_TIMEOUT,
        };
        Duration::from_millis(ms)
    }
}

impl RequestState {
    fn new() -> Self {
        Self {
            retries: 0,
            known_nodes: HashSet::new(),
        }
    }

    fn insert(&mut self, peer: PublicKey) {
        self.known_nodes.insert(peer);
    }

    fn remove(&mut self, peer: &PublicKey) {
        self.retries += 1;
        self.known_nodes.remove(peer);
    }

    fn is_empty(&self) -> bool {
        self.known_nodes.is_empty()
    }

    fn peek(&self) -> Option<PublicKey> {
        self.known_nodes.iter().next().cloned()
    }
}

impl ProposeState {
    /// Returns hash of the propose.
    pub fn hash(&self) -> Hash {
        self.propose.hash()
    }

    /// Returns block hash propose was executed.
    pub fn block_hash(&self) -> Option<Hash> {
        self.block_hash
    }

    /// Set block hash on propose execute.
    pub fn set_block_hash(&mut self, block_hash: Hash) {
        self.block_hash = Some(block_hash)
    }

    /// Returns propose-message.
    pub fn message(&self) -> &Signed<Propose> {
        &self.propose
    }

    /// Returns unknown transactions of the propose.
    pub fn unknown_txs(&self) -> &HashSet<Hash> {
        &self.unknown_txs
    }

    /// Returns `true` if there are unknown transactions in the propose.
    pub fn has_unknown_txs(&self) -> bool {
        !self.unknown_txs.is_empty()
    }

    /// Indicates whether Propose has been saved to the consensus messages cache
    pub fn is_saved(&self) -> bool {
        self.is_saved
    }

    /// Marks Propose as saved to the consensus messages cache
    pub fn set_saved(&mut self, saved: bool) {
        self.is_saved = saved;
    }
}

impl BlockState {
    /// Creates a new `BlockState` instance with the given parameters.
    pub fn new(hash: Hash, patch: Patch, txs: Vec<Hash>, proposer_id: ValidatorId) -> Self {
        Self {
            hash,
            patch,
            txs,
            proposer_id,
        }
    }

    /// Returns hash of the block.
    pub fn hash(&self) -> Hash {
        self.hash
    }

    /// Returns the changes that should be made for block committing.
    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    /// Returns block's transactions.
    pub fn txs(&self) -> &Vec<Hash> {
        &self.txs
    }

    /// Returns id of the validator that proposed the block.
    pub fn proposer_id(&self) -> ValidatorId {
        self.proposer_id
    }
}

impl IncompleteBlock {
    /// Returns `BlockResponse` message.
    pub fn message(&self) -> &Signed<BlockResponse> {
        &self.msg
    }

    /// Returns unknown transactions of the block.
    pub fn unknown_txs(&self) -> &HashSet<Hash> {
        &self.unknown_txs
    }

    /// Returns `true` if there are unknown transactions in the block.
    pub fn has_unknown_txs(&self) -> bool {
        !self.unknown_txs.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
/// Shared `ConnectList` representation to be used in network.
pub struct SharedConnectList {
    inner: Arc<RwLock<ConnectList>>,
}

impl SharedConnectList {
    /// Creates `SharedConnectList` from `ConnectList`.
    pub fn from_connect_list(connect_list: ConnectList) -> Self {
        SharedConnectList {
            inner: Arc::new(RwLock::new(connect_list)),
        }
    }

    /// Returns `true` if a peer with the given public key can connect.
    pub fn is_peer_allowed(&self, public_key: &PublicKey) -> bool {
        let connect_list = self.inner.read().expect("ConnectList read lock");
        connect_list.is_peer_allowed(public_key)
    }

    /// Return `peers` from underlying `ConnectList`
    pub fn peers(&self) -> Vec<ConnectInfo> {
        self.inner
            .read()
            .expect("ConnectList read lock")
            .peers
            .iter()
            .map(|(pk, a)| ConnectInfo {
                address: a.address.clone(),
                public_key: *pk,
            })
            .collect()
    }

    /// Update peer address in the connect list.
    pub fn update_peer(&mut self, public_key: &PublicKey, address: String) {
        let mut conn_list = self.inner.write().expect("ConnectList write lock");
        conn_list.update_peer(public_key, address);
    }

    /// Get peer address using public key.
    pub fn find_address_by_key(&self, public_key: &PublicKey) -> Option<PeerAddress> {
        let connect_list = self.inner.read().expect("ConnectList read lock");
        connect_list.find_address_by_pubkey(public_key).cloned()
    }
}

impl State {
    /// Creates state with the given parameters.
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::too_many_arguments))]
    pub fn new(
        validator_id: Option<ValidatorId>,
        consensus_public_key: PublicKey,
        consensus_secret_key: SecretKey,
        service_public_key: PublicKey,
        service_secret_key: SecretKey,
        connect_list: ConnectList,
        stored: StoredConfiguration,
        connect: Signed<Connect>,
        peers: HashMap<PublicKey, Signed<Connect>>,
        last_hash: Hash,
        last_height: Height,
        height_start_time: SystemTime,
    ) -> Self {
        Self {
            validator_state: validator_id.map(ValidatorState::new),
            consensus_public_key,
            consensus_secret_key,
            service_public_key,
            service_secret_key,
            connect_list: SharedConnectList::from_connect_list(connect_list),
            peers,
            connections: HashMap::new(),
            height: last_height,
            height_start_time,
            round: Round::zero(),
            locked_round: Round::zero(),
            locked_propose: None,
            last_hash,

            proposes: HashMap::new(),
            blocks: HashMap::new(),
            prevotes: HashMap::new(),
            precommits: HashMap::new(),

            queued: Vec::new(),

            unknown_txs: HashMap::new(),
            unknown_proposes_with_precommits: HashMap::new(),

            nodes_max_height: BTreeMap::new(),
            validators_rounds: BTreeMap::new(),

            our_connect_message: connect,

            requests: HashMap::new(),

            config: stored,

            incomplete_block: None,
        }
    }

    /// Returns `ValidatorState` if the node is validator.
    pub fn validator_state(&self) -> &Option<ValidatorState> {
        &self.validator_state
    }

    /// Returns validator id of the node if it is a validator. Returns `None` otherwise.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_state.as_ref().map(|s| s.id())
    }

    /// Updates the validator id. If there hasn't been `ValidatorState` for that id, then a new
    /// state will be created.
    pub fn renew_validator_id(&mut self, id: Option<ValidatorId>) {
        let validator_state = self.validator_state.take();
        self.validator_state = id.map(move |id| match validator_state {
            Some(mut state) => {
                state.set_validator_id(id);
                state
            }
            None => ValidatorState::new(id),
        });
    }

    /// Checks if the node is a validator.
    pub fn is_validator(&self) -> bool {
        self.validator_state().is_some()
    }

    /// Checks if the node is a leader for the current height and round.
    pub fn is_leader(&self) -> bool {
        self.validator_state()
            .as_ref()
            .map_or(false, |validator| self.leader(self.round()) == validator.id)
    }

    /// Returns node's ConnectList.
    pub fn connect_list(&self) -> SharedConnectList {
        self.connect_list.clone()
    }

    /// Returns public (consensus and service) keys of known validators.
    pub fn validators(&self) -> &[ValidatorKeys] {
        &self.config.validator_keys
    }

    /// Returns `StoredConfiguration`.
    pub fn config(&self) -> &StoredConfiguration {
        &self.config
    }

    /// Returns validator id with a specified public key.
    pub fn find_validator(&self, peer: PublicKey) -> Option<ValidatorId> {
        self.validators()
            .iter()
            .position(|pk| pk.consensus_key == peer)
            .map(|id| ValidatorId(id as u16))
    }

    /// Returns `ConsensusConfig`.
    pub fn consensus_config(&self) -> &ConsensusConfig {
        &self.config.consensus
    }

    /// Returns `BTreeMap` with service configs identified by name.
    pub fn services_config(&self) -> &BTreeMap<String, Value> {
        &self.config.services
    }

    /// Replaces `StoredConfiguration` with a new one and updates validator id of the current node
    /// if the new config is different from the previous one.
    pub fn update_config(&mut self, config: StoredConfiguration) {
        if self.config == config {
            return;
        }

        trace!("Updating node config={:#?}", config);
        let validator_id = config
            .validator_keys
            .iter()
            .position(|pk| pk.consensus_key == *self.consensus_public_key())
            .map(|id| ValidatorId(id as u16));

        // TODO: update connect list (ECR-1745)

        self.renew_validator_id(validator_id);
        trace!("Validator={:#?}", self.validator_state());

        self.config = config;
    }

    /// Adds the public key, address, and `Connect` message of a validator.
    pub fn add_peer(&mut self, pubkey: PublicKey, msg: Signed<Connect>) -> bool {
        self.peers.insert(pubkey, msg).is_none()
    }

    /// Add connection to the connection list.
    pub fn add_connection(&mut self, pubkey: PublicKey, address: ConnectedPeerAddr) {
        self.connections.insert(pubkey, address);
    }

    /// Removes a peer by the socket address. Returns `Some` (connect message) of the peer if it was
    /// indeed connected or `None` if there was no connection with given socket address.
    pub fn remove_peer_with_pubkey(&mut self, key: &PublicKey) -> Option<Signed<Connect>> {
        self.connections.remove(key);
        if let Some(c) = self.peers.remove(key) {
            Some(c)
        } else {
            None
        }
    }

    /// Checks if this node considers a peer to be a validator.
    pub fn peer_is_validator(&self, pubkey: &PublicKey) -> bool {
        self.config
            .validator_keys
            .iter()
            .any(|x| x.consensus_key == *pubkey)
    }

    /// Checks if a peer is in this node's connection list.
    pub fn peer_in_connect_list(&self, pubkey: &PublicKey) -> bool {
        self.connect_list.is_peer_allowed(pubkey)
    }

    /// Returns the keys of known peers with their `Connect` messages.
    pub fn peers(&self) -> &HashMap<PublicKey, Signed<Connect>> {
        &self.peers
    }

    /// Returns the addresses of known connections with public keys of its' validators.
    pub fn connections(&self) -> &HashMap<PublicKey, ConnectedPeerAddr> {
        &self.connections
    }

    /// Returns public key of a validator identified by id.
    pub fn consensus_public_key_of(&self, id: ValidatorId) -> Option<PublicKey> {
        let id: usize = id.into();
        self.validators().get(id).map(|x| x.consensus_key)
    }

    /// Returns the consensus public key of the current node.
    pub fn consensus_public_key(&self) -> &PublicKey {
        &self.consensus_public_key
    }

    /// Returns the consensus secret key of the current node.
    pub fn consensus_secret_key(&self) -> &SecretKey {
        &self.consensus_secret_key
    }

    /// Returns the service public key of the current node.
    pub fn service_public_key(&self) -> &PublicKey {
        &self.service_public_key
    }

    /// Returns the service secret key of the current node.
    pub fn service_secret_key(&self) -> &SecretKey {
        &self.service_secret_key
    }

    /// Returns the leader id for the specified round and current height.
    pub fn leader(&self, round: Round) -> ValidatorId {
        let height: u64 = self.height().into();
        let round: u64 = round.into();
        ValidatorId(((height + round) % (self.validators().len() as u64)) as u16)
    }

    /// Updates known round for a validator and returns
    /// a new actual round if at least one non byzantine validators is guaranteed to be on a higher round.
    /// Otherwise returns None.
    pub fn update_validator_round(&mut self, id: ValidatorId, round: Round) -> Option<Round> {
        // Update known round.
        {
            let known_round = self.validators_rounds.entry(id).or_insert_with(Round::zero);
            if round <= *known_round {
                // keep only maximum round
                trace!(
                    "Received a message from a lower round than we know already,\
                     message_round = {},\
                     known_round = {}.",
                    round,
                    known_round
                );
                return None;
            }
            *known_round = round;
        }

        // Find highest non-byzantine round.
        // At max we can have (N - 1) / 3 byzantine nodes.
        // It is calculated via rounded up integer division.
        let max_byzantine_count = (self.validators().len() + 2) / 3 - 1;
        if self.validators_rounds.len() <= max_byzantine_count {
            trace!("Count of validators, lower then max byzantine count.");
            return None;
        }

        let mut rounds: Vec<_> = self.validators_rounds.iter().map(|(_, v)| v).collect();
        rounds.sort_unstable_by(|a, b| b.cmp(a));

        if *rounds[max_byzantine_count] > self.round {
            Some(*rounds[max_byzantine_count])
        } else {
            None
        }
    }

    /// Returns the height for a validator identified by the public key.
    pub fn node_height(&self, key: &PublicKey) -> Height {
        *self.nodes_max_height.get(key).unwrap_or(&Height::zero())
    }

    /// Updates known height for a validator identified by the public key.
    pub fn set_node_height(&mut self, key: PublicKey, height: Height) {
        *self
            .nodes_max_height
            .entry(key)
            .or_insert_with(Height::zero) = height;
    }

    /// Returns a list of nodes whose height is bigger than one of the current node.
    pub fn nodes_with_bigger_height(&self) -> Vec<&PublicKey> {
        self.nodes_max_height
            .iter()
            .filter(|&(_, h)| *h > self.height())
            .map(|(v, _)| v)
            .collect()
    }

    /// Returns sufficient number of votes for current validators number.
    pub fn majority_count(&self) -> usize {
        Self::byzantine_majority_count(self.validators().len())
    }

    /// Returns sufficient number of votes for the given validators number.
    pub fn byzantine_majority_count(total: usize) -> usize {
        total * 2 / 3 + 1
    }

    /// Returns current height.
    pub fn height(&self) -> Height {
        self.height
    }

    /// Returns start time of the current height.
    pub fn height_start_time(&self) -> SystemTime {
        self.height_start_time
    }

    /// Returns the current round.
    pub fn round(&self) -> Round {
        self.round
    }

    /// Returns a hash of the last block.
    pub fn last_hash(&self) -> &Hash {
        &self.last_hash
    }

    /// Locks the node to the specified round and propose hash.
    ///
    /// # Panics
    ///
    /// Panics if the current "locked round" is bigger or equal to the new one.
    pub fn lock(&mut self, round: Round, hash: Hash) {
        if self.locked_round >= round {
            panic!("Incorrect lock")
        }
        self.locked_round = round;
        self.locked_propose = Some(hash);
    }

    /// Returns locked round number. Zero means that the node is not locked to any round.
    pub fn locked_round(&self) -> Round {
        self.locked_round
    }

    /// Returns propose hash on which the node makes lock.
    pub fn locked_propose(&self) -> Option<Hash> {
        self.locked_propose
    }

    /// Returns mutable propose state identified by hash.
    pub fn propose_mut(&mut self, hash: &Hash) -> Option<&mut ProposeState> {
        self.proposes.get_mut(hash)
    }
    /// Returns propose state identified by hash.
    pub fn propose(&self, hash: &Hash) -> Option<&ProposeState> {
        self.proposes.get(hash)
    }

    /// Returns a block with the specified hash.
    pub fn block(&self, hash: &Hash) -> Option<&BlockState> {
        self.blocks.get(hash)
    }

    /// Updates mode's round.
    pub fn jump_round(&mut self, round: Round) {
        self.round = round;
    }

    /// Increments node's round by one.
    pub fn new_round(&mut self) {
        self.round.increment();
    }

    /// Return incomplete block.
    pub fn incomplete_block(&self) -> Option<&IncompleteBlock> {
        self.incomplete_block.as_ref()
    }

    /// Increments the node height by one and resets previous height data.
    pub fn new_height(&mut self, block_hash: &Hash, height_start_time: SystemTime) {
        self.height.increment();
        self.height_start_time = height_start_time;
        self.round = Round::first();
        self.locked_round = Round::zero();
        self.locked_propose = None;
        self.last_hash = *block_hash;
        // TODO: Destruct/construct structure HeightState instead of call clear. (ECR-171)
        self.blocks.clear();
        self.proposes.clear();
        self.unknown_proposes_with_precommits.clear();
        self.prevotes.clear();
        self.precommits.clear();
        self.validators_rounds.clear();
        if let Some(ref mut validator_state) = self.validator_state {
            validator_state.clear();
        }
        self.requests.clear(); // FIXME: Clear all timeouts. (ECR-171)
        self.incomplete_block = None;
    }

    /// Returns a list of queued consensus messages.
    pub fn queued(&mut self) -> Vec<ConsensusMessage> {
        let mut queued = Vec::new();
        ::std::mem::swap(&mut self.queued, &mut queued);
        queued
    }

    /// Add consensus message to the queue.
    pub fn add_queued(&mut self, msg: ConsensusMessage) {
        self.queued.push(msg);
    }

    /// Checks whether some proposes are waiting for this transaction.
    /// Returns a list of proposes that don't contain unknown transactions.
    ///
    /// Transaction is ignored if the following criteria are fulfilled:
    ///
    /// - transaction isn't contained in unknown transaction list of any propose
    /// - transaction isn't a part of block
    pub fn check_incomplete_proposes(&mut self, tx_hash: Hash) -> Vec<(Hash, Round)> {
        let mut full_proposes = Vec::new();
        for (propose_hash, propose_state) in &mut self.proposes {
            propose_state.unknown_txs.remove(&tx_hash);
            if propose_state.unknown_txs.is_empty() {
                full_proposes.push((*propose_hash, propose_state.message().round()));
            }
        }

        full_proposes
    }

    /// Checks if there is an incomplete block that waits for this transaction.
    /// Returns a block that don't contain unknown transactions.
    ///
    /// Transaction is ignored if the following criteria are fulfilled:
    ///
    /// - transaction isn't contained in the unknown transactions list of block
    /// - transaction isn't a part of block
    pub fn remove_unknown_transaction(&mut self, tx_hash: Hash) -> Option<IncompleteBlock> {
        if let Some(ref mut incomplete_block) = self.incomplete_block {
            incomplete_block.unknown_txs.remove(&tx_hash);
            if incomplete_block.unknown_txs.is_empty() {
                return Some(incomplete_block.clone());
            }
        }
        None
    }

    /// Returns pre-votes for the specified round and propose hash.
    pub fn prevotes(&self, round: Round, propose_hash: Hash) -> &[Signed<Prevote>] {
        self.prevotes
            .get(&(round, propose_hash))
            .map_or_else(|| [].as_ref(), |votes| votes.messages().as_slice())
    }

    /// Returns pre-commits for the specified round and propose hash.
    pub fn precommits(&self, round: Round, propose_hash: Hash) -> &[Signed<Precommit>] {
        self.precommits
            .get(&(round, propose_hash))
            .map_or_else(|| [].as_ref(), |votes| votes.messages().as_slice())
    }

    /// Returns `true` if this node has pre-vote for the specified round.
    ///
    /// # Panics
    ///
    /// Panics if this method is called for a non-validator node.
    pub fn have_prevote(&self, propose_round: Round) -> bool {
        if let Some(ref validator_state) = *self.validator_state() {
            validator_state.have_prevote(propose_round)
        } else {
            panic!("called have_prevote for auditor node")
        }
    }

    /// Adds propose from this node to the proposes list for the current height. Such propose
    /// cannot contain unknown transactions. Returns hash of the propose.
    pub fn add_self_propose(&mut self, msg: Signed<Propose>) -> Hash {
        debug_assert!(self.validator_state().is_some());
        let propose_hash = msg.hash();
        self.proposes.insert(
            propose_hash,
            ProposeState {
                propose: msg,
                unknown_txs: HashSet::new(),
                block_hash: None,
                // TODO: For the moment it's true because this code gets called immediately after
                // saving a propose to the cache. Think about making this approach less error-prone.
                // (ECR-1635)
                is_saved: true,
            },
        );

        propose_hash
    }

    /// Adds propose from other node. Returns `ProposeState` if it is a new propose.
    pub fn add_propose<S: AsRef<dyn Snapshot>>(
        &mut self,
        msg: Signed<Propose>,
        transactions: &MapIndex<S, Hash, Signed<RawTransaction>>,
        transaction_pool: &KeySetIndex<S, Hash>,
    ) -> Result<&ProposeState, failure::Error> {
        let propose_hash = msg.hash();
        match self.proposes.entry(propose_hash) {
            Entry::Occupied(..) => bail!("Propose already found"),
            Entry::Vacant(e) => {
                let mut unknown_txs = HashSet::new();
                for hash in msg.transactions() {
                    if transactions.get(hash).is_some() {
                        if !transaction_pool.contains(hash) {
                            bail!(
                                "Received propose with already \
                                 committed transaction"
                            )
                        }
                    } else {
                        unknown_txs.insert(*hash);
                    }
                }

                for tx in &unknown_txs {
                    self.unknown_txs
                        .entry(*tx)
                        .or_insert_with(Vec::new)
                        .push(propose_hash);
                }

                Ok(e.insert(ProposeState {
                    propose: msg,
                    unknown_txs,
                    block_hash: None,
                    is_saved: false,
                }))
            }
        }
    }

    /// Adds block to the list of blocks for the current height. Returns `BlockState` if it is a
    /// new block.
    pub fn add_block(
        &mut self,
        block_hash: Hash,
        patch: Patch,
        txs: Vec<Hash>,
        proposer_id: ValidatorId,
    ) -> Option<&BlockState> {
        match self.blocks.entry(block_hash) {
            Entry::Occupied(..) => None,
            Entry::Vacant(e) => Some(e.insert(BlockState {
                hash: block_hash,
                patch,
                txs,
                proposer_id,
            })),
        }
    }

    /// Finds unknown transactions in the block and persists transactions along
    /// with other info as a pending block.
    ///
    ///  # Panics
    ///
    /// - Already there is an incomplete block.
    /// - Received block has already committed transaction.
    pub fn create_incomplete_block<S: AsRef<dyn Snapshot>>(
        &mut self,
        msg: &Signed<BlockResponse>,
        txs: &MapIndex<S, Hash, Signed<RawTransaction>>,
        txs_pool: &KeySetIndex<S, Hash>,
    ) -> &IncompleteBlock {
        assert!(self.incomplete_block().is_none());

        let mut unknown_txs = HashSet::new();
        for hash in msg.transactions() {
            if txs.get(hash).is_some() {
                if !txs_pool.contains(hash) {
                    panic!(
                        "Received block with already \
                         committed transaction"
                    )
                }
            } else {
                unknown_txs.insert(*hash);
            }
        }

        self.incomplete_block = Some(IncompleteBlock {
            msg: msg.clone(),
            unknown_txs,
        });

        self.incomplete_block().unwrap()
    }

    /// Adds pre-vote. Returns `true` there are +2/3 pre-votes.
    ///
    /// # Panics
    ///
    /// A node panics if it has already sent a different `Prevote` for the same round.
    pub fn add_prevote(&mut self, msg: Signed<Prevote>) -> bool {
        let majority_count = self.majority_count();
        if let Some(ref mut validator_state) = self.validator_state {
            if validator_state.id == msg.validator() {
                if let Some(other) = validator_state
                    .our_prevotes
                    .insert(msg.round(), msg.clone())
                {
                    if other != msg {
                        panic!(
                            "Trying to send different prevotes for the same round: \
                             old = {:?}, new = {:?}",
                            other, msg
                        );
                    }
                }
            }
        }

        let key = (msg.round(), *msg.propose_hash());
        let validators_len = self.validators().len();
        let votes = self
            .prevotes
            .entry(key)
            .or_insert_with(|| Votes::new(validators_len));
        votes.insert(msg);
        votes.count() >= majority_count
    }

    /// Returns `true` if there are +2/3 pre-votes for the specified round and hash.
    pub fn has_majority_prevotes(&self, round: Round, propose_hash: Hash) -> bool {
        match self.prevotes.get(&(round, propose_hash)) {
            Some(votes) => votes.count() >= self.majority_count(),
            None => false,
        }
    }

    /// Returns ids of validators that that sent pre-votes for the specified propose.
    pub fn known_prevotes(&self, round: Round, propose_hash: &Hash) -> BitVec {
        let len = self.validators().len();
        self.prevotes
            .get(&(round, *propose_hash))
            .map_or_else(|| BitVec::from_elem(len, false), |x| x.validators().clone())
    }

    /// Returns ids of validators that that sent pre-commits for the specified propose.
    pub fn known_precommits(&self, round: Round, propose_hash: &Hash) -> BitVec {
        let len = self.validators().len();
        self.precommits
            .get(&(round, *propose_hash))
            .map_or_else(|| BitVec::from_elem(len, false), |x| x.validators().clone())
    }

    /// Adds pre-commit. Returns `true` there are +2/3 pre-commits.
    ///
    /// # Panics
    ///
    /// A node panics if it has already sent a different `Precommit` for the same round.
    pub fn add_precommit(&mut self, msg: Signed<Precommit>) -> bool {
        let majority_count = self.majority_count();
        if let Some(ref mut validator_state) = self.validator_state {
            if validator_state.id == msg.validator() {
                if let Some(other) = validator_state
                    .our_precommits
                    .insert(msg.round(), msg.clone())
                {
                    if other.propose_hash() != msg.propose_hash() {
                        panic!(
                            "Trying to send different precommits for same round, old={:?}, \
                             new={:?}",
                            other, msg
                        );
                    }
                }
            }
        }

        let key = (msg.round(), *msg.block_hash());
        let validators_len = self.validators().len();
        let votes = self
            .precommits
            .entry(key)
            .or_insert_with(|| Votes::new(validators_len));
        votes.insert(msg);
        votes.count() >= majority_count
    }

    /// Adds unknown (for this node) propose.
    pub fn add_unknown_propose_with_precommits(
        &mut self,
        round: Round,
        propose_hash: Hash,
        block_hash: Hash,
    ) {
        self.unknown_proposes_with_precommits
            .entry(propose_hash)
            .or_insert_with(Vec::new)
            .push((round, block_hash));
    }

    /// Removes propose from the list of unknown proposes and returns its round and hash.
    pub fn take_unknown_propose_with_precommits(
        &mut self,
        propose_hash: &Hash,
    ) -> Vec<(Round, Hash)> {
        self.unknown_proposes_with_precommits
            .remove(propose_hash)
            .unwrap_or_default()
    }

    /// Returns true if the node has +2/3 pre-commits for the specified round and block hash.
    pub fn has_majority_precommits(&self, round: Round, block_hash: Hash) -> bool {
        match self.precommits.get(&(round, block_hash)) {
            Some(votes) => votes.count() >= self.majority_count(),
            None => false,
        }
    }

    /// Returns `true` if the node doesn't have proposes different from the locked one.
    pub fn have_incompatible_prevotes(&self) -> bool {
        for round in self.locked_round.next().iter_to(self.round.next()) {
            match self.validator_state {
                Some(ref validator_state) => {
                    if let Some(msg) = validator_state.our_prevotes.get(&round) {
                        // TODO: Inefficient. (ECR-171)
                        if Some(*msg.propose_hash()) != self.locked_propose {
                            return true;
                        }
                    }
                }
                None => unreachable!(),
            }
        }
        false
    }

    /// Adds data-request to the queue. Returns `true` if it is a new request.
    pub fn request(&mut self, data: RequestData, peer: PublicKey) -> bool {
        let state = self.requests.entry(data).or_insert_with(RequestState::new);
        let is_new = state.is_empty();
        state.insert(peer);
        is_new
    }

    /// Returns public key of a peer that has required information. Returned key is removed from
    /// the corresponding validators list, so next time request will be sent to a different peer.
    pub fn retry(&mut self, data: &RequestData, peer: Option<PublicKey>) -> Option<PublicKey> {
        let next = {
            let state = if let Some(state) = self.requests.get_mut(data) {
                state
            } else {
                return None;
            };
            if let Some(peer) = peer {
                state.remove(&peer);
            }
            state.peek()
        };

        if next.is_none() {
            self.requests.remove(data);
        };
        next
    }

    /// Removes the specified request from the pending request list.
    pub fn remove_request(&mut self, data: &RequestData) -> HashSet<PublicKey> {
        let state = self.requests.remove(data);
        state.map(|s| s.known_nodes).unwrap_or_default()
    }

    /// Returns the `Connect` message of the current node.
    pub fn our_connect_message(&self) -> &Signed<Connect> {
        &self.our_connect_message
    }

    /// Updates the `Connect` message of the current node.
    pub fn set_our_connect_message(&mut self, msg: Signed<Connect>) {
        self.our_connect_message = msg;
    }

    /// Add peer to node's `ConnectList`.
    pub fn add_peer_to_connect_list(&mut self, peer: ConnectInfo) {
        let mut list = self
            .connect_list
            .inner
            .write()
            .expect("ConnectList write lock");
        list.add(peer);
    }
}
