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

use exonum::{
    crypto::{self, CryptoHash, Hash, HASH_SIZE},
    storage::{Fork, ProofListIndex, ProofMapIndex, Snapshot, StorageValue},
};

use std::{borrow::Cow, ops::Deref};

use proto;
use transactions::Propose;

const YEA_TAG: u8 = 1;
const NAY_TAG: u8 = 2;

// Defines `&str` constants with given name and value.
macro_rules! define_names {
    ($($name:ident => $value:expr;)+) => (
        $(const $name: &str = concat!("configuration.", $value);)*
    )
}

define_names! {
    PROPOSES => "proposes";
    PROPOSE_HASHES => "propose_hashes";
    VOTES => "votes";
}

/// Extended information about a proposal used for the storage.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "proto::ProposeData")]
pub struct ProposeData {
    /// Proposal transaction.
    pub tx_propose: Propose,
    /// Merkle root of all votes for the proposal.
    pub votes_history_hash: Hash,
    /// Number of eligible voting validators.
    pub num_validators: u64,
}

impl ProposeData {
    /// New ProposeData.
    pub fn new(tx_propose: Propose, votes_history_hash: &Hash, num_validators: u64) -> Self {
        Self {
            tx_propose,
            votes_history_hash: *votes_history_hash,
            num_validators,
        }
    }
}

lazy_static! {
    static ref NO_VOTE_BYTES: Vec<u8> = vec![0u8];
}

/// A enum used to represent different kinds of vote, `Vote` and `VoteAgainst` transactions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Copy)]
#[serde(rename_all = "lowercase", tag = "vote_type", content = "tx_hash")]
pub enum VotingDecision {
    /// `Vote` transaction `Hash` variant.
    Yea(Hash),
    /// `VoteAgainst` transaction `Hash` variant.
    Nay(Hash),
}

impl CryptoHash for VotingDecision {
    fn hash(&self) -> Hash {
        let res = StorageValue::into_bytes(*self);
        res.hash()
    }
}

impl VotingDecision {
    /// Returns internal transaction hash.
    pub fn tx_hash(&self) -> Hash {
        match *self {
            VotingDecision::Yea(h) => h,
            VotingDecision::Nay(h) => h,
        }
    }
}

impl StorageValue for VotingDecision {
    fn into_bytes(self) -> Vec<u8> {
        let (tag, mut res) = match self {
            VotingDecision::Yea(vote) => (YEA_TAG, vote.into_bytes()),
            VotingDecision::Nay(vote_against) => (NAY_TAG, vote_against.into_bytes()),
        };
        res.push(tag);
        res
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        assert_eq!(bytes.len(), HASH_SIZE + 1);
        let tag = bytes[HASH_SIZE];
        let raw_hash = Hash::from_slice(&bytes[0..HASH_SIZE]).unwrap();
        match tag {
            YEA_TAG => VotingDecision::Yea(raw_hash),
            NAY_TAG => VotingDecision::Nay(raw_hash),
            _ => panic!("invalid voting tag: {}", tag),
        }
    }
}

/// A functional equivalent to `Option<VotingDecision>` used to store votes in the service
/// schema.
///
/// # Notes
///
/// The `None` variant of the type is represented by a `Vote` with all bytes set to zero.
#[derive(Clone, Debug, PartialEq)]
pub struct MaybeVote(Option<VotingDecision>);

impl MaybeVote {
    /// Creates a `None` variant.
    pub fn none() -> Self {
        MaybeVote(None)
    }

    /// Creates a `Some` variant.
    pub fn some(vote: VotingDecision) -> Self {
        MaybeVote(Some(vote))
    }

    /// Returns true if it's a `Some` variant hold `Vote`.
    pub fn is_consent(&self) -> bool {
        match self.0 {
            Some(VotingDecision::Yea(_)) => true,
            _ => false,
        }
    }
}

impl From<MaybeVote> for Option<VotingDecision> {
    fn from(vote: MaybeVote) -> Option<VotingDecision> {
        vote.0
    }
}

impl From<VotingDecision> for MaybeVote {
    fn from(vote: VotingDecision) -> MaybeVote {
        MaybeVote(Some(vote))
    }
}

impl Deref for MaybeVote {
    type Target = Option<VotingDecision>;

    fn deref(&self) -> &Option<VotingDecision> {
        &self.0
    }
}

impl CryptoHash for MaybeVote {
    fn hash(&self) -> Hash {
        match self.0 {
            Some(ref vote) => vote.hash(),
            None => crypto::hash(&NO_VOTE_BYTES),
        }
    }
}

impl StorageValue for MaybeVote {
    fn into_bytes(self) -> Vec<u8> {
        match self.0 {
            Some(v) => v.into_bytes(),
            None => NO_VOTE_BYTES.clone(),
        }
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        if NO_VOTE_BYTES.as_slice().eq(bytes.as_ref()) {
            MaybeVote::none()
        } else {
            MaybeVote::some(VotingDecision::from_bytes(bytes))
        }
    }
}

/// Database schema used by the configuration service.
#[derive(Debug)]
pub struct Schema<T> {
    view: T,
}

impl<T> Schema<T>
where
    T: AsRef<dyn Snapshot>,
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
    pub fn propose_data_by_config_hash(&self) -> ProofMapIndex<&dyn Snapshot, Hash, ProposeData> {
        ProofMapIndex::new(PROPOSES, self.view.as_ref())
    }

    /// Returns a table of hashes of proposed configurations in the commit order.
    pub fn config_hash_by_ordinal(&self) -> ProofListIndex<&dyn Snapshot, Hash> {
        ProofListIndex::new(PROPOSE_HASHES, self.view.as_ref())
    }

    /// Returns a table of votes of validators for a particular proposal, referenced
    /// by its configuration hash.
    pub fn votes_by_config_hash(
        &self,
        config_hash: &Hash,
    ) -> ProofListIndex<&dyn Snapshot, MaybeVote> {
        ProofListIndex::new_in_family(VOTES, config_hash, self.view.as_ref())
    }

    /// Returns a `Propose` transaction with a particular configuration hash.
    pub fn propose(&self, cfg_hash: &Hash) -> Option<Propose> {
        self.propose_data_by_config_hash()
            .get(cfg_hash)?
            .tx_propose
            .into()
    }

    /// Returns a list of votes for the proposal corresponding to the given configuration hash.
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::let_and_return))]
    pub fn votes(&self, cfg_hash: &Hash) -> Vec<Option<VotingDecision>> {
        let votes_by_config_hash = self.votes_by_config_hash(cfg_hash);
        let votes = votes_by_config_hash.iter().map(MaybeVote::into).collect();
        votes
    }

    /// Returns state hash values used by the configuration service.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.propose_data_by_config_hash().merkle_root(),
            self.config_hash_by_ordinal().merkle_root(),
        ]
    }
}

impl<'a> Schema<&'a mut Fork> {
    /// Mutable version of the `propose_data_by_config_hash` index.
    pub(crate) fn propose_data_by_config_hash_mut(
        &mut self,
    ) -> ProofMapIndex<&mut Fork, Hash, ProposeData> {
        ProofMapIndex::new(PROPOSES, &mut self.view)
    }

    /// Mutable version of the `config_hash_by_ordinal` index.
    pub(crate) fn config_hash_by_ordinal_mut(&mut self) -> ProofListIndex<&mut Fork, Hash> {
        ProofListIndex::new(PROPOSE_HASHES, &mut self.view)
    }

    /// Mutable version of the `votes_by_config_hash` index.
    pub(crate) fn votes_by_config_hash_mut(
        &mut self,
        config_hash: &Hash,
    ) -> ProofListIndex<&mut Fork, MaybeVote> {
        ProofListIndex::new_in_family(VOTES, config_hash, &mut self.view)
    }
}
