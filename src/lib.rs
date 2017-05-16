//! # Introduction
//! This crate implements the standalone configuration service of `Exonum` blockchain,
//! which, upon being plugged in, allows modifying
//! `Exonum` blockchain configuration by means of [propose config](struct.TxConfigPropose.html)
//! and [vote for proposed config](struct.TxConfigVote.html) transactions, signed by validators
//! - actual blockchain participants.
//!
//! It also contains http api implementation for public queries (get actual/following
//! configuration, etc.) and private queries, intended for use only by validator nodes' maintainers
//! (post configuration propose, post vote for a configuration propose).
//!
//! `Exonum` blockchain configuration is composed of:
//!
//! - consensus algorithm parameters
//! - list of validators' public keys - list of identities of consensus participants
//! - configuration of all services, plugged in for a specific blockchain instance.
//!
//! It also contains auxiliary fields:
//!
//! - `actual_from` - blockchain height, upon reaching which current config is to become actual.
//! - `previous_cfg_hash` - hash of previous configuration, which validators' set is allowed to cast
//! votes for current config.
//!
//! See [StoredConfiguration](../exonum/blockchain/config/struct.StoredConfiguration.html)
//! in exonum.
//!
//! While using the service's transactions and/or api, it's important to understand, how [hash of a
//! configuration](../exonum/blockchain/config/struct.StoredConfiguration.html#method.hash) is
//! calculated. It's calculated as a hash of normalized `String` bytes, containing
//! configuration json representation.
//! When a new propose is put via `TxConfigPropose`:
//!
//! 1. [bytes](struct.TxConfigPropose.html#method.cfg) of a `String`, containing configuration
//! json ->
//! 2. `String` ->
//! 3. `StoredConfiguration` ->
//! 4. unique normalized `String` for a unique configuration ->
//! 5. bytes ->
//! 6. [hash](../exonum/crypto/fn.hash.html)(bytes)
//!
//! The same [hash of a configuration]
//! (../exonum/blockchain/config/struct.StoredConfiguration.html#method.hash) is referenced in
//! `TxConfigVote` in [cfg_hash](struct.TxConfigVote.html#method.cfg_hash).
//!
//! # Examples
//!
//! Run `Exonum` blockchain testnet with single configuration service turned on for it in a
//! single process (2 threads per node: 1 - for exonum node and 1 - for http api listener)
//!
//! ```rust,no_run
//! extern crate iron;
//! extern crate env_logger;
//! extern crate clap;
//! extern crate serde;
//! extern crate serde_json;
//! extern crate bodyparser;

//! extern crate exonum;
//! extern crate router;
//! extern crate configuration_service;

//! use std::net::SocketAddr;
//! use clap::{Arg, App};

//! use exonum::blockchain::{Blockchain, Service};
//! use exonum::node::Node;
//! use exonum::helpers::clap::{GenerateCommand, RunCommand};
//! use exonum::helpers::{run_node_with_api, NodeRunOptions};

//! use configuration_service::ConfigurationService;

//! fn main() {
//!     exonum::crypto::init();
//!     exonum::helpers::init_logger().unwrap();

//!     let app = App::new("Simple configuration api demo program")
//!         .version(env!("CARGO_PKG_VERSION"))
//!         .author("Aleksey S. <aleksei.sidorov@xdev.re>")
//!         .about("Demo validator node")
//!         .subcommand(GenerateCommand::new())
//!         .subcommand(RunCommand::new()
//!                         .arg(Arg::with_name("CFG_PUB_HTTP_PORT")
//!                                  .short("p")
//!                                  .long("public-port")
//!                                  .value_name("CFG_PUB_HTTP_PORT")
//!                                  .help("Run public config api http server on given port")
//!                                  .takes_value(true))
//!                         .arg(Arg::with_name("CFG_PRIV_HTTP_PORT")
//!                                  .short("s")
//!                                  .long("private-port")
//!                                  .value_name("CFG_PRIV_HTTP_PORT")
//!                                  .help("Run config api http server on given port")
//!                                  .takes_value(true)));
//!     let matches = app.get_matches();

//!     match matches.subcommand() {
//!         ("generate", Some(matches)) => GenerateCommand::execute(matches),
//!         ("run", Some(matches)) => {
//!             let pub_port: Option<u16> = matches
//!                 .value_of("CFG_PUB_HTTP_PORT")
//!                 .map(|x| x.parse().unwrap());
//!             let priv_port: Option<u16> = matches
//!                 .value_of("CFG_PRIV_HTTP_PORT")
//!                 .map(|x| x.parse().unwrap());
//!             let node_cfg = RunCommand::node_config(matches);
//!             let db = RunCommand::db(matches);

//!             let services: Vec<Box<Service>> = vec![Box::new(ConfigurationService::new())];
//!             let blockchain = Blockchain::new(db, services);

//!             let node = Node::new(blockchain, node_cfg);
//!             let opts = NodeRunOptions {
//!                 enable_explorer: true,
//!                 public_api_address: pub_port
//!                     .map(|port| SocketAddr::from(([127, 0, 0, 1], port))),
//!                 private_api_address: priv_port
//!                     .map(|port| SocketAddr::from(([127, 0, 0, 1], port))),
//!             };
//!             run_node_with_api(node, opts);
//!         }
//!         _ => {
//!             unreachable!("Wrong subcommand");
//!         }
//!     }
//! }
//! ```
//!
#[macro_use]
extern crate exonum;
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
use router::Router;

use exonum::api::Api;
use exonum::messages::Field;
use exonum::blockchain::{Service, Transaction, Schema, ApiContext};
use exonum::node::State;
use exonum::crypto::{Signature, PublicKey, hash, Hash, HASH_SIZE};
use exonum::messages::utils::U64;
use exonum::messages::{RawMessage, Message, FromRaw, RawTransaction, Error as MessageError};
use exonum::storage::{StorageValue, List, Map, View, MapTable, MerkleTable, MerklePatriciaTable,
                      Result as StorageResult};
use exonum::blockchain::StoredConfiguration;

type ProposeData = StorageValueConfigProposeData;
/// Value of [service_id](struct.ConfigurationService.html#method.service_id) of
/// `ConfigurationService`
pub const CONFIG_SERVICE: u16 = 1;
/// Value of [message_type](../exonum/messages/struct.MessageBuffer.html#method.message_type) of
/// `TxConfigPropose`
pub const CONFIG_PROPOSE_MESSAGE_ID: u16 = 0;
/// Value of [message_type](../exonum/messages/struct.MessageBuffer.html#method.message_type) of
/// `TxConfigVote`
pub const CONFIG_VOTE_MESSAGE_ID: u16 = 1;

lazy_static! {
#[doc="
Specific [TxConfigVote](TxConfigVote.t.html) with all bytes in message set to 0.
It is used as placeholder in database for votes of validators, which didn't cast votes."]
    pub static ref ZEROVOTE: TxConfigVote = TxConfigVote::new_with_signature(&PublicKey::zero(),
    &Hash::zero(), &Signature::zero());
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
/// 1. `TxConfigPropose` in `tx_propose` field.
///
/// 2. Reference to
/// [votes_by_config_hash](struct.ConfigurationSchema.html#method.votes_by_config_hash) table. This
///    reference is represented by 2 fields:
///   - `votest_history_hash`
///   - `num_votes`
///
/// Length of the table is stored in `num_votes` field, which isn't changed
/// after table initialization, because number of possible vote slots for a config is determined by
/// number of validators in its previous config.
///
/// Table's root hash - in `votes_history_hash` field, which is
/// modified after a vote from validator is added.
impl StorageValueConfigProposeData {
    /// Method to mutate `votes_history_hash` field containing root hash of
    /// [votes_by_config_hash](struct.ConfigurationSchema.html#method.votes_by_config_hash)
    /// after replacing [empty
    /// vote](struct.ZEROVOTE.html) with a real `TxConfigVote` cast by a validator.
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
        cfg:            &[u8]       [32 => 40]
    }
}

message! {
    TxConfigVote {
        const TYPE = CONFIG_SERVICE;
        const ID = CONFIG_VOTE_MESSAGE_ID;
        const SIZE = 64;

        from:           &PublicKey  [00 => 32]
        cfg_hash:       &Hash       [32 => 64]
    }
}

/// Helper enum to aggregate `TxConfigPropose` and `TxConfigVote` within. This enum implements
/// `Exonum` traits, necessary for messages and transactions: `Message`, `FromRaw`, `Transaction`
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

        let precommit =
            TxConfigPropose::new_with_signature(&h.from,
                                                &StorageValue::serialize(h.cfg.clone()),
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

/// Struct, implementing [Service](../exonum/blockchain/service/trait.Service.html) trait template.
/// Most of the actual business logic of modifying `Exonum` blockchain configuration is inside of
/// [TxConfigPropose](struct.TxConfigPropose.html#method.execute) and
/// [TxConfigVote](struct.TxConfigVote.html#method.execute).
#[derive(Default)]
pub struct ConfigurationService {}

/// `ConfigurationService` database schema: tables and logically atomic mutation methods.
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

    /// Returns a table of all config proposes `TxConfigPropose`, which are stored
    /// within
    /// `StorageValueConfigProposeData` along with votes' data.
    ///
    /// - Table **key** is [hash of a configuration]
    /// (../exonum/blockchain/config/struct.StoredConfiguration.html#method.hash).
    /// This hash is normalized when a new propose is put via `put_propose`:
    ///   1. [bytes](struct.TxConfigPropose.html#method.cfg) of a `String`,
    ///   containing configuration json ->
    ///   2. `String` ->
    ///   3. [StoredConfiguration]
    ///   (../exonum/blockchain/config/struct.StoredConfiguration.html) ->
    ///   4. unique normalized `String` for a unique configuration ->
    ///   5. bytes ->
    ///   6. [hash](../exonum/crypto/fn.hash.html)(bytes)
    /// - Table **value** is `StorageValueConfigProposeData`, containing
    /// `TxConfigPropose`,
    /// which contains
    /// [bytes](struct.TxConfigPropose.html#method.cfg), corresponding to
    /// **key**.
    pub fn propose_data_by_config_hash
        (&self)
         -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, Hash, ProposeData> {
        MerklePatriciaTable::new(MapTable::new(vec![4], self.view))
    }

    /// - Table **index** is propose_id - position of a proposed [hash of a configuration]
    /// (../exonum/blockchain/config/struct.StoredConfiguration.html#method.hash) in the
    /// corresponding `TxConfigPropose` commit order.
    /// - Table **value** is [hash of a configuration]
    /// (../exonum/blockchain/config/struct.StoredConfiguration.html#method.hash) - **key** of
    /// `propose_data_by_config_hash`
    pub fn config_hash_by_ordinal(&self) -> MerkleTable<MapTable<View, [u8], Vec<u8>>, u64, Hash> {
        MerkleTable::new(MapTable::new(vec![9], self.view))
    }
    /// Returns a table of votes of validators for config, referenced by the
    /// queried
    /// `config_hash` - [hash of a configuration]
    /// (../exonum/blockchain/config/struct.StoredConfiguration.html#method.hash).
    ///
    /// 1. The list of validators, who can vote for a config, is determined by
    /// `validators` of previous [StoredConfiguration]
    /// (../exonum/blockchain/config/struct.StoredConfiguration.html).
    /// 2. Config, previous to a `StoredConfiguration` is referenced by
    /// `previous_cfg_hash` in `StoredConfiguration`.
    ///
    /// - Table **index** is validator_id - position of a validator's `PublicKey`
    /// in validator list of config,
    /// previous to config, referenced by the queried `config_hash`.
    /// - Table **value** is `TxConfigVote`, cast by validator with
    /// [PublicKey](struct.TxConfigVote.html#method.from), corresponding to **index**.
    pub fn votes_by_config_hash
        (&self,
         config_hash: &Hash)
         -> MerkleTable<MapTable<View, [u8], Vec<u8>>, u64, TxConfigVote> {
        let mut prefix = vec![5; 1 + HASH_SIZE];
        prefix[1..].copy_from_slice(config_hash.as_ref());
        MerkleTable::new(MapTable::new(prefix, self.view))
    }

    /// Put a new `StorageValueConfigProposeData` into `propose_data_by_config_hash` table with
    /// following fields:
    ///
    /// - **tx_propose** - `tx_propose` argument
    /// - **num_votes** - `validators.len()` of [StoredConfiguration]
    /// (../exonum/blockchain/config/struct.StoredConfiguration.html),
    /// referenced by `previous_cfg_hash` of config, stored in `tx_propose`.
    /// - **votes_history_hash** - root_hash of corresponding `votes_by_config_hash` table in a
    /// state right after initialization (all indices contain [empty vote](struct.ZEROVOTE.html)).
    ///
    /// If an entry with the same [hash of a configuration]
    /// (../exonum/blockchain/config/struct.StoredConfiguration.html#method.hash) is present
    /// in `propose_data_by_config_hash`, as in config inside of `tx_propose`, nothing is done.
    pub fn put_propose(&self, tx_propose: TxConfigPropose) -> StorageResult<()> {
        let cfg = <StoredConfiguration as StorageValue>::deserialize(tx_propose.cfg().to_vec());
        let cfg_hash = &StorageValue::hash(&cfg);

        if let Some(old_tx_propose) = self.get_propose(cfg_hash)? {
            error!("Discarding TxConfigPropose:{} which contains an already posted config. \
                    Previous TxConfigPropose:{}",
                   serde_json::to_string(&tx_propose)?,
                   serde_json::to_string(&old_tx_propose)?);
            return Ok(());
        }

        let general_schema = Schema::new(self.view);
        let prev_cfg = general_schema
            .configs()
            .get(&cfg.previous_cfg_hash)?
            .expect(&format!("Previous cfg:{:?} unexpectedly not found for TxConfigPropose:{:?}",
                            &cfg.previous_cfg_hash,
                            serde_json::to_string(&tx_propose)?));

        let votes_table = self.votes_by_config_hash(cfg_hash);
        debug_assert!(votes_table.is_empty().unwrap());
        let num_validators = prev_cfg.validators.len();
        for _ in 0..num_validators {
            votes_table.append(ZEROVOTE.clone())?;
        }
        let propose_data_by_config_hash = StorageValueConfigProposeData::new(tx_propose,
                                                                             &votes_table
                                                                                  .root_hash()?,
                                                                             num_validators as u64);
        let propose_data_by_config_hash_table = self.propose_data_by_config_hash();
        debug_assert!(propose_data_by_config_hash_table
                          .get(cfg_hash)
                          .unwrap()
                          .is_none());
        propose_data_by_config_hash_table
            .put(cfg_hash, propose_data_by_config_hash)?;
        self.config_hash_by_ordinal().append(*cfg_hash)
    }

    pub fn get_propose(&self, cfg_hash: &Hash) -> StorageResult<Option<TxConfigPropose>> {
        let option_propose_data_by_config_hash = self.propose_data_by_config_hash().get(cfg_hash)?;
        Ok(option_propose_data_by_config_hash
               .map(|propose_data_by_config_hash| propose_data_by_config_hash.tx_propose()))
    }

    pub fn put_vote(&self, tx_vote: TxConfigVote) -> StorageResult<()> {
        let cfg_hash = tx_vote.cfg_hash();
        let propose_data_by_config_hash_table = self.propose_data_by_config_hash();
        let mut propose_propose_data_by_config_hash = propose_data_by_config_hash_table
            .get(cfg_hash)?
            .expect(&format!("Corresponding propose unexpectedly not found for TxConfigVote:{:?}",
                            &tx_vote));

        let tx_propose = propose_propose_data_by_config_hash.tx_propose();
        let prev_cfg_hash =
            <StoredConfiguration as StorageValue>::deserialize(tx_propose.cfg().to_vec())
                .previous_cfg_hash;
        let general_schema = Schema::new(self.view);
        let prev_cfg = general_schema
            .configs()
            .get(&prev_cfg_hash)?
            .expect(&format!("Previous cfg:{:?} unexpectedly not found for TxConfigVote:{:?}",
                            prev_cfg_hash,
                            &tx_vote));
        let from: &PublicKey = tx_vote.from();
        let validator_id = prev_cfg
            .validators
            .iter()
            .position(|pk| pk == from)
            .expect(&format!("See !prev_cfg.validators.contains(self.from()) for \
                              TxConfigVote:{:?}",
                             &tx_vote));

        let votes_for_cfg_table = self.votes_by_config_hash(cfg_hash);
        votes_for_cfg_table
            .set(validator_id as u64, tx_vote.clone())?;
        propose_propose_data_by_config_hash.set_history_hash(&votes_for_cfg_table.root_hash()?);
        propose_data_by_config_hash_table.put(cfg_hash, propose_propose_data_by_config_hash)
    }

    pub fn get_votes(&self, cfg_hash: &Hash) -> StorageResult<Vec<Option<TxConfigVote>>> {
        let votes_table = self.votes_by_config_hash(cfg_hash);
        let votes_values = votes_table.values()?;
        let votes_options = votes_values
            .into_iter()
            .map(|vote| if vote == ZEROVOTE.clone() {
                     None
                 } else {
                     Some(vote)
                 })
            .collect::<Vec<_>>();
        Ok(votes_options)
    }

    pub fn state_hash(&self) -> StorageResult<Vec<Hash>> {
        Ok(vec![self.propose_data_by_config_hash().root_hash()?,
                self.config_hash_by_ordinal().root_hash()?])
    }
}

impl TxConfigPropose {
    pub fn execute(&self, view: &View) -> StorageResult<()> {
        let blockchain_schema = Schema::new(view);
        let config_schema = ConfigurationSchema::new(view);


        let following_config: Option<StoredConfiguration> = blockchain_schema
            .following_configuration()?;

        if let Some(foll_cfg) = following_config {
            error!("Discarding TxConfigPropose: {} as there is an already scheduled next config: \
                    {:?} ",
                   serde_json::to_string(self)?,
                   foll_cfg);
            return Ok(());
        }

        let actual_config: StoredConfiguration = blockchain_schema.actual_configuration()?;

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

        config_schema.put_propose(self.clone())?;

        debug!("Put TxConfigPropose:{} to config_proposes table",
               serde_json::to_string(self)?);
        Ok(())
    }
}

impl TxConfigVote {
    pub fn execute(&self, view: &View) -> StorageResult<()> {
        let blockchain_schema = Schema::new(view);
        let config_schema = ConfigurationSchema::new(view);

        let propose_option = config_schema.get_propose(self.cfg_hash())?;
        if propose_option.is_none() {
            error!("Discarding TxConfigVote:{:?} which references unknown config hash",
                   self);
            return Ok(());
        }


        let following_config: Option<StoredConfiguration> = blockchain_schema
            .following_configuration()?;

        if let Some(foll_cfg) = following_config {
            error!("Discarding TxConfigVote: {:?} as there is an already scheduled next config: \
                    {:?} ",
                   self,
                   foll_cfg);
            return Ok(());
        }

        let actual_config: StoredConfiguration = blockchain_schema.actual_configuration()?;

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
        debug!("Put TxConfigVote:{:?} to corresponding cfg votes_by_config_hash table",
               self);

        let mut votes_count = 0;

        for vote_option in config_schema.get_votes(self.cfg_hash())? {
            if vote_option.is_some() {
                votes_count += 1;
            }
        }

        if votes_count >= State::byzantine_majority_count(actual_config.validators.len()) {
            blockchain_schema.commit_configuration(parsed_config)?;
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

    /// `ConfigurationService` returns a vector, containing the single [root_hash]
    /// (../exonum/storage/struct.MerklePatriciaTable.html#method.root_hash)
    /// of [all config proposes table]
    /// (struct.ConfigurationSchema.html#method.propose_data_by_config_hash).
    ///
    /// Thus, `state_hash` is affected by any new valid propose and indirectly by
    /// any new vote for a propose.
    ///
    /// When a new vote for a config propose is added the [root_hash]
    /// (../exonum/storage/struct.MerkleTable.html#method.root_hash) of corresponding
    /// [votes for a propose table](struct.ConfigurationSchema.html#method.votes_by_config_hash)
    /// is modified. Such hash is stored in each entry of [all config proposes table]
    /// (struct.ConfigurationSchema.html#method.propose_data_by_config_hash)
    /// - `StorageValueConfigProposeData`.
    fn state_hash(&self, view: &View) -> StorageResult<Vec<Hash>> {
        let schema = ConfigurationSchema::new(view);
        schema.state_hash()
    }

    /// Returns box ([ConfigTx](ConfigTx.t.html))
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        ConfigTx::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn wire_public_api(&self, ctx: &ApiContext, router: &mut Router) {
        let api = config_api::PublicConfigApi { blockchain: ctx.blockchain().clone() };
        api.wire(router);
    }

    fn wire_private_api(&self, ctx: &ApiContext, router: &mut Router) {
        let api = config_api::PrivateConfigApi {
            channel: ctx.node_channel().clone(),
            config: (ctx.public_key().clone(), ctx.secret_key().clone()),
        };
        api.wire(router);
    }
}
