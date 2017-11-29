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
//! - list of services public keys
//! - configuration of all services, plugged in for a specific blockchain instance.
//!
//! It also contains auxiliary fields:
//!
//! - `actual_from` - blockchain height, upon reaching which current config is to become actual.
//! - `previous_cfg_hash` - hash of previous configuration, which validators' set is allowed to cast
//! votes for current config.
//!
//! See [`StoredConfiguration`][sc] in exonum.
//! [sc]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/config/struct.StoredConfiguration.html>
//!
//! While using the service's transactions and/or api, it's important to understand, how [hash of a
//! configuration][sc] is calculated. It's calculated as a hash of normalized `String` bytes,
//! containing configuration json representation.
//! When a new propose is put via `TxConfigPropose`:
//! [sc]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/config/struct.StoredConfiguration.html>
//!
//! 1. [bytes](struct.TxConfigPropose.html#method.cfg) of a `String`, containing configuration
//! json ->
//! 2. `String` ->
//! 3. `StoredConfiguration` ->
//! 4. unique normalized `String` for a unique configuration ->
//! 5. bytes ->
//! 6. [hash](https://docs.rs/exonum/0.3.0/exonum/crypto/fn.hash.html)(bytes)
//!
//! The same [hash of a configuration][1] is referenced in
//! `TxConfigVote` in [`cfg_hash`](struct.TxConfigVote.html#method.cfg_hash).
//! [1]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/config/
//!struct.StoredConfiguration.html#method.hash>
//!
//! # Examples
//!
//! Run `Exonum` blockchain testnet with single configuration service turned on for it in a
//! single process (2 threads per node: 1 - for exonum node and 1 - for http api listener)
//!
//! ```rust,no_run
//! extern crate exonum;
//! extern crate exonum_configuration;
//!
//! use exonum::helpers::fabric::NodeBuilder;
//!
//! use exonum_configuration::ConfigurationServiceFactory;
//!
//! fn main() {
//!     exonum::helpers::init_logger().unwrap();
//!     NodeBuilder::new()
//!         .with_service(Box::new(ConfigurationServiceFactory))
//!         .run();
//! }
//! ```

#![cfg_attr(feature = "cargo-clippy", allow(block_in_if_condition_stmt))]
// TODO: Remove when https://github.com/rust-lang-nursery/rust-clippy/issues/2190 is fixed.
#![cfg_attr(feature="cargo-clippy", allow(doc_markdown))]

#[macro_use]
extern crate exonum;
#[cfg(test)]
#[macro_use]
extern crate exonum_testkit;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate iron;
extern crate router;
extern crate bodyparser;
extern crate params;
#[macro_use]
extern crate lazy_static;

use router::Router;
use iron::Handler;

use exonum::api::Api;
use exonum::blockchain::{StoredConfiguration, Service, Transaction, Schema, ApiContext, gen_prefix};
use exonum::node::State;
use exonum::helpers::fabric::{ServiceFactory, Context};
use exonum::crypto::{Signature, PublicKey, Hash};
use exonum::messages::{Message, FromRaw, RawTransaction};
use exonum::storage::{Fork, ProofListIndex, ProofMapIndex, Snapshot, StorageValue};
use exonum::encoding::{Field, Error as StreamStructError};
use exonum::encoding::serialize::json::reexport as serde_json;

/// Configuration service http api.
pub mod config_api;
#[cfg(test)]
mod tests;

type ProposeData = StorageValueConfigProposeData;
/// Value of [`service_id`](struct.ConfigurationService.html#method.service_id) of
/// `ConfigurationService`
pub const CONFIG_SERVICE: u16 = 1;
/// Value of [`message_type`][1] of `TxConfigPropose`.
/// [1]: <https://docs.rs/exonum/0.3.0/exonum/messages/
///struct.MessageBuffer.html#method.message_type>
pub const CONFIG_PROPOSE_MESSAGE_ID: u16 = 0;
/// Value of [`message_type`][1] of `TxConfigVote`.
/// [1]: <https://docs.rs/exonum/0.3.0/exonum/messages/
///struct.MessageBuffer.html#method.message_type>
pub const CONFIG_VOTE_MESSAGE_ID: u16 = 1;

lazy_static! {
#[doc="
Specific [TxConfigVote](TxConfigVote.t.html) with all bytes in message set to 0.
It is used as placeholder in database for votes of validators, which didn't cast votes."]
    pub static ref ZEROVOTE: TxConfigVote = TxConfigVote::new_with_signature(&PublicKey::zero(),
    &Hash::zero(), &Signature::zero());
}

encoding_struct! {
    struct StorageValueConfigProposeData {
        const SIZE = 48;

        field tx_propose:            TxConfigPropose   [00 => 8]
        field votes_history_hash:    &Hash             [8 => 40]
        field num_votes:             u64               [40 => 48]
    }
}

/// This structure logically contains 2 fields:
///
/// 1. `TxConfigPropose` in `tx_propose` field.
///
/// 2. Reference to
///   [`votes_by_config_hash`](struct.ConfigurationSchema.html#method.votes_by_config_hash) table.
///   This reference is represented by 2 fields:
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
    /// [`votes_by_config_hash`](struct.ConfigurationSchema.html#method.votes_by_config_hash)
    /// after replacing [empty
    /// vote](struct.ZEROVOTE.html) with a real `TxConfigVote` cast by a validator.
    pub fn set_history_hash(&mut self, hash: &Hash) {
        Field::write(&hash, &mut self.raw, 8, 40);
    }
}

message! {
    struct TxConfigPropose {
        const TYPE = CONFIG_SERVICE;
        const ID = CONFIG_PROPOSE_MESSAGE_ID;
        const SIZE = 40;

        field from:           &PublicKey  [00 => 32]
        field cfg:            &str        [32 => 40]
    }
}

message! {
    struct TxConfigVote {
        const TYPE = CONFIG_SERVICE;
        const ID = CONFIG_VOTE_MESSAGE_ID;
        const SIZE = 64;

        field from:           &PublicKey  [00 => 32]
        field cfg_hash:       &Hash       [32 => 64]
    }
}

/// Structure, implementing [Service][1] trait template.
/// Most of the actual business logic of modifying `Exonum` blockchain configuration is inside of
/// [`TxConfigPropose`](struct.TxConfigPropose.html#method.execute) and
/// [`TxConfigVote`](struct.TxConfigVote.html#method.execute).
/// [1]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/trait.Service.html>
#[derive(Default)]
pub struct ConfigurationService {}

/// `ConfigurationService` database schema: tables and logically atomic mutation methods.
pub struct ConfigurationSchema<T> {
    view: T,
}


impl<T> ConfigurationSchema<T>
where
    T: AsRef<Snapshot>,
{
    pub fn new(snapshot: T) -> ConfigurationSchema<T> {
        ConfigurationSchema { view: snapshot }
    }

    /// Returns a `ProofMapIndex` table of all config proposes `TxConfigPropose`, which are stored
    /// within
    /// `StorageValueConfigProposeData` along with votes' data.
    ///
    /// - Table **key** is [hash of a configuration][1].
    /// This hash is normalized when a new propose is put via `put_propose`:
    ///   1. [bytes](struct.TxConfigPropose.html#method.cfg) of a `String`,
    ///   containing configuration json ->
    ///   2. `String` ->
    ///   3. [StoredConfiguration]
    ///   (https://docs.rs/exonum/0.3.0/exonum/blockchain/config/struct.StoredConfiguration.html) ->
    ///   4. unique normalized `String` for a unique configuration ->
    ///   5. bytes ->
    ///   6. [hash](https://docs.rs/exonum/0.3.0/exonum/crypto/fn.hash.html)(bytes)
    /// - Table **value** is `StorageValueConfigProposeData`, containing
    /// `TxConfigPropose`,
    /// which contains
    /// [bytes](struct.TxConfigPropose.html#method.cfg), corresponding to
    /// **key**.
    /// [1]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/config/
    ///struct.StoredConfiguration.html#method.hash>
    pub fn propose_data_by_config_hash(&self) -> ProofMapIndex<&T, Hash, ProposeData> {
        ProofMapIndex::new("configuration.proposes", &self.view)
    }

    /// Returns a `ProofListIndex` table of hashes of proposed configurations in propose
    /// commit order.
    ///
    /// - Table **index** is propose_id - position of a proposed [hash of a configuration][1] in
    /// the corresponding `TxConfigPropose` commit order.
    /// - Table **value** is [hash of a configuration][1] - **key** of
    /// `propose_data_by_config_hash`.
    /// [1]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/config/
    ///struct.StoredConfiguration.html#method.hash>
    pub fn config_hash_by_ordinal(&self) -> ProofListIndex<&T, Hash> {
        ProofListIndex::new("configuration.propose_hashes", &self.view)
    }

    /// Returns a `ProofListIndex` table of votes of validators for config, referenced by the
    /// queried
    /// `config_hash` - [hash of a configuration][1].
    /// [1]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/config/
    ///struct.StoredConfiguration.html#method.hash>
    ///
    /// 1. The list of validators, who can vote for a config, is determined by
    /// `validators` of previous [StoredConfiguration]
    /// (https://docs.rs/exonum/0.3.0/exonum/blockchain/config/struct.StoredConfiguration.html).
    /// 2. Config, previous to a `StoredConfiguration` is referenced by
    /// `previous_cfg_hash` in `StoredConfiguration`.
    ///
    /// - Table **index** is validator_id - position of a validator's `PublicKey`
    /// in validator list of config,
    /// previous to config, referenced by the queried `config_hash`.
    /// - Table **value** is `TxConfigVote`, cast by validator with
    /// [PublicKey](struct.TxConfigVote.html#method.from), corresponding to **index**.
    pub fn votes_by_config_hash(&self, config_hash: &Hash) -> ProofListIndex<&T, TxConfigVote> {
        ProofListIndex::with_prefix("configuration.votes", gen_prefix(config_hash), &self.view)
    }

    pub fn get_propose(&self, cfg_hash: &Hash) -> Option<TxConfigPropose> {
        let option_propose_data_by_config_hash = self.propose_data_by_config_hash().get(cfg_hash);
        option_propose_data_by_config_hash.map(|propose_data_by_config_hash| {
            propose_data_by_config_hash.tx_propose()
        })
    }

    #[cfg_attr(feature = "cargo-clippy", allow(let_and_return))]
    pub fn get_votes(&self, cfg_hash: &Hash) -> Vec<Option<TxConfigVote>> {
        let votes_table = self.votes_by_config_hash(cfg_hash);
        let votes = votes_table
            .into_iter()
            .map(|vote| if vote == ZEROVOTE.clone() {
                None
            } else {
                Some(vote)
            })
            .collect();
        votes
    }

    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.propose_data_by_config_hash().root_hash(),
            self.config_hash_by_ordinal().root_hash(),
        ]
    }
}

impl<'a> ConfigurationSchema<&'a mut Fork> {
    /// Mutable version of `propose_data_by_config_hash` index.
    pub fn propose_data_by_config_hash_mut(
        &mut self,
    ) -> ProofMapIndex<&mut Fork, Hash, ProposeData> {
        ProofMapIndex::new("configuration.proposes", &mut self.view)
    }

    /// Mutable version of `config_hash_by_ordinal` index.
    pub fn config_hash_by_ordinal_mut(&mut self) -> ProofListIndex<&mut Fork, Hash> {
        ProofListIndex::new("configuration.propose_hashes", &mut self.view)
    }

    /// Mutable version of `votes_by_config_hash` index.
    pub fn votes_by_config_hash_mut(
        &mut self,
        config_hash: &Hash,
    ) -> ProofListIndex<&mut Fork, TxConfigVote> {
        ProofListIndex::with_prefix(
            "configuration.votes",
            gen_prefix(config_hash),
            &mut self.view,
        )
    }


    /// Put a new `StorageValueConfigProposeData` into `propose_data_by_config_hash` table with
    /// following fields:
    ///
    /// - **tx_propose** - `tx_propose` argument
    /// - **num_votes** - `validators.len()` of [StoredConfiguration]
    /// (https://docs.rs/exonum/0.3.0/exonum/blockchain/config/struct.StoredConfiguration.html),
    /// referenced by `previous_cfg_hash` of config, stored in `tx_propose`.
    /// - **votes_history_hash** - root_hash of corresponding `votes_by_config_hash` table in a
    /// state right after initialization (all indices contain [empty vote](struct.ZEROVOTE.html)).
    ///
    /// If an entry with the same [hash of a configuration][1] is present in
    /// `propose_data_by_config_hash`, as in config inside of `tx_propose`, nothing is done.
    /// [1]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/config/
    ///struct.StoredConfiguration.html#method.hash>
    pub fn put_propose(&mut self, tx_propose: TxConfigPropose) -> bool {
        let cfg =
            <StoredConfiguration as StorageValue>::from_bytes(tx_propose.cfg().as_bytes().into());
        let cfg_hash = &StorageValue::hash(&cfg);

        if let Some(old_tx_propose) = self.get_propose(cfg_hash) {
            error!(
                "Discarding TxConfigPropose:{} which contains an already posted config. \
                    Previous TxConfigPropose:{}",
                serde_json::to_string(&tx_propose).unwrap(),
                serde_json::to_string(&old_tx_propose).unwrap()
            );
            return false;
        }

        let prev_cfg = Schema::new(&self.view)
            .configs()
            .get(&cfg.previous_cfg_hash)
            .expect(&format!(
                "Previous cfg:{:?} unexpectedly not found for TxConfigPropose:{:?}",
                &cfg.previous_cfg_hash,
                serde_json::to_string(&tx_propose).unwrap()
            ));

        let propose_data_by_config_hash = {
            let mut votes_table = self.votes_by_config_hash_mut(cfg_hash);
            debug_assert!(votes_table.is_empty());
            let num_validators = prev_cfg.validator_keys.len();
            for _ in 0..num_validators {
                votes_table.push(ZEROVOTE.clone());
            }

            StorageValueConfigProposeData::new(
                tx_propose,
                &votes_table.root_hash(),
                num_validators as u64,
            )
        };

        {
            let mut propose_data_by_config_hash_table = self.propose_data_by_config_hash_mut();
            debug_assert!(propose_data_by_config_hash_table.get(cfg_hash).is_none());
            propose_data_by_config_hash_table.put(cfg_hash, propose_data_by_config_hash);
        }
        self.config_hash_by_ordinal_mut().push(*cfg_hash);
        true
    }

    pub fn put_vote(&mut self, tx_vote: &TxConfigVote) -> bool {
        let cfg_hash = tx_vote.cfg_hash();
        let mut propose_data_by_config_hash = self.propose_data_by_config_hash()
            .get(cfg_hash)
            .expect(&format!(
                "Corresponding propose unexpectedly not found for TxConfigVote:{:?}",
                tx_vote
            ));

        let tx_propose = propose_data_by_config_hash.tx_propose();
        let prev_cfg_hash = <StoredConfiguration as StorageValue>::from_bytes(
            tx_propose.cfg().as_bytes().into(),
        ).previous_cfg_hash;
        let prev_cfg = Schema::new(&self.view)
            .configs()
            .get(&prev_cfg_hash)
            .expect(&format!(
                "Previous cfg:{:?} unexpectedly not found for TxConfigVote:{:?}",
                prev_cfg_hash,
                tx_vote
            ));
        //expect above depends on restriction during propose execute()
        //    let actual_config: StoredConfiguration = Schema::new(&fork).actual_configuration();
        //    ...
        //    if config_candidate_body.previous_cfg_hash != actual_config_hash {
        let from: &PublicKey = tx_vote.from();
        let validator_id = prev_cfg
            .validator_keys
            .iter()
            .position(|pk| pk.service_key == *from)
            .expect(&format!(
                "See !prev_cfg.validators.contains(self.from()) for \
                              TxConfigVote:{:?}",
                tx_vote
            ));
        //expect above depends on restrictions both during propose and vote execute()
        //    if !actual_config.validators.contains(self.from()) {
        //        error!("Discarding TxConfigVote:{:?} from unknown validator. ",
        //               self);
        //        return;
        //    }
        let res: bool;
        {
            let mut votes_for_cfg_table = self.votes_by_config_hash_mut(cfg_hash);
            if votes_for_cfg_table.get(validator_id as u64).unwrap() == ZEROVOTE.clone() {
                votes_for_cfg_table.set(validator_id as u64, tx_vote.clone());
                propose_data_by_config_hash.set_history_hash(&votes_for_cfg_table.root_hash());
                res = true;
            } else {
                res = false;
            }
        }
        if res {
            self.propose_data_by_config_hash_mut().put(
                cfg_hash,
                propose_data_by_config_hash,
            );
        }
        res
    }
}

impl<T> ConfigurationSchema<T> {
    pub fn into_snapshot(self) -> T {
        self.view
    }
}

impl Transaction for TxConfigPropose {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, fork: &mut Fork) {
        let following_config: Option<StoredConfiguration> = Schema::new(&fork)
            .following_configuration();

        if let Some(foll_cfg) = following_config {
            error!(
                "Discarding TxConfigPropose: {} as there is an already scheduled next config: \
                    {:?} ",
                serde_json::to_string(self).unwrap(),
                foll_cfg
            );
            return;
        }

        let actual_config: StoredConfiguration = Schema::new(&fork).actual_configuration();

        if !actual_config.validator_keys.iter().any(|k| {
            k.service_key == *self.from()
        })
        {
            error!(
                "Discarding TxConfigPropose:{} from unknown validator. ",
                serde_json::to_string(self).unwrap()
            );
            return;
        }

        let config_candidate = StoredConfiguration::try_deserialize(self.cfg().as_bytes());
        if config_candidate.is_err() {
            error!(
                "Discarding TxConfigPropose:{} which contains config, which cannot be parsed: \
                    {:?}",
                serde_json::to_string(self).unwrap(),
                config_candidate
            );
            return;
        }

        let actual_config_hash = actual_config.hash();
        let config_candidate_body = config_candidate.unwrap();
        if config_candidate_body.previous_cfg_hash != actual_config_hash {
            error!(
                "Discarding TxConfigPropose:{} which does not reference actual config: {:?}",
                serde_json::to_string(self).unwrap(),
                actual_config
            );
            return;
        }

        let current_height = Schema::new(&fork).height().next();
        let actual_from = config_candidate_body.actual_from;
        if actual_from <= current_height {
            error!(
                "Discarding TxConfigPropose:{} which has actual_from height less than or \
                    equal to current: {:?}",
                serde_json::to_string(self).unwrap(),
                current_height
            );
            return;
        }

        let result = ConfigurationSchema::new(fork).put_propose(self.clone());

        if result {
            trace!(
                "Put TxConfigPropose:{} to config_proposes table",
                serde_json::to_string(self).unwrap()
            );
        }
    }
}

impl Transaction for TxConfigVote {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, fork: &mut Fork) {
        let propose_option = ConfigurationSchema::new(&fork).get_propose(self.cfg_hash());
        if propose_option.is_none() {
            error!(
                "Discarding TxConfigVote:{:?} which references unknown config hash",
                self
            );
            return;
        }


        let following_config: Option<StoredConfiguration> = Schema::new(&fork)
            .following_configuration();

        if let Some(foll_cfg) = following_config {
            error!(
                "Discarding TxConfigVote: {:?} as there is an already scheduled next config: \
                    {:?} ",
                self,
                foll_cfg
            );
            return;
        }

        let actual_config: StoredConfiguration = Schema::new(&fork).actual_configuration();

        if !actual_config.validator_keys.iter().any(|k| {
            k.service_key == *self.from()
        })
        {
            error!(
                "Discarding TxConfigVote:{:?} from unknown validator. ",
                self
            );
            return;
        }

        let referenced_tx_propose = propose_option.unwrap();
        let parsed_config =
            StoredConfiguration::try_deserialize(referenced_tx_propose.cfg().as_bytes()).unwrap();
        let actual_config_hash = actual_config.hash();
        if parsed_config.previous_cfg_hash != actual_config_hash {
            error!(
                "Discarding TxConfigVote:{:?}, whose corresponding TxConfigPropose:{} does \
                    not reference actual config: {:?}",
                self,
                serde_json::to_string(&referenced_tx_propose).unwrap(),
                actual_config
            );
            return;
        }

        let current_height = Schema::new(&fork).height().next();
        let actual_from = parsed_config.actual_from;
        if actual_from <= current_height {
            error!(
                "Discarding TxConfigVote:{:?}, whose corresponding TxConfigPropose:{} has \
                    actual_from height less than or equal to current: {:?}",
                self,
                serde_json::to_string(&referenced_tx_propose).unwrap(),
                current_height
            );
            return;
        }

        let mut configuration_schema = ConfigurationSchema::new(fork);
        let result = configuration_schema.put_vote(self);
        if !result {
            return;
        }

        trace!(
            "Put TxConfigVote:{:?} to corresponding cfg votes_by_config_hash table",
            self
        );

        let mut votes_count = 0;

        {
            for vote_option in configuration_schema.get_votes(self.cfg_hash()) {
                if vote_option.is_some() {
                    votes_count += 1;
                }
            }
        }

        let fork = configuration_schema.into_snapshot();
        if votes_count >= State::byzantine_majority_count(actual_config.validator_keys.len()) {
            Schema::new(fork).commit_configuration(parsed_config);
        }
    }
}

impl ConfigurationService {
    pub fn new() -> ConfigurationService {
        ConfigurationService {}
    }
}

impl Service for ConfigurationService {
    fn service_name(&self) -> &'static str {
        "configuration"
    }

    fn service_id(&self) -> u16 {
        CONFIG_SERVICE
    }

    /// `ConfigurationService` returns a vector, containing the single [root_hash][1]
    /// of [all config proposes table]
    /// (struct.ConfigurationSchema.html#method.propose_data_by_config_hash).
    /// [1]: <https://docs.rs/exonum/0.3.0/exonum/storage/proof_list_index/
    ///struct.ProofListIndex.html#method.root_hash>
    ///
    /// Thus, `state_hash` is affected by any new valid propose and indirectly by
    /// any new vote for a propose.
    ///
    /// When a new vote for a config propose is added the [root_hash][1]
    ///  of corresponding
    /// [votes for a propose table](struct.ConfigurationSchema.html#method.votes_by_config_hash)
    /// is modified. Such hash is stored in each entry of [all config proposes table]
    /// (struct.ConfigurationSchema.html#method.propose_data_by_config_hash)
    /// - `StorageValueConfigProposeData`.
    /// [1]: <https://docs.rs/exonum/0.3.0/exonum/storage/proof_map_index/
    ///struct.ProofMapIndex.html#method.root_hash>
    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
        let schema = ConfigurationSchema::new(snapshot);
        schema.state_hash()
    }

    /// Returns box ([Transaction][1]).
    /// [1]: https://docs.rs/exonum/0.3.0/exonum/blockchain/trait.Transaction.html
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, StreamStructError> {
        match raw.message_type() {
            CONFIG_PROPOSE_MESSAGE_ID => Ok(Box::new(TxConfigPropose::from_raw(raw)?)),
            CONFIG_VOTE_MESSAGE_ID => Ok(Box::new(TxConfigVote::from_raw(raw)?)),
            _ => Err(StreamStructError::IncorrectMessageType {
                message_type: raw.message_type(),
            }),
        }
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = config_api::PublicConfigApi { blockchain: ctx.blockchain().clone() };
        api.wire(&mut router);
        Some(Box::new(router))
    }

    fn private_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = config_api::PrivateConfigApi {
            channel: ctx.node_channel().clone(),
            config: (*ctx.public_key(), ctx.secret_key().clone()),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

/// A configuration service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct ConfigurationServiceFactory;

impl ServiceFactory for ConfigurationServiceFactory {
    fn make_service(&mut self, _: &Context) -> Box<Service> {
        Box::new(ConfigurationService::new())
    }
}
