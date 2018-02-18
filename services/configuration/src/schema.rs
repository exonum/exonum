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

use transactions::{Propose, Vote};

encoding_struct! {
    /// Extended information about a proposal used for the storage.
    struct ProposeData {
        /// Proposal transaction.
        tx_propose: Propose,
        /// Merkle root of all votes for the proposal.
        votes_history_hash: &Hash,
        /// Number of eligible voting validators.
        num_validators: u64,
    }
}

impl ProposeData {
    /// Changes the `votes_history_hash` field of this structure, leaving other fields intact.
    pub fn set_history_hash(mut self, hash: &Hash) -> Self {
        Field::write(&hash, &mut self.raw, 8, 40);
        self
    }
}

/// Database schema used by the configuration service.
#[derive(Debug)]
pub struct Schema<T> {
    view: T,
}

impl<T> Schema<T>
where
    T: AsRef<Snapshot>,
{
    /// Creates a new schema.
    pub fn new(snapshot: T) -> Schema<T> {
        Schema { view: snapshot }
    }

    /// Returns propose information indexed by the hash of the configuration corresponding
    /// to a proposal.
    ///
    /// Consult [the crate-level docs](index.html) for details how hashes of the configuration
    /// are calculated.
    pub fn propose_data_by_config_hash(&self) -> ProofMapIndex<&Snapshot, Hash, ProposeData> {
        ProofMapIndex::new("configuration.proposes", self.view.as_ref())
    }

    /// Returns a table of hashes of proposed configurations in the commit order.
    pub fn config_hash_by_ordinal(&self) -> ProofListIndex<&Snapshot, Hash> {
        ProofListIndex::new("configuration.propose_hashes", self.view.as_ref())
    }

    /// Returns a table of votes of validators for a particular proposal, referenced
    /// by its configuration hash.
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
            .map(Vote::into_option)
            .collect();
        votes
    }

    /// Returns state hash values used by the configuration service.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.propose_data_by_config_hash().root_hash(),
            self.config_hash_by_ordinal().root_hash(),
        ]
    }
}

impl<'a> Schema<&'a mut Fork> {
    /// Mutable version of the `propose_data_by_config_hash` index.
    pub(crate) fn propose_data_by_config_hash_mut(
        &mut self,
    ) -> ProofMapIndex<&mut Fork, Hash, ProposeData> {
        ProofMapIndex::new("configuration.proposes", &mut self.view)
    }

    /// Mutable version of the `config_hash_by_ordinal` index.
    pub(crate) fn config_hash_by_ordinal_mut(&mut self) -> ProofListIndex<&mut Fork, Hash> {
        ProofListIndex::new("configuration.propose_hashes", &mut self.view)
    }

    /// Mutable version of the `votes_by_config_hash` index.
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
