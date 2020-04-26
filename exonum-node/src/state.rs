// Copyright 2020 The Exonum Team
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

use anyhow::bail;
use bit_vec::BitVec;
use exonum::{
    blockchain::{
        Block, BlockKind, BlockPatch, ConsensusConfig, PersistentPool, TransactionCache,
        ValidatorKeys,
    },
    crypto::{Hash, PublicKey},
    helpers::{byzantine_quorum, Height, Milliseconds, Round, ValidatorId},
    keys::Keys,
    merkledb::{access::RawAccess, KeySetIndex, MapIndex, ObjectHash, Snapshot},
    messages::{AnyTx, Precommit, Verified},
};
use log::{error, trace};

use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap, HashSet},
    sync::{Arc, RwLock},
    time::{Duration, SystemTime},
};

use crate::{
    connect_list::ConnectList,
    consensus::RoundAction,
    events::network::ConnectedPeerAddr,
    messages::{Connect, Consensus as ConsensusMessage, Prevote, Propose, Status},
    Configuration, ConnectInfo, FlushPoolStrategy,
};

// TODO: Move request timeouts into node configuration. (ECR-171)

/// Timeout value for the `ProposeRequest` message.
pub const PROPOSE_REQUEST_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `TransactionsRequest` message.
pub const TRANSACTIONS_REQUEST_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `PrevotesRequest` message.
pub const PREVOTES_REQUEST_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `BlockRequest` message.
pub const BLOCK_REQUEST_TIMEOUT: Milliseconds = 100;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PeerState {
    pub epoch: Height,
    pub blockchain_height: Height,
}

impl PeerState {
    pub fn new(status: &Status) -> Self {
        Self {
            epoch: status.epoch,
            blockchain_height: status.blockchain_height,
        }
    }
}

impl Default for PeerState {
    fn default() -> Self {
        Self {
            epoch: Height::zero(),
            blockchain_height: Height::zero(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct AdvancedPeers {
    pub peers_with_greater_height: Vec<PublicKey>,
    pub peers_with_greater_epoch: Vec<PublicKey>,
}

impl AdvancedPeers {
    /// Forms a message to send to a connected peer to query a (pseudo-)block with
    /// a larger height / epoch.
    pub fn send_message(&self, state: &State) -> Option<(PublicKey, RequestData)> {
        // If there are any peers with known greater blockchain height, sent a request
        // to one of them.
        let block_height = state.blockchain_height();
        for peer in &self.peers_with_greater_height {
            if state.peers().contains_key(peer) {
                return Some((*peer, RequestData::Block(block_height)));
            }
        }

        // If there are no peers at the greater height, but there are peers with a greater epoch,
        // send a request to one of them.
        let data = RequestData::BlockOrEpoch {
            block_height,
            epoch: state.epoch(),
        };
        for peer in &self.peers_with_greater_epoch {
            if state.peers().contains_key(peer) {
                return Some((*peer, data));
            }
        }

        None
    }
}

/// State of the `NodeHandler`.
#[derive(Debug)]
pub(crate) struct State {
    validator_state: Option<ValidatorState>,
    our_connect_message: Verified<Connect>,

    config: ConsensusConfig,
    connect_list: SharedConnectList,

    peers: HashMap<PublicKey, Verified<Connect>>,
    connections: HashMap<PublicKey, ConnectedPeerAddr>,
    epoch_start_time: SystemTime,
    epoch: Height,
    blockchain_height: Height,

    round: Round,
    locked_round: Round,
    locked_propose: Option<Hash>,
    last_hash: Hash,

    // Messages.
    proposes: HashMap<Hash, ProposeState>,
    blocks: HashMap<Hash, BlockState>,
    prevotes: HashMap<(Round, Hash), Votes<Verified<Prevote>>>,
    precommits: HashMap<(Round, Hash), Votes<Verified<Precommit>>>,

    queued: Vec<ConsensusMessage>,

    unknown_txs: HashMap<Hash, Vec<Hash>>,
    proposes_confirmed_by_majority: HashMap<Hash, (Round, Hash)>,

    // Our requests state.
    requests: HashMap<RequestData, RequestState>,

    // Maximum of node epoch / height in consensus messages.
    peer_states: BTreeMap<PublicKey, PeerState>,
    validators_rounds: BTreeMap<ValidatorId, Round>,

    incomplete_block: Option<IncompleteBlock>,

    // Cache that stores transactions before adding to persistent pool.
    tx_cache: BTreeMap<Hash, Verified<AnyTx>>,
    flush_pool_strategy: FlushPoolStrategy,

    // An in-memory set of transaction hashes, rejected by a node
    // within block.
    //
    // Those transactions are stored to be known if some node will propose a block
    // with one of them, so node could lookup for it.
    //
    // This set is cleared every block.
    //
    // TODO: This may be a vector for DoS attacks by memory exhaustion. [ECR-2067]
    invalid_txs: HashSet<Hash>,

    keys: Keys,
}

/// State of a validator node.
#[derive(Debug, Clone)]
pub(crate) struct ValidatorState {
    id: ValidatorId,
    our_prevotes: HashMap<Round, Verified<Prevote>>,
    our_precommits: HashMap<Round, Verified<Precommit>>,
}

/// `RequestData` represents a request for some data to other nodes. Each enum variant will be
/// translated to the corresponding request-message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RequestData {
    /// Represents `ProposeRequest` message.
    Propose(Hash),
    /// Represents `PoolTransactionsRequest` message.
    PoolTransactions,
    /// Represents `TransactionsRequest` message for `Propose`.
    ProposeTransactions(Hash),
    /// Represents `TransactionsRequest` message for `BlockResponse`.
    BlockTransactions,
    /// Represents `PrevotesRequest` message.
    Prevotes(Round, Hash),
    /// Represents `BlockRequest` message.
    Block(Height),
    /// Represents `BlockRequest` message with `epoch` field set. This is used to request
    /// a block at the next height or a block skip with a successive epoch.
    BlockOrEpoch { block_height: Height, epoch: Height },
}

#[derive(Debug)]
struct RequestState {
    // Number of attempts made.
    retries: u16,
    // Nodes that have the required information.
    known_nodes: HashSet<PublicKey>,
}

/// `ProposeState` represents the state of some propose and is used for tracking of unknown
/// transactions.
#[derive(Debug)]
pub struct ProposeState {
    propose: Verified<Propose>,
    unknown_txs: HashSet<Hash>,
    /// Hash of the block corresponding to the `Propose`, if the block has been executed.
    block_hash: Option<Hash>,
    /// Whether the message has been saved to the consensus messages' cache or not.
    is_saved: bool,
    /// Whether the propose contains invalid transactions or not.
    is_valid: bool,
}

/// State of a block.
#[derive(Debug)]
pub struct BlockState {
    hash: Hash,
    // Changes that should be made for block committing.
    patch: Option<BlockPatch>,
    txs: Vec<Hash>,
    proposer_id: ValidatorId,
    kind: BlockKind,
    epoch: Height,
}

/// Incomplete block.
#[derive(Clone, Debug)]
pub struct IncompleteBlock {
    pub header: Block,
    pub precommits: Vec<Verified<Precommit>>,
    pub transactions: Vec<Hash>,
    unknown_txs: HashSet<Hash>,
}

/// `VoteMessage` trait represents voting messages such as `Precommit` and `Prevote`.
pub trait VoteMessage: Clone {
    /// Return validator if of the message.
    fn validator(&self) -> ValidatorId;
}

impl VoteMessage for Verified<Precommit> {
    fn validator(&self) -> ValidatorId {
        self.payload().validator
    }
}

impl VoteMessage for Verified<Prevote> {
    fn validator(&self) -> ValidatorId {
        self.payload().validator
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
            Self::Propose(..) => PROPOSE_REQUEST_TIMEOUT,
            Self::ProposeTransactions(..) | Self::BlockTransactions | Self::PoolTransactions => {
                TRANSACTIONS_REQUEST_TIMEOUT
            }
            Self::Prevotes(..) => PREVOTES_REQUEST_TIMEOUT,
            Self::Block(..) | Self::BlockOrEpoch { .. } => BLOCK_REQUEST_TIMEOUT,
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
    /// Returns kind of the block proposed by this `Propose` message.
    pub fn block_kind(&self) -> BlockKind {
        if self.propose.payload().skip {
            BlockKind::Skip
        } else {
            BlockKind::Normal
        }
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
    pub fn message(&self) -> &Verified<Propose> {
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

    /// Returns `true` if there are invalid transactions in the propose.
    pub fn has_invalid_txs(&self) -> bool {
        !self.is_valid
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
    /// Returns block kind.
    pub fn kind(&self) -> BlockKind {
        self.kind
    }

    /// Returns the epoch that the block belongs to.
    pub fn epoch(&self) -> Height {
        self.epoch
    }

    /// Returns the changes that should be made for block committing.
    pub fn patch(&mut self) -> BlockPatch {
        self.patch.take().expect("Patch is already committed")
    }

    /// Returns hashes of the transactions in the block.
    pub fn txs(&self) -> &[Hash] {
        &self.txs
    }

    /// Returns id of the validator that proposed the block.
    pub fn proposer_id(&self) -> ValidatorId {
        self.proposer_id
    }
}

impl IncompleteBlock {
    pub fn new(
        header: Block,
        transactions: Vec<Hash>,
        precommits: Vec<Verified<Precommit>>,
    ) -> Self {
        Self {
            header,
            transactions,
            precommits,
            unknown_txs: HashSet::new(),
        }
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

/// Shared `ConnectList` representation to be used in network.
#[derive(Clone, Debug, Default)]
pub(crate) struct SharedConnectList {
    inner: Arc<RwLock<ConnectList>>,
}

impl SharedConnectList {
    /// Creates `SharedConnectList` from `ConnectList`.
    pub fn from_connect_list(connect_list: ConnectList) -> Self {
        Self {
            inner: Arc::new(RwLock::new(connect_list)),
        }
    }

    /// Returns `true` if a peer with the given public key can connect.
    pub(crate) fn is_peer_allowed(&self, public_key: &PublicKey) -> bool {
        let connect_list = self.inner.read().expect("ConnectList read lock");
        connect_list.is_peer_allowed(public_key)
    }

    /// Return `peers` from the underlying `ConnectList`.
    pub(crate) fn peers(&self) -> Vec<ConnectInfo> {
        let connect_list = self.inner.read().expect("ConnectList read lock");

        connect_list
            .peers
            .iter()
            .map(|(pk, addr)| ConnectInfo {
                address: addr.to_owned(),
                public_key: *pk,
            })
            .collect()
    }

    /// Update peer address in the connect list.
    pub(super) fn update_peer(&mut self, public_key: &PublicKey, address: String) {
        let mut conn_list = self.inner.write().expect("ConnectList write lock");
        conn_list.update_peer(public_key, address);
    }

    /// Get peer address using public key.
    pub(crate) fn find_address_by_key(&self, public_key: &PublicKey) -> Option<String> {
        let connect_list = self.inner.read().expect("ConnectList read lock");
        connect_list
            .find_address_by_pubkey(public_key)
            .map(str::to_string)
    }
}

impl State {
    /// Creates state with the given parameters.
    pub fn new(
        config: Configuration,
        consensus_config: ConsensusConfig,
        connect: Connect,
        peers: HashMap<PublicKey, Verified<Connect>>,
        last_block: &Block,
        last_block_skip: Option<&Block>,
        epoch_start_time: SystemTime,
    ) -> Self {
        let validator_id = consensus_config
            .validator_keys
            .iter()
            .position(|pk| pk.consensus_key == config.keys.consensus_pk());

        let our_connect_message = Verified::from_value(
            connect,
            config.keys.consensus_pk(),
            config.keys.consensus_sk(),
        );

        let last_epoch = last_block_skip
            .map_or_else(|| last_block.epoch(), Block::epoch)
            .expect("No `epoch` recorded in the saved block");

        Self {
            validator_state: validator_id.map(|id| ValidatorState::new(ValidatorId(id as u16))),
            connect_list: SharedConnectList::from_connect_list(config.connect_list),
            peers,
            connections: HashMap::new(),
            epoch: last_epoch.next(),
            epoch_start_time,
            blockchain_height: last_block.height.next(),
            round: Round::zero(),
            locked_round: Round::zero(),
            locked_propose: None,
            last_hash: last_block.object_hash(),

            proposes: HashMap::new(),
            blocks: HashMap::new(),
            prevotes: HashMap::new(),
            precommits: HashMap::new(),

            queued: Vec::new(),

            unknown_txs: HashMap::new(),
            proposes_confirmed_by_majority: HashMap::new(),

            peer_states: BTreeMap::new(),
            validators_rounds: BTreeMap::new(),

            our_connect_message,

            requests: HashMap::new(),
            config: consensus_config,

            incomplete_block: None,
            tx_cache: BTreeMap::new(),
            flush_pool_strategy: config.mempool.flush_pool_strategy,
            invalid_txs: HashSet::default(),

            keys: config.keys,
        }
    }

    /// Returns `ValidatorState` if the node is validator.
    fn validator_state(&self) -> &Option<ValidatorState> {
        &self.validator_state
    }

    /// Returns validator id of the node if it is a validator. Returns `None` otherwise.
    pub(crate) fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_state.as_ref().map(ValidatorState::id)
    }

    /// Updates the validator id. If there hasn't been `ValidatorState` for that id, then a new
    /// state will be created.
    fn renew_validator_id(&mut self, id: Option<ValidatorId>) {
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
    pub(super) fn is_validator(&self) -> bool {
        self.validator_state().is_some()
    }

    /// Checks if the node is a leader for the current height and round.
    pub fn is_leader(&self) -> bool {
        self.validator_state()
            .as_ref()
            .map_or(false, |validator| self.leader(self.round()) == validator.id)
    }

    /// Returns a connect list of the node.
    pub fn connect_list(&self) -> SharedConnectList {
        self.connect_list.clone()
    }

    /// Returns public (consensus and service) keys of known validators.
    pub(crate) fn validators(&self) -> &[ValidatorKeys] {
        &self.config.validator_keys
    }

    /// Returns `ConsensusConfig`.
    pub fn config(&self) -> &ConsensusConfig {
        &self.config
    }

    /// Returns `ConsensusConfig`.
    pub(super) fn consensus_config(&self) -> &ConsensusConfig {
        &self.config
    }

    /// Replaces `ConsensusConfig` with a new one and updates validator ID of the current node
    /// if the new config is different from the previous one.
    pub fn update_config(&mut self, config: ConsensusConfig) {
        if self.config == config {
            return;
        }

        trace!("Updating node config={:#?}", config);
        let validator_id = config
            .validator_keys
            .iter()
            .position(|pk| pk.consensus_key == self.keys().consensus_pk())
            .map(|id| ValidatorId(id as u16));

        // TODO: update connect list (ECR-1745)

        self.renew_validator_id(validator_id);
        trace!("Validator={:#?}", self.validator_state());

        self.config = config;
    }

    /// Adds the public key, address, and `Connect` message of a validator.
    pub(super) fn add_peer(&mut self, pubkey: PublicKey, msg: Verified<Connect>) -> bool {
        self.peers.insert(pubkey, msg).is_none()
    }

    /// Add connection to the connection list.
    pub(super) fn add_connection(&mut self, pubkey: PublicKey, address: ConnectedPeerAddr) {
        self.connections.insert(pubkey, address);
    }

    /// Removes a peer by the socket address. Returns `Some` (connect message) of the peer if it was
    /// indeed connected or `None` if there was no connection with given socket address.
    pub(super) fn remove_peer_with_pubkey(&mut self, key: &PublicKey) -> Option<Verified<Connect>> {
        self.connections.remove(key);
        if let Some(c) = self.peers.remove(key) {
            Some(c)
        } else {
            None
        }
    }

    /// Checks if this node considers a peer to be a validator.
    pub(super) fn peer_is_validator(&self, pubkey: &PublicKey) -> bool {
        self.config
            .validator_keys
            .iter()
            .any(|x| &x.consensus_key == pubkey)
    }

    /// Checks if a peer is in this node's connection list.
    pub(super) fn peer_in_connect_list(&self, pubkey: &PublicKey) -> bool {
        self.connect_list.is_peer_allowed(pubkey)
    }

    /// Returns the keys of known peers with their `Connect` messages.
    pub fn peers(&self) -> &HashMap<PublicKey, Verified<Connect>> {
        &self.peers
    }

    /// Returns the addresses of known connections with public keys of its' validators.
    pub(crate) fn connections(&self) -> &HashMap<PublicKey, ConnectedPeerAddr> {
        &self.connections
    }

    /// Returns public key of a validator identified by id.
    pub(super) fn consensus_public_key_of(&self, id: ValidatorId) -> Option<PublicKey> {
        let id: usize = id.into();
        self.validators().get(id).map(|x| x.consensus_key)
    }

    /// Returns the keys of this node.
    pub fn keys(&self) -> &Keys {
        &self.keys
    }

    /// Returns the leader id for the specified round and current height.
    pub fn leader(&self, round: Round) -> ValidatorId {
        let epoch: u64 = self.epoch().into();
        let round: u64 = round.into();
        ValidatorId(((epoch + round) % (self.validators().len() as u64)) as u16)
    }

    /// Updates known round for a validator and returns
    /// a new actual round if at least one non byzantine validators is guaranteed to be on a higher round.
    /// Otherwise returns None.
    pub(super) fn update_validator_round(
        &mut self,
        id: ValidatorId,
        round: Round,
    ) -> Option<Round> {
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

    /// Updates known height / epoch for a validator identified by the public key.
    pub(super) fn update_peer_state(&mut self, key: PublicKey, new_state: PeerState) {
        let current_state = self.peer_states.entry(key).or_default();
        if current_state.epoch < new_state.epoch {
            if current_state.blockchain_height <= new_state.blockchain_height {
                *current_state = new_state;
            } else {
                log::warn!(
                    "Node {:?} has provided inconsistent `Status`: previously known \
                     peer state was {:?}, and the new one is {:?}",
                    key,
                    current_state,
                    new_state
                );
            }
        }
    }

    /// Returns a list of nodes whose height is bigger than one of the current node.
    pub(super) fn advanced_peers(&self) -> AdvancedPeers {
        let mut peers_with_greater_height = vec![];
        let mut peers_with_greater_epoch = vec![];
        for (&key, state) in &self.peer_states {
            if state.blockchain_height > self.blockchain_height {
                peers_with_greater_height.push(key);
            } else if state.epoch > self.epoch {
                peers_with_greater_epoch.push(key);
            }
        }
        AdvancedPeers {
            peers_with_greater_height,
            peers_with_greater_epoch,
        }
    }

    /// Returns sufficient number of votes for current validators number.
    pub(crate) fn majority_count(&self) -> usize {
        byzantine_quorum(self.validators().len())
    }

    /// Returns current epoch of the consensus algorithm.
    pub fn epoch(&self) -> Height {
        self.epoch
    }

    pub fn blockchain_height(&self) -> Height {
        self.blockchain_height
    }

    /// Returns the start time of the current consensus epoch.
    pub(super) fn epoch_start_time(&self) -> SystemTime {
        self.epoch_start_time
    }

    /// Sets the start time of the current consensus epoch.
    pub(super) fn set_epoch_start_time(&mut self, time: SystemTime) {
        self.epoch_start_time = time;
    }

    /// Returns the current round.
    pub fn round(&self) -> Round {
        self.round
    }

    /// Returns a hash of the last block.
    pub(super) fn last_hash(&self) -> Hash {
        self.last_hash
    }

    /// Locks the node to the specified round and propose hash.
    ///
    /// # Panics
    ///
    /// Panics if the current "locked round" is bigger or equal to the new one.
    pub(super) fn lock(&mut self, round: Round, hash: Hash) {
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
    pub(super) fn propose_mut(&mut self, hash: &Hash) -> Option<&mut ProposeState> {
        self.proposes.get_mut(hash)
    }

    /// Returns propose state identified by hash.
    pub(super) fn propose(&self, hash: &Hash) -> Option<&ProposeState> {
        self.proposes.get(hash)
    }

    /// Returns a block with the specified hash.
    pub(super) fn block(&self, hash: &Hash) -> Option<&BlockState> {
        self.blocks.get(hash)
    }

    pub(super) fn take_block_for_commit(&mut self, hash: &Hash) -> BlockState {
        self.blocks
            .remove(hash)
            .expect("Cannot retrieve block for commit")
    }

    /// Updates mode's round.
    pub(super) fn jump_round(&mut self, round: Round) {
        self.round = round;
    }

    /// Increments node's round by one.
    pub(super) fn new_round(&mut self) {
        self.round.increment();
    }

    /// Return incomplete block.
    pub(super) fn incomplete_block(&self) -> Option<&IncompleteBlock> {
        self.incomplete_block.as_ref()
    }

    /// Returns a saved block that was just completed.
    pub(super) fn take_completed_block(&mut self) -> IncompleteBlock {
        let block = self.incomplete_block.take().expect("No saved block");
        debug_assert!(!block.has_unknown_txs());
        block
    }

    /// Updates the node epoch and resets previous epoch data.
    pub(super) fn new_epoch(&mut self, new_epoch: Height, epoch_start_time: SystemTime) {
        debug_assert!(new_epoch > self.epoch);

        self.epoch = new_epoch;
        self.epoch_start_time = epoch_start_time;
        self.round = Round::first();
        self.locked_round = Round::zero();
        self.locked_propose = None;
        // TODO: Destruct/construct structure HeightState instead of call clear. (ECR-171)
        self.blocks.clear();
        self.proposes.clear();
        self.proposes_confirmed_by_majority.clear();
        self.prevotes.clear();
        self.precommits.clear();
        self.validators_rounds.clear();
        if let Some(ref mut validator_state) = self.validator_state {
            validator_state.clear();
        }
        self.requests.clear(); // FIXME: Clear all timeouts. (ECR-171)
        self.incomplete_block = None;
    }

    /// Increments the node height by one together with entering a new epoch.
    pub(super) fn new_height(
        &mut self,
        block_hash: Hash,
        new_epoch: Height,
        epoch_start_time: SystemTime,
    ) {
        self.new_epoch(new_epoch, epoch_start_time);
        self.blockchain_height.increment();
        self.last_hash = block_hash;
        self.invalid_txs.clear();
    }

    /// Returns a list of queued consensus messages.
    pub(super) fn queued(&mut self) -> Vec<ConsensusMessage> {
        let mut queued = Vec::new();
        std::mem::swap(&mut self.queued, &mut queued);
        queued
    }

    /// Add consensus message to the queue.
    pub(super) fn add_queued(&mut self, msg: ConsensusMessage) {
        self.queued.push(msg);
    }

    /// Checks whether some proposes are waiting for this transaction.
    /// Returns a list of proposes that don't contain unknown transactions.
    ///
    /// Transaction is ignored if the following criteria are fulfilled:
    ///
    /// - transaction isn't contained in unknown transaction list of any propose
    /// - transaction isn't a part of block
    pub(super) fn check_incomplete_proposes(&mut self, tx_hash: Hash) -> Vec<(Hash, Round)> {
        let mut full_proposes = Vec::new();
        for (propose_hash, propose_state) in &mut self.proposes {
            let contained_tx = propose_state.unknown_txs.remove(&tx_hash);
            if contained_tx && self.invalid_txs.contains(&tx_hash) {
                // Mark prevote with newly received invalid transaction as invalid.
                propose_state.is_valid = false;
            }

            if propose_state.unknown_txs.is_empty() {
                let round = propose_state.message().payload().round;
                full_proposes.push((*propose_hash, round));
            }
        }

        // Depending on the build type and amount of proposes, we may want
        // to reorder proposes. See comments in both implementations of
        // `reorder_proposes_if_needed` to get details.
        Self::reorder_proposes_if_needed(&mut full_proposes);

        full_proposes
    }

    #[cfg(debug_assertions)]
    fn reorder_proposes_if_needed(full_proposes: &mut Vec<(Hash, Round)>) {
        // For tests we don't care about DoS attacks, so (unlike the release
        // version) we *always* sort by both round *and hash*.
        // This is useful for tests to have proposes in a predictable order.
        full_proposes.sort_unstable_by(|(hash1, round1), (hash2, round2)| {
            // Compare rounds first.
            // Note that we call `cmp` on `round2` to obtain descending order.
            let cmp_result = round2.cmp(round1);
            if let std::cmp::Ordering::Equal = cmp_result {
                // Rounds are equal, compare by hash (in direct order,
                // since it doesn't affect anything).
                hash1.cmp(hash2)
            } else {
                // Rounds are different, use the comparison result.
                cmp_result
            }
        });
    }

    #[cfg(not(debug_assertions))]
    fn reorder_proposes_if_needed(full_proposes: &mut Vec<(Hash, Round)>) {
        // Since it's more likely to commit a propose with greater round,
        // it makes sense to process proposes ordered descendingly by the
        // round number.
        // However, if we have a lot of proposes, the overhead of sorting
        // can become significant, and we don't want to create a space for
        // DoS attack. Thus the maximum amount of proposes for which
        // sorting is applied is limited.
        // Despite that, the limit is big enough to won't be achieved within
        // normal blockchain functioning.

        // TODO: Clarify the value for this constant (ECR-4050).
        const MAX_PROPOSES_AMOUNT_FOR_SORTING: usize = 10;

        if full_proposes.len() <= MAX_PROPOSES_AMOUNT_FOR_SORTING {
            full_proposes.sort_unstable_by(|(_, round1), (_, round2)| {
                // Note that we call `cmp` on `round2` to obtain descending order.
                // Unlike debug version, we don't sort by hash.
                round2.cmp(&round1)
            });
        }
    }

    /// Checks if there is an incomplete block that waits for this transaction.
    ///
    /// Returns `NewHeight` if the locally saved block has been completed (i.e., the node can
    /// commit it now), `None` otherwise.
    ///
    /// Transaction is ignored if the following criteria are fulfilled:
    ///
    /// - transaction isn't contained in the unknown transactions list of block
    /// - transaction isn't a part of block
    ///
    /// # Panics
    ///
    /// Panics if transaction for incomplete block is known as invalid.
    pub(super) fn remove_unknown_transaction(&mut self, tx_hash: Hash) -> RoundAction {
        if let Some(ref mut incomplete_block) = self.incomplete_block {
            if self.invalid_txs.contains(&tx_hash) {
                panic!("Received a block with transaction known as invalid");
            }

            incomplete_block.unknown_txs.remove(&tx_hash);
            if incomplete_block.unknown_txs.is_empty() {
                return RoundAction::NewEpoch;
            }
        }
        RoundAction::None
    }

    /// Returns pre-votes for the specified round and propose hash.
    pub(super) fn prevotes(&self, round: Round, propose_hash: Hash) -> &[Verified<Prevote>] {
        self.prevotes
            .get(&(round, propose_hash))
            .map_or_else(|| [].as_ref(), |votes| votes.messages().as_slice())
    }

    /// Returns pre-commits for the specified round and propose hash.
    pub(super) fn precommits(&self, round: Round, propose_hash: Hash) -> &[Verified<Precommit>] {
        self.precommits
            .get(&(round, propose_hash))
            .map_or_else(|| [].as_ref(), |votes| votes.messages().as_slice())
    }

    /// Returns `true` if this node has pre-vote for the specified round.
    ///
    /// # Panics
    ///
    /// Panics if this method is called for a non-validator node.
    pub(super) fn have_prevote(&self, propose_round: Round) -> bool {
        if let Some(ref validator_state) = *self.validator_state() {
            validator_state.have_prevote(propose_round)
        } else {
            panic!("called have_prevote for auditor node")
        }
    }

    /// Adds propose from this node to the proposes list for the current height. Such propose
    /// cannot contain unknown transactions. Returns hash of the propose.
    pub(super) fn add_self_propose(&mut self, msg: Verified<Propose>) -> Hash {
        debug_assert!(self.validator_state().is_some());
        let propose_hash = msg.object_hash();
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
                // We expect ourself not to produce invalid proposes.
                is_valid: true,
            },
        );

        propose_hash
    }

    /// Adds propose from other node. Returns `ProposeState` if it is a new propose.
    pub(super) fn add_propose<T: RawAccess>(
        &mut self,
        msg: Verified<Propose>,
        transactions: &MapIndex<T, Hash, Verified<AnyTx>>,
        transaction_pool: &KeySetIndex<T, Hash>,
    ) -> anyhow::Result<&ProposeState> {
        let propose_hash = msg.object_hash();
        match self.proposes.entry(propose_hash) {
            Entry::Occupied(..) => bail!("Propose already found"),
            Entry::Vacant(e) => {
                let mut is_valid = true;
                let mut unknown_txs = HashSet::new();
                for hash in &msg.payload().transactions {
                    if self.tx_cache.contains_key(hash) {
                        // Tx with `hash` is  not committed yet.
                        continue;
                    }

                    if transactions.contains(hash) {
                        if !transaction_pool.contains(hash) {
                            bail!("Received propose with already committed transaction");
                        }
                    } else if self.invalid_txs.contains(hash) {
                        // If the propose contains an invalid transaction,
                        // we don't stop processing, since we expect this propose to
                        // be declined by the consensus rules.
                        error!(
                            "Received propose {:?} with transaction {:?} known as invalid",
                            msg.payload(),
                            hash
                        );
                        is_valid = false;
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
                    is_valid,
                }))
            }
        }
    }

    /// Adds block to the collection of known blocks.
    pub(super) fn add_block(
        &mut self,
        patch: BlockPatch,
        txs: Vec<Hash>,
        proposer_id: ValidatorId,
        epoch: Height,
    ) {
        let block_hash = patch.block_hash();
        let kind = patch.kind();
        self.blocks.entry(block_hash).or_insert(BlockState {
            hash: block_hash,
            patch: Some(patch),
            txs,
            proposer_id,
            kind,
            epoch,
        });
    }

    /// Finds unknown transactions in the block and persists transactions along
    /// with other info as a pending block.
    ///
    ///  # Panics
    ///
    /// - Already there is an incomplete block.
    /// - Received block has already committed transaction.
    /// - Block contains a transaction that is incorrect.
    pub(super) fn create_incomplete_block(
        &mut self,
        mut incomplete_block: IncompleteBlock,
        snapshot: &dyn Snapshot,
        txs_pool: &KeySetIndex<&dyn Snapshot, Hash>,
    ) -> &IncompleteBlock {
        assert!(self.incomplete_block().is_none());
        let tx_cache = PersistentPool::new(snapshot, &self.tx_cache);

        for hash in &incomplete_block.transactions {
            if tx_cache.contains_transaction(*hash) {
                if !self.tx_cache.contains_key(hash) && !txs_pool.contains(hash) {
                    panic!("Received block with already committed transaction");
                }
            } else if self.invalid_txs.contains(hash) {
                panic!("Received a block with transaction known as invalid");
            } else {
                incomplete_block.unknown_txs.insert(*hash);
            }
        }

        self.incomplete_block = Some(incomplete_block);
        self.incomplete_block().unwrap()
    }

    /// Adds pre-vote. Returns `true` there are +2/3 pre-votes.
    ///
    /// # Panics
    ///
    /// A node panics if it has already sent a different `Prevote` for the same round.
    pub(super) fn add_prevote(&mut self, msg: Verified<Prevote>) -> bool {
        let majority_count = self.majority_count();
        if let Some(ref mut validator_state) = self.validator_state {
            if validator_state.id == msg.validator() {
                if let Some(other) = validator_state
                    .our_prevotes
                    .insert(msg.payload().round, msg.clone())
                {
                    // Our node should not ever send two different prevotes within one round.
                    assert_eq!(
                        other, msg,
                        "Trying to send different prevotes for the same round"
                    )
                }
            }
        }

        let key = (msg.payload().round, msg.payload().propose_hash);
        let validators_len = self.validators().len();
        let votes = self
            .prevotes
            .entry(key)
            .or_insert_with(|| Votes::new(validators_len));
        votes.insert(msg);
        votes.count() >= majority_count
    }

    /// Returns `true` if there are +2/3 pre-votes for the specified round and hash.
    pub(super) fn has_majority_prevotes(&self, round: Round, propose_hash: Hash) -> bool {
        match self.prevotes.get(&(round, propose_hash)) {
            Some(votes) => votes.count() >= self.majority_count(),
            None => false,
        }
    }

    /// Returns ids of validators that that sent pre-votes for the specified propose.
    pub(super) fn known_prevotes(&self, round: Round, propose_hash: Hash) -> BitVec {
        let len = self.validators().len();
        self.prevotes
            .get(&(round, propose_hash))
            .map_or_else(|| BitVec::from_elem(len, false), |x| x.validators().clone())
    }

    /// Adds pre-commit. Returns `true` there are +2/3 pre-commits.
    ///
    /// # Panics
    ///
    /// A node panics if it has already sent a different `Precommit` for the same round.
    pub(super) fn add_precommit(&mut self, msg: Verified<Precommit>) -> bool {
        let majority_count = self.majority_count();
        if let Some(ref mut validator_state) = self.validator_state {
            if validator_state.id == msg.validator() {
                if let Some(other) = validator_state
                    .our_precommits
                    .insert(msg.payload().round, msg.clone())
                {
                    if other.payload().propose_hash != msg.payload().propose_hash {
                        panic!(
                            "Trying to send different precommits for same round, old={:?}, \
                             new={:?}",
                            other, msg
                        );
                    }
                }
            }
        }

        let key = (msg.payload().round, msg.payload().block_hash);
        let validators_len = self.validators().len();
        let votes = self
            .precommits
            .entry(key)
            .or_insert_with(|| Votes::new(validators_len));
        votes.insert(msg);
        votes.count() >= majority_count
    }

    /// Adds a propose that was confirmed by a majority of
    /// validator nodes without our participation.
    pub(super) fn add_propose_confirmed_by_majority(
        &mut self,
        round: Round,
        propose_hash: Hash,
        block_hash: Hash,
    ) {
        let old_value = self
            .proposes_confirmed_by_majority
            .insert(propose_hash, (round, block_hash));

        debug_assert!(
            old_value.map_or(true, |val| val == (round, block_hash)),
            "Attempt to add another propose confirmed by majority"
        );
    }

    /// Removes a propose from the list of unknown proposes and returns its round and hash.
    pub(super) fn take_confirmed_propose(&mut self, propose_hash: &Hash) -> Option<(Round, Hash)> {
        self.proposes_confirmed_by_majority.remove(propose_hash)
    }

    /// Returns true if the node has +2/3 pre-commits for the specified round and block hash.
    pub(super) fn has_majority_precommits(&self, round: Round, block_hash: Hash) -> bool {
        match self.precommits.get(&(round, block_hash)) {
            Some(votes) => votes.count() >= self.majority_count(),
            None => false,
        }
    }

    /// Returns `true` if the node doesn't have proposes different from the locked one.
    pub(super) fn have_incompatible_prevotes(&self) -> bool {
        for round in self.locked_round.next().iter_to(self.round.next()) {
            match self.validator_state {
                Some(ref validator_state) => {
                    if let Some(msg) = validator_state.our_prevotes.get(&round) {
                        // TODO: Inefficient. (ECR-171)
                        if Some(msg.payload().propose_hash) != self.locked_propose {
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
    pub(super) fn request(&mut self, data: RequestData, peer: PublicKey) -> bool {
        let state = self.requests.entry(data).or_insert_with(RequestState::new);
        let is_new = state.is_empty();
        state.insert(peer);
        is_new
    }

    /// Returns public key of a peer that has required information. Returned key is removed from
    /// the corresponding validators list, so next time request will be sent to a different peer.
    pub(super) fn retry(
        &mut self,
        data: &RequestData,
        peer: Option<PublicKey>,
    ) -> Option<PublicKey> {
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
    pub(super) fn remove_request(&mut self, data: &RequestData) -> HashSet<PublicKey> {
        let state = self.requests.remove(data);
        state.map(|s| s.known_nodes).unwrap_or_default()
    }

    /// Returns the `Connect` message of the current node.
    pub(super) fn our_connect_message(&self) -> &Verified<Connect> {
        &self.our_connect_message
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

    /// Returns the transactions cache length.
    pub fn tx_cache_len(&self) -> usize {
        self.tx_cache.len()
    }

    /// Returns reference to the transactions cache.
    pub fn tx_cache(&self) -> &BTreeMap<Hash, Verified<AnyTx>> {
        &self.tx_cache
    }

    /// Returns mutable reference to the transactions cache.
    pub(super) fn tx_cache_mut(&mut self) -> &mut BTreeMap<Hash, Verified<AnyTx>> {
        &mut self.tx_cache
    }

    /// Returns interval between flushing transaction pool to the database, if any.
    pub(super) fn flush_pool_timeout(&self) -> Option<Duration> {
        match self.flush_pool_strategy {
            FlushPoolStrategy::Timeout { timeout } => Some(Duration::from_millis(timeout)),
            _ => None,
        }
    }

    /// Checks if the pool flushing strategy prescribes to flush transactions immediately
    /// on initial processing.
    pub(super) fn persist_txs_immediately(&self) -> bool {
        match self.flush_pool_strategy {
            FlushPoolStrategy::Immediate => true,
            _ => false,
        }
    }

    /// Returns mutable reference to the invalid transactions cache.
    pub(super) fn invalid_txs_mut(&mut self) -> &mut HashSet<Hash> {
        &mut self.invalid_txs
    }
}
