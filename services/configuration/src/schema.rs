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

//! Storage schema for the configuration service.

use exonum::blockchain::gen_prefix;
use exonum::encoding::Field;
use exonum::crypto::Hash;
use exonum::storage::{Fork, ProofListIndex, ProofMapIndex, Snapshot};

use transactions::{Propose, Vote, ZEROVOTE};

encoding_struct! {
    /// This structure logically contains 2 fields:
    ///
    /// 1. `Propose` in `tx_propose` field.
    ///
    /// 2. Reference to
    ///   [`votes_by_config_hash`](struct.ConfigurationSchema.html#method.votes_by_config_hash) table.
    ///   This reference is represented by 2 fields:
    ///   - `votes_history_hash`
    ///   - `num_validators`
    ///
    /// Length of the table is stored in `num_validators` field, which isn't changed
    /// after table initialization, because number of possible vote slots for a config is determined by
    /// number of validators in its previous config.
    ///
    /// Table's root hash - in `votes_history_hash` field, which is
    /// modified after a vote from validator is added.
    struct ProposeData {
        tx_propose: Propose,
        votes_history_hash: &Hash,
        num_validators: u64,
    }
}

impl ProposeData {
    /// Method to mutate `votes_history_hash` field containing root hash of
    /// [`votes_by_config_hash`](struct.ConfigurationSchema.html#method.votes_by_config_hash)
    /// after replacing [empty
    /// vote](struct.ZEROVOTE.html) with a real `Vote` cast by a validator.
    pub fn set_history_hash(mut self, hash: &Hash) -> Self {
        Field::write(&hash, &mut self.raw, 8, 40);
        self
    }
}

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

    /// Returns a `ProofMapIndex` table of all config proposes `Propose`, which are stored
    /// within `ProposeData` along with votes' data.
    ///
    /// - Table **key** is [hash of a configuration][1].
    /// This hash is normalized when a new propose is put via `put_propose`:
    ///   1. [bytes](struct.Propose.html#method.cfg) of a `String`,
    ///   containing configuration json ->
    ///   2. `String` ->
    ///   3. [StoredConfiguration]
    ///   (https://docs.rs/exonum/0.3.0/exonum/blockchain/config/struct.StoredConfiguration.html) ->
    ///   4. unique normalized `String` for a unique configuration ->
    ///   5. bytes ->
    ///   6. [hash](https://docs.rs/exonum/0.3.0/exonum/crypto/fn.hash.html)(bytes)
    /// - Table **value** is `ProposeData`, containing
    /// `Propose`,
    /// which contains
    /// [bytes](struct.Propose.html#method.cfg), corresponding to
    /// **key**.
    /// [1]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/config/
    ///struct.StoredConfiguration.html#method.hash>
    pub fn propose_data_by_config_hash(&self) -> ProofMapIndex<&Snapshot, Hash, ProposeData> {
        ProofMapIndex::new("configuration.proposes", self.view.as_ref())
    }

    /// Returns a `ProofListIndex` table of hashes of proposed configurations in propose
    /// commit order.
    ///
    /// - Table **index** is propose_id - position of a proposed [hash of a configuration][1] in
    /// the corresponding `Propose` commit order.
    /// - Table **value** is [hash of a configuration][1] - **key** of
    /// `propose_data_by_config_hash`.
    /// [1]: <https://docs.rs/exonum/0.3.0/exonum/blockchain/config/
    ///struct.StoredConfiguration.html#method.hash>
    pub fn config_hash_by_ordinal(&self) -> ProofListIndex<&Snapshot, Hash> {
        ProofListIndex::new("configuration.propose_hashes", self.view.as_ref())
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
    /// - Table **value** is `Vote`, cast by validator with
    /// [PublicKey](struct.Vote.html#method.from), corresponding to **index**.
    pub fn votes_by_config_hash(&self, config_hash: &Hash) -> ProofListIndex<&Snapshot, Vote> {
        ProofListIndex::with_prefix(
            "configuration.votes",
            gen_prefix(config_hash),
            self.view.as_ref(),
        )
    }

    /// Returns a `Propose` transaction with a particular configuration hash.
    pub fn propose(&self, cfg_hash: &Hash) -> Option<Propose> {
        self.propose_data_by_config_hash().get(cfg_hash).map(
            |propose_data| propose_data.tx_propose(),
        )
    }

    /// Returns a list of votes for the proposal corresponding to the given configuration hash.
    #[cfg_attr(feature = "cargo-clippy", allow(let_and_return))]
    pub fn votes(&self, cfg_hash: &Hash) -> Vec<Option<Vote>> {
        let votes_by_config_hash = self.votes_by_config_hash(cfg_hash);
        let votes = votes_by_config_hash
            .iter()
            .map(|vote| if vote == ZEROVOTE.clone() {
                None
            } else {
                Some(vote)
            })
            .collect();
        votes
    }

    /// Returns state hash values used by the service.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.propose_data_by_config_hash().root_hash(),
            self.config_hash_by_ordinal().root_hash(),
        ]
    }
}

impl<'a> ConfigurationSchema<&'a mut Fork> {
    /// Mutable version of `propose_data_by_config_hash` index.
    pub(crate) fn propose_data_by_config_hash_mut(
        &mut self,
    ) -> ProofMapIndex<&mut Fork, Hash, ProposeData> {
        ProofMapIndex::new("configuration.proposes", &mut self.view)
    }

    /// Mutable version of `config_hash_by_ordinal` index.
    pub(crate) fn config_hash_by_ordinal_mut(&mut self) -> ProofListIndex<&mut Fork, Hash> {
        ProofListIndex::new("configuration.propose_hashes", &mut self.view)
    }

    /// Mutable version of `votes_by_config_hash` index.
    pub(crate) fn votes_by_config_hash_mut(
        &mut self,
        config_hash: &Hash,
    ) -> ProofListIndex<&mut Fork, Vote> {
        ProofListIndex::with_prefix(
            "configuration.votes",
            gen_prefix(config_hash),
            &mut self.view,
        )
    }
}
