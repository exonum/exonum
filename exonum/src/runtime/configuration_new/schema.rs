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

//! Storage schema for the configuration service.

use crate::{
    crypto::{Hash, HASH_SIZE},
    proto,
};

use exonum_merkledb::{
    BinaryValue, IndexAccess, MapIndex, ObjectHash, ProofListIndex, ProofMapIndex,
};

use std::{borrow::Cow, ops::Deref};

use super::transactions::Propose;

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
    SERVICE_IDS => "service_ids";
}

/// Extended information about a proposal used for the storage.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "proto::schema::configuration::ProposeData", crate = "crate")]
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

static NO_VOTE_BYTES: [u8; 1] = [0u8];

/// A enum used to represent different kinds of vote, `Vote` and `VoteAgainst` transactions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Copy)]
#[serde(rename_all = "lowercase", tag = "vote_type", content = "tx_hash")]
pub enum VotingDecision {
    /// `Vote` transaction `Hash` variant.
    Yea(Hash),
    /// `VoteAgainst` transaction `Hash` variant.
    Nay(Hash),
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

impl BinaryValue for VotingDecision {
    fn to_bytes(&self) -> Vec<u8> {
        let (tag, mut res) = match self {
            VotingDecision::Yea(vote) => (YEA_TAG, vote.into_bytes()),
            VotingDecision::Nay(vote_against) => (NAY_TAG, vote_against.into_bytes()),
        };
        res.push(tag);
        res
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        assert_eq!(bytes.len(), HASH_SIZE + 1);
        let tag = bytes[HASH_SIZE];
        let raw_hash = Hash::from_slice(&bytes[0..HASH_SIZE]).unwrap();
        let res = match tag {
            YEA_TAG => VotingDecision::Yea(raw_hash),
            NAY_TAG => VotingDecision::Nay(raw_hash),
            _ => panic!("invalid voting tag: {}", tag),
        };

        Ok(res)
    }
}

impl_object_hash_for_binary_value! { VotingDecision }

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

impl_object_hash_for_binary_value! { MaybeVote }

impl BinaryValue for MaybeVote {
    fn to_bytes(&self) -> Vec<u8> {
        match self.0 {
            Some(v) => v.into_bytes(),
            None => NO_VOTE_BYTES.to_vec(),
        }
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let res = if NO_VOTE_BYTES.eq(bytes.as_ref()) {
            MaybeVote::none()
        } else {
            MaybeVote::some(VotingDecision::from_bytes(bytes)?)
        };

        Ok(res)
    }
}

/// Database schema used by the configuration service.
#[derive(Debug)]
pub struct Schema<T> {
    access: T,
}

impl<T> Schema<T>
where
    T: IndexAccess,
{
    /// Creates a new schema.
    pub fn new(access: T) -> Schema<T> {
        Schema { access }
    }

    /// Returns propose information indexed by the hash of the configuration corresponding
    /// to a proposal.
    ///
    /// Consult [the crate-level docs](index.html) for details how hashes of the configuration
    /// are calculated.
    pub fn propose_data_by_config_hash(&self) -> ProofMapIndex<T, Hash, ProposeData> {
        ProofMapIndex::new(PROPOSES, self.access.clone())
    }

    /// Returns a table of hashes of proposed configurations in the commit order.
    pub fn config_hash_by_ordinal(&self) -> ProofListIndex<T, Hash> {
        ProofListIndex::new(PROPOSE_HASHES, self.access.clone())
    }

    /// Returns a table with mapping between service instance names and their identifiers.
    pub fn service_ids(&self) -> MapIndex<T, String, u32> {
        MapIndex::new(SERVICE_IDS, self.access.clone())
    }

    /// Returns a table of votes of validators for a particular proposal, referenced
    /// by its configuration hash.
    pub fn votes_by_config_hash(&self, config_hash: &Hash) -> ProofListIndex<T, MaybeVote> {
        ProofListIndex::new_in_family(VOTES, config_hash, self.access.clone())
    }

    /// Returns a `Propose` transaction with a particular configuration hash.
    pub fn propose(&self, cfg_hash: &Hash) -> Option<Propose> {
        self.propose_data_by_config_hash()
            .get(cfg_hash)?
            .tx_propose
            .into()
    }

    /// Returns a list of votes for the proposal corresponding to the given configuration hash.
    pub fn votes(&self, cfg_hash: &Hash) -> Vec<Option<VotingDecision>> {
        let votes_by_config_hash = self.votes_by_config_hash(cfg_hash);
        votes_by_config_hash.iter().map(MaybeVote::into).collect()
    }

    /// Returns state hash values used by the configuration service.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.propose_data_by_config_hash().object_hash(),
            self.config_hash_by_ordinal().object_hash(),
        ]
    }
}
