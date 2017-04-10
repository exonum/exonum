//! # Introduction
//! This crate implements the standalone configuration service of `Exonum` blockchain, which, upon being plugged in, allows modifying 
//! `Exonum` blockchain configuration by means of [propose config](struct.TxConfigPropose.html) and [vote for proposed config](struct.TxConfigVote.html) transactions, signed by validators - actual blockchain
//! participants. 
//! 
//! It also contains http api implementation for public queries (get actual/following
//! configuration, etc.) and private queries, intended for use only by validator nodes' admins (post configuration propose, post vote for a
//! configuration propose).
//! 
//! `Exonum` blockchain configuration is composed of: 
//! 
//! - consensus algorithm parameters 
//! - list of validators' public keys - list of identities of consensus participants
//! - configuration of all services, plugged in for a specific blockchain instance. 
//! 
//! It also contains auxiliary fields: 
//! 
//! - actual_from - blockchain height, upon reaching which current config is to become actual. 
//! - previous_cfg_hash - hash of previous configuration, which validators' set is allowed to cast
//! votes for current config.
//! 
//! For details see [StoredConfiguration in exonum](../exonum/blockchain/config/struct.StoredConfiguration.html)
//!
//! # Examples
//!
//! Run `Exonum` blockchain testnet with single configuration service turned on for it in a single process (2 threads per node: 1 - for exonum node and 1 - for http api listener)
//!
//! ```rust,no_run
//! extern crate iron;
//! extern crate tempdir;
//! extern crate router;
//! extern crate exonum;
//! extern crate blockchain_explorer;
//! extern crate configuration_service;
//!
//! use std::thread;
//!
//! use std::net::SocketAddr;
//! use tempdir::TempDir;
//! use router::Router;
//!
//! use exonum::blockchain::Blockchain;
//! use exonum::node::Node;
//! use exonum::storage::{LevelDB, LevelDBOptions};
//! use blockchain_explorer::api::Api;
//! use blockchain_explorer::helpers::generate_testnet_config;
//! use configuration_service::{ConfigurationService};
//! use configuration_service::config_api::{PublicConfigApi, PrivateConfigApi};
//!
//! fn main() {
//!     // Init crypto engine and pretty logger.
//!     exonum::crypto::init();
//!     blockchain_explorer::helpers::init_logger().unwrap();
//!
//!     // Blockchain params
//!     let count = 4;
//!     // Inner exonum network start port (4000, 4001, 4002, ..)
//!     let start_port = 4000;
//!     // External http api port (8000, 8001, 8002, ...)
//!     let api_port = 8000;
//!     let tmpdir_handle = TempDir::new("exonum_configuration").unwrap();
//!     let destdir = tmpdir_handle.path();
//!
//!     // Generate blockchain configuration
//!     let node_cfgs = generate_testnet_config(count, start_port);
//!
//!     // Create testnet threads
//!     let node_threads = {
//!         let mut node_threads = Vec::new();
//!         for idx in 0..count as usize {
//!             // Create configuration service for node[idx]
//!             let service = ConfigurationService::new();
//!             // Create database for node[idx]
//!             let db = {
//!                 let mut options = LevelDBOptions::new();
//!                 let path = destdir.join(idx.to_string());
//!                 options.create_if_missing = true;
//!                 LevelDB::new(&path, options).expect("Unable to create database")
//!             };
//!             // Create node[idx]
//!             let blockchain = Blockchain::new(db, vec![Box::new(service)]);
//!             let mut node = Node::new(blockchain.clone(), node_cfgs[idx].clone());
//!             let channel_clone = node.channel().clone();
//!             let node_thread = thread::spawn(move || {
//!                                                 // Run it in separate thread
//!                                                 node.run().expect("Unable to run node");
//!                                             });
//!             node_threads.push(node_thread);
//!
//!             let node_cfg = node_cfgs[idx].clone();
//!             // Create node api thread
//!             let api_thread = thread::spawn(move || {
//!
//!                 let private_config_api = PrivateConfigApi {
//!                     channel: channel_clone,
//!                     config: (node_cfg.public_key, node_cfg.secret_key),
//!                 };
//!
//!                 let public_config_api = PublicConfigApi {
//!                     blockchain: blockchain,
//!                 };
//!
//!                 let listen_address: SocketAddr =
//!                     format!("127.0.0.1:{}", api_port+idx).parse().unwrap();
//!
//!                 let mut router = Router::new();
//!                 private_config_api.wire(&mut router);
//!                 public_config_api.wire(&mut router);
//!                 let chain = iron::Chain::new(router);
//!                 iron::Iron::new(chain).http(listen_address).unwrap();
//!             });
//!
//!             node_threads.push(api_thread);
//!         }
//!         node_threads
//!     };
//!
//!     for node_thread in node_threads {
//!         node_thread.join().unwrap();
//!     }
//! }
//! ```
//!
#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#[macro_use]
extern crate exonum;
extern crate blockchain_explorer;
#[macro_use]
extern crate log;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate iron;
extern crate router;
extern crate bodyparser;
extern crate params;
#[macro_use]
extern crate lazy_static;

/// Configuration service http api.
pub mod config_api;
use std::fmt;

use serde::{Serialize, Serializer, Deserialize, Deserializer};

use exonum::messages::Field;
use exonum::blockchain::{Service, Transaction, Schema, NodeState};
use exonum::node::State;
use exonum::crypto::{Signature, PublicKey, hash, Hash, HASH_SIZE};
use exonum::messages::utils::U64;
use exonum::messages::{RawMessage, Message, FromRaw, RawTransaction, Error as MessageError};
use exonum::storage::{StorageValue, List, Map, View, MapTable, MerkleTable, MerklePatriciaTable,
                      Result as StorageResult};
use exonum::blockchain::StoredConfiguration;

///Value of [service_id of ConfigurationService](struct.ConfigurationService.html#method.service_id) 
pub const CONFIG_SERVICE: u16 = 1;
///Value of [message_type](../exonum/messages/struct.MessageBuffer.html#method.message_type) of
///[TxConfigPropose](struct.TxConfigPropose.html)
pub const CONFIG_PROPOSE_MESSAGE_ID: u16 = 0;
///Value of [message_type](../exonum/messages/struct.MessageBuffer.html#method.message_type) of
///[TxConfigVote](struct.TxConfigVote.html)
pub const CONFIG_VOTE_MESSAGE_ID: u16 = 1;

lazy_static! {
#[doc="
Specific [TxConfigVote](TxConfigVote.t.html) with all bytes in message set to 0. 
It is used as placeholder in database for votes of validators, which didn't cast votes."]
    pub static ref ZEROVOTE: TxConfigVote = TxConfigVote::new_with_signature(&PublicKey::zero(), &Hash::zero(), &Signature::zero());
}

storage_value! {
    StorageValueConfigProposeData {
        const SIZE = 48;

        tx_propose:            TxConfigPropose   [00 => 8]
        votes_history_hash:    &Hash             [8 => 40]
        num_votes:             u64               [40 => 48]
    }
}

/// This structure logically contains 2 fields: 
/// 
/// 1 - [TxConfigPropose](TxConfigPropose.t.html) in [tx_propose](#method.tx_propose) field.
/// 
/// 2 - reference to [table with all validators' votes for a config propose](struct.ConfigurationSchema.html#method.config_votes), indexed by validator_id `u64` and containing values of [TxConfigVote](TxConfigVote.t.html) type. 
/// 
/// Length of the table is stored in [num_votes](#method.num_votes) field, which isn't changed
/// after table initialization, because number of possible vote slots for a config is determined by
/// number of validators in its previous config. 
/// 
/// Table's root hash - in [votes_history_hash](#method.votes_history_hash) field, which is
/// modified after a vote from validator is added.
impl StorageValueConfigProposeData {
///Method to mutate [votes_history_hash](#method.votes_history_hash) field containing root hash of [table with all validators' votes for a config propose](struct.ConfigurationSchema.html#method.config_votes) after replacing [empty
///vote](struct.ZEROVOTE.html) with a real [TxConfigVote](TxConfigVote.t.html) cast by a validator.
    pub fn set_history_hash(&mut self, hash: &Hash) {
        Field::write(&hash, &mut self.raw, 8, 40);
    }
}

#[derive(Serialize, Deserialize)]
struct StorageValueConfigProposeDataSerdeHelper {
    tx_propose: TxConfigPropose,
    votes_history_hash: Hash,
    num_votes: U64,
}

impl Serialize for StorageValueConfigProposeData {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let helper = StorageValueConfigProposeDataSerdeHelper {
            tx_propose: self.tx_propose(),
            votes_history_hash: *self.votes_history_hash(),
            num_votes: U64(self.num_votes()),
        };
        helper.serialize(ser)
    }
}

impl Deserialize for StorageValueConfigProposeData {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let h = <StorageValueConfigProposeDataSerdeHelper>::deserialize(deserializer)?;

        let precommit =
            StorageValueConfigProposeData::new(h.tx_propose, &h.votes_history_hash, h.num_votes.0);
        Ok(precommit)
    }
}

message! {
    TxConfigPropose {
        const TYPE = CONFIG_SERVICE;
        const ID = CONFIG_PROPOSE_MESSAGE_ID;
        const SIZE = 40;

        from:           &PublicKey  [00 => 32]
        cfg:            &[u8]       [32 => 40] // serialized config bytes
    }
}

message! {
    TxConfigVote {
        const TYPE = CONFIG_SERVICE;
        const ID = CONFIG_VOTE_MESSAGE_ID;
        const SIZE = 64;

        from:           &PublicKey  [00 => 32]
        cfg_hash:       &Hash       [32 => 64] // hash of config we're voting for
    }
}

#[derive(Clone, PartialEq)]
pub enum ConfigTx {
    ConfigPropose(TxConfigPropose),
    ConfigVote(TxConfigVote),
}

#[derive(Deserialize)]
struct TxConfigProposeSerdeHelper {
    from: PublicKey,
    cfg: StoredConfiguration,
    signature: Signature,
}

#[derive(Deserialize)]
struct TxConfigVoteSerdeHelper {
    from: PublicKey,
    cfg_hash: Hash,
    signature: Signature,
}

impl Serialize for TxConfigPropose {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state;
        state = ser.serialize_struct("config_propose", 3)?;
        ser.serialize_struct_elt(&mut state, "from", self.from())?;
        if let Ok(cfg) = StoredConfiguration::try_deserialize(self.cfg()) {
            ser.serialize_struct_elt(&mut state, "cfg", cfg)?;
        } else {
            ser.serialize_struct_elt(&mut state, "cfg", self.cfg())?;
        }
        ser.serialize_struct_elt(&mut state, "signature", self.raw().signature())?;
        ser.serialize_struct_end(state)
    }
}

impl Deserialize for TxConfigPropose {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let h = <TxConfigProposeSerdeHelper>::deserialize(deserializer)?;

        let precommit = TxConfigPropose::new_with_signature(&h.from,
                                                            &StorageValue::serialize(h.cfg
                                                                .clone()),
                                                            &h.signature);
        Ok(precommit)
    }
}

impl Serialize for TxConfigVote {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state;
        state = ser.serialize_struct("vote", 3)?;
        ser.serialize_struct_elt(&mut state, "from", self.from())?;
        ser.serialize_struct_elt(&mut state, "cfg_hash", self.cfg_hash())?;
        ser.serialize_struct_elt(&mut state, "signature", self.raw().signature())?;
        ser.serialize_struct_end(state)
    }
}

impl Deserialize for TxConfigVote {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let h = <TxConfigVoteSerdeHelper>::deserialize(deserializer)?;

        let precommit = TxConfigVote::new_with_signature(&h.from, &h.cfg_hash, &h.signature);
        Ok(precommit)
    }
}

impl Serialize for ConfigTx {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        match *self {
            ConfigTx::ConfigPropose(ref propose) => propose.serialize(ser),
            ConfigTx::ConfigVote(ref vote) => vote.serialize(ser),
        }
    }
}

#[derive(Default)]
pub struct ConfigurationService {}

pub struct ConfigurationSchema<'a> {
    view: &'a View,
}

impl ConfigTx {
    pub fn from(&self) -> &PublicKey {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.from(),
            ConfigTx::ConfigVote(ref msg) => msg.from(),
        }
    }
}

impl fmt::Debug for ConfigTx {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => write!(fmt, "{:?}", msg),
            ConfigTx::ConfigVote(ref msg) => write!(fmt, "{:?}", msg),
        }
    }
}

impl FromRaw for ConfigTx {
    fn from_raw(raw: RawMessage) -> Result<ConfigTx, MessageError> {
        match raw.message_type() {
            CONFIG_PROPOSE_MESSAGE_ID => {
                Ok(ConfigTx::ConfigPropose(TxConfigPropose::from_raw(raw)?))
            }
            CONFIG_VOTE_MESSAGE_ID => Ok(ConfigTx::ConfigVote(TxConfigVote::from_raw(raw)?)),
            _ => Err(MessageError::IncorrectMessageType { message_type: raw.message_type() }),
        }
    }
}

impl Message for ConfigTx {
    fn raw(&self) -> &RawMessage {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.raw(),
            ConfigTx::ConfigVote(ref msg) => msg.raw(),
        }
    }

    fn verify_signature(&self, public_key: &PublicKey) -> bool {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => msg.verify_signature(public_key),
            ConfigTx::ConfigVote(ref msg) => msg.verify_signature(public_key),
        }
    }

    fn hash(&self) -> Hash {
        match *self {
            ConfigTx::ConfigPropose(ref msg) => Message::hash(msg),
            ConfigTx::ConfigVote(ref msg) => Message::hash(msg),
        }
    }
}

impl<'a> ConfigurationSchema<'a> {
    pub fn new(view: &'a View) -> ConfigurationSchema {
        ConfigurationSchema { view: view }
    }

    /// mapping hash(config) -> TxConfigPropose
    pub fn config_data
        (&self)
         -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, Hash, StorageValueConfigProposeData> {
        MerklePatriciaTable::new(MapTable::new(vec![4], self.view))
    }
    /// mapping validator_id -> TxConfigVote
    pub fn config_votes(&self,
                    config_hash: &Hash)
                    -> MerkleTable<MapTable<View, [u8], Vec<u8>>, u64, TxConfigVote> {
        let mut prefix = vec![5; 1 + HASH_SIZE];
        prefix[1..].copy_from_slice(config_hash.as_ref());
        MerkleTable::new(MapTable::new(prefix, self.view))
    }

    pub fn put_propose(&self,
                       cfg_hash: &Hash,
                       tx_propose: TxConfigPropose,
                       num_validators: u64)
                       -> StorageResult<()> {
        let votes_table = self.config_votes(cfg_hash);
        debug_assert!(votes_table.is_empty().unwrap());
        for _ in 0..(num_validators as usize) {
            votes_table.append(ZEROVOTE.clone())?;
        }
        let config_data =
            StorageValueConfigProposeData::new(tx_propose, &votes_table.root_hash()?, num_validators);
        let config_data_table = self.config_data();
        debug_assert!(config_data_table.get(cfg_hash).unwrap().is_none());
        config_data_table.put(cfg_hash, config_data)
    }

    pub fn get_propose(&self, cfg_hash: &Hash) -> StorageResult<Option<TxConfigPropose>> {
        let option_config_data = self.config_data().get(cfg_hash)?;
        Ok(option_config_data.map(|config_data| config_data.tx_propose()))
    }

    pub fn put_vote(&self, tx_vote: TxConfigVote) -> StorageResult<()> {
        let cfg_hash = tx_vote.cfg_hash();
        let config_data_table = self.config_data();
        let mut propose_config_data = config_data_table.get(cfg_hash)?
            .expect(&format!("Corresponding propose unexpectedly not found for TxConfigVote:{:?}",
                             &tx_vote));

        let tx_propose = propose_config_data.tx_propose();
        let prev_cfg_hash = <StoredConfiguration as StorageValue>::deserialize(tx_propose.cfg().to_vec())
            .previous_cfg_hash;
        let general_schema = Schema::new(self.view);
        let prev_cfg = general_schema.configs()
            .get(&prev_cfg_hash)?
            .expect(&format!("Previous cfg:{:?} unexpectedly not found for TxConfigVote:{:?}",
                             prev_cfg_hash,
                             &tx_vote));
        let from: &PublicKey = tx_vote.from();
        let validator_id = prev_cfg.validators
            .iter()
            .position(|pk| pk == from)
            .expect(&format!("See !prev_cfg.validators.contains(self.from()) for \
                              TxConfigVote:{:?}",
                             &tx_vote));

        let votes_for_cfg_table = self.config_votes(cfg_hash);
        votes_for_cfg_table.set(validator_id as u64, tx_vote.clone())?;
        propose_config_data.set_history_hash(&votes_for_cfg_table.root_hash()?);
        config_data_table.put(cfg_hash, propose_config_data)
    }

    pub fn get_votes(&self, cfg_hash: &Hash) -> StorageResult<Vec<Option<TxConfigVote>>> {
        let votes_table = self.config_votes(cfg_hash);
        let votes_values = votes_table.values()?;
        let votes_options = votes_values.into_iter()
            .map(|vote| {
                if vote == ZEROVOTE.clone() {
                    None
                } else {
                    Some(vote)
                }
            })
            .collect::<Vec<_>>();
        Ok(votes_options)
    }

    pub fn state_hash(&self) -> StorageResult<Vec<Hash>> {
        Ok(vec![self.config_data().root_hash()?])
    }
}

impl TxConfigPropose {
    fn execute(&self, view: &View) -> StorageResult<()> {
        let blockchain_schema = Schema::new(view);
        let config_schema = ConfigurationSchema::new(view);

        let following_config: Option<StoredConfiguration> =
            blockchain_schema.get_following_configuration()?;
        if let Some(foll_cfg) = following_config {
            error!("Discarding TxConfigPropose: {} as there is an already scheduled next config: \
                    {:?} ",
                   serde_json::to_string(self)?,
                   foll_cfg);
            return Ok(());
        }

        let actual_config: StoredConfiguration = blockchain_schema.get_actual_configuration()?;

        if !actual_config.validators.contains(self.from()) {
            error!("Discarding TxConfigPropose:{} from unknown validator. ",
                   serde_json::to_string(self)?);
            return Ok(());
        }

        let config_candidate = StoredConfiguration::try_deserialize(self.cfg());
        if config_candidate.is_err() {
            error!("Discarding TxConfigPropose:{} which contains config, which cannot be parsed: \
                    {:?}",
                   serde_json::to_string(self)?,
                   config_candidate);
            return Ok(());
        }

        let actual_config_hash = actual_config.hash();
        let config_candidate_body = config_candidate.unwrap();
        if config_candidate_body.previous_cfg_hash != actual_config_hash {
            error!("Discarding TxConfigPropose:{} which does not reference actual config: {:?}",
                   serde_json::to_string(self)?,
                   actual_config);
            return Ok(());
        }

        let current_height = blockchain_schema.current_height()?;
        let actual_from = config_candidate_body.actual_from;
        if actual_from <= current_height {
            error!("Discarding TxConfigPropose:{} which has actual_from height less than or \
                    equal to current: {:?}",
                   serde_json::to_string(self)?,
                   current_height);
            return Ok(());
        }

        let config_hash = config_candidate_body.hash();

        if let Some(tx_propose) = config_schema.get_propose(&config_hash)? {
            error!("Discarding TxConfigPropose:{} which contains an already posted config. \
                    Previous TxConfigPropose:{}",
                   serde_json::to_string(self)?,
                   serde_json::to_string(&tx_propose)?);
            return Ok(());
        }

        config_schema.put_propose(&config_hash,
                         self.clone(),
                         actual_config.validators.len() as u64)?;

        debug!("Put TxConfigPropose:{} to config_proposes table",
               serde_json::to_string(self)?);
        Ok(())
    }
}

impl TxConfigVote {
    fn execute(&self, view: &View) -> StorageResult<()> {
        let blockchain_schema = Schema::new(view);
        let config_schema = ConfigurationSchema::new(view);

        let propose_option = config_schema.get_propose(self.cfg_hash())?;
        if propose_option.is_none() {
            error!("Discarding TxConfigVote:{:?} which references unknown config hash",
                   self);
            return Ok(());
        }

        let following_config: Option<StoredConfiguration> =
            blockchain_schema.get_following_configuration()?;
        if let Some(foll_cfg) = following_config {
            error!("Discarding TxConfigVote: {:?} as there is an already scheduled next config: \
                    {:?} ",
                   self,
                   foll_cfg);
            return Ok(());
        }

        let actual_config: StoredConfiguration = blockchain_schema.get_actual_configuration()?;

        if !actual_config.validators.contains(self.from()) {
            error!("Discarding TxConfigVote:{:?} from unknown validator. ",
                   self);
            return Ok(());
        }

        let referenced_tx_propose = propose_option.unwrap();
        let parsed_config = StoredConfiguration::try_deserialize(referenced_tx_propose.cfg())
            .unwrap();
        let actual_config_hash = actual_config.hash();
        if parsed_config.previous_cfg_hash != actual_config_hash {
            error!("Discarding TxConfigVote:{:?}, whose corresponding TxConfigPropose:{} does \
                    not reference actual config: {:?}",
                   self,
                   serde_json::to_string(&referenced_tx_propose)?,
                   actual_config);
            return Ok(());
        }

        let current_height = blockchain_schema.current_height()?;
        let actual_from = parsed_config.actual_from;
        if actual_from <= current_height {
            error!("Discarding TxConfigVote:{:?}, whose corresponding TxConfigPropose:{} has \
                    actual_from height less than or equal to current: {:?}",
                   self,
                   serde_json::to_string(&referenced_tx_propose)?,
                   current_height);
            return Ok(());
        }

        config_schema.put_vote(self.clone())?;
        debug!("Put TxConfigVote:{:?} to corresponding cfg config_votes table",
               self);

        let mut votes_count = 0;

        for vote_option in config_schema.get_votes(self.cfg_hash())? {
            if vote_option.is_some() {
                votes_count += 1;
            }
        }

        if votes_count >= State::byzantine_majority_count(actual_config.validators.len()) {
            blockchain_schema.commit_actual_configuration(parsed_config)?;
        }
        Ok(())
    }
}

impl Transaction for ConfigTx {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, view: &View) -> StorageResult<()> {
        match *self {
            ConfigTx::ConfigPropose(ref tx) => tx.execute(view),
            ConfigTx::ConfigVote(ref tx) => tx.execute(view),
        }
    }
}

impl ConfigurationService {
    pub fn new() -> ConfigurationService {
        ConfigurationService {}
    }
}

impl Service for ConfigurationService {
    fn service_id(&self) -> u16 {
        CONFIG_SERVICE
    }

    fn state_hash(&self, view: &View) -> StorageResult<Vec<Hash>> {
        let schema = ConfigurationSchema::new(view);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        ConfigTx::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn handle_commit(&self, state: &mut NodeState) -> StorageResult<()> {
        let old_cfg = state.actual_config().clone();

        let new_cfg = {
            let schema = Schema::new(state.view());
            schema.get_actual_configuration()?
        };

        if new_cfg != old_cfg {
            info!("Updated node config={:#?}", new_cfg);
            state.update_config(new_cfg);
        }
        Ok(())
    }
}
