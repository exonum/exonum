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

use exonum_derive::{BinaryValue, ObjectHash};
use exonum_merkledb::{BinaryValue, MapProof};
use exonum_proto::ProtobufConvert;
use failure::Error;

use std::{borrow::Cow, fmt};

use crate::{
    crypto::Hash,
    helpers::{Height, ValidatorId},
    messages::{Precommit, Verified},
    proto::{self, OrderedMap},
};

/// Trait that represents key in block header entry map. Provide
/// mapping between `NAME` of the entry and its value.
///
/// # Usage
///
/// see [`Block::get_entry()`].
///
/// [`Block::get_entry()`]: struct.Block.html#method.get_entry
pub trait BlockHeaderKey {
    /// Key name.
    const NAME: &'static str;
    /// Type of the value associated with this key.
    type Value: BinaryValue;
}

/// Proposer identifier.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ProposerId(pub ValidatorId);

impl fmt::Display for ProposerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl BinaryValue for ProposerId {
    fn to_bytes(&self) -> Vec<u8> {
        (self.0).0.to_bytes()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, Error> {
        Ok(Self(ValidatorId(u16::from_bytes(bytes)?)))
    }
}

impl From<ValidatorId> for ProposerId {
    fn from(validator_id: ValidatorId) -> Self {
        ProposerId(validator_id)
    }
}

impl BlockHeaderKey for ProposerId {
    const NAME: &'static str = "proposer_id";
    type Value = Self;
}

/// Expandable set of headers allowed to be added to the block.
pub type AdditionalHeaders = OrderedMap<String, Vec<u8>>;

impl AdditionalHeaders {
    /// New instance of `AdditionalHeaders`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert new header to the map.
    pub fn insert<K: BlockHeaderKey>(&mut self, value: K::Value) {
        self.0.insert(K::NAME.into(), value.to_bytes());
    }

    /// Get header from the map.
    pub fn get<K: BlockHeaderKey>(&self) -> Option<&[u8]> {
        self.0.get(K::NAME).map(|v| v.as_slice())
    }
}

/// Exonum block header data structure.
///
/// A block is essentially a list of transactions, which is
/// a result of the consensus algorithm (thus authenticated by the supermajority of validators)
/// and is applied atomically to the blockchain state.
///
/// The header only contains the amount of transactions and the transactions root hash as well as
/// other information, but not the transactions themselves.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Block")]
pub struct Block {
    /// Height of the block, which is also the number of this particular
    /// block in the blockchain.
    pub height: Height,
    /// Number of transactions in this block.
    pub tx_count: u32,
    /// Hash link to the previous block in the blockchain.
    pub prev_hash: Hash,
    /// Root hash of the Merkle tree of transactions in this block.
    pub tx_hash: Hash,
    /// Hash of the blockchain state after applying transactions in the block.
    pub state_hash: Hash,
    /// Root hash of the Merkle Patricia tree of the erroneous calls performed within the block.
    /// These calls can include transactions, `before_transactions` and/or `after_transactions` hooks
    /// for services.
    pub error_hash: Hash,
    /// Additional information that can be added into the block.
    pub additional_headers: AdditionalHeaders,
}

impl Block {
    /// Inserts new additional header to the block.
    #[doc(hidden)]
    pub fn add_header<K: BlockHeaderKey>(&mut self, value: K::Value) {
        self.additional_headers.insert::<K>(value);
    }

    /// Get block additional header value for specified key type. Key type is specified via
    /// type parameter.
    ///
    /// # Usage
    ///
    /// ```
    /// # use exonum::crypto::Hash;
    /// # use exonum::blockchain::{Block, BlockHeaderKey, AdditionalHeaders};
    /// # use exonum::helpers::Height;
    /// # use exonum::merkledb::BinaryValue;
    /// # use failure::Error;
    /// # use std::borrow::Cow;
    ///
    /// // Suppose we store a list of active service IDs in a block.
    /// // We can do this by defining a corresponding BlockHeaderKey implementation.
    /// struct ActiveServices {
    ///     service_id: u32,
    /// }
    ///
    /// # impl BinaryValue for ActiveServices {
    /// #    fn to_bytes(&self) -> Vec<u8> { vec![] }
    /// #    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, Error> { Ok(Self { service_id: 0 }) }
    /// # }
    ///
    /// // To implement `BlockHeaderKey` we need to provide the key name and a corresponding
    /// // value type. In this case it's `Self`.
    /// impl BlockHeaderKey for ActiveServices {
    ///     const NAME: &'static str = "active_services";
    ///     type Value = Self;
    /// }
    ///
    /// // Create an empty block.
    /// let mut block = Block {
    ///   #  height: Height(0),
    ///   #  tx_count: 0,
    ///   #  prev_hash: Hash::zero(),
    ///   #  tx_hash: Hash::zero(),
    ///   #  state_hash: Hash::zero(),
    ///   #  error_hash: Hash::zero(),
    ///     additional_headers: AdditionalHeaders::new(),
    /// };
    ///
    /// let services = block.get_header::<ActiveServices>().expect("Entry deserialization error");
    /// assert!(services.is_none())
    /// ```
    pub fn get_header<K: BlockHeaderKey>(&self) -> Result<Option<K::Value>, failure::Error>
    where
        K::Value: BinaryValue,
    {
        self.additional_headers
            .get::<K>()
            .map(|bytes: &[u8]| K::Value::from_bytes(Cow::Borrowed(bytes)))
            .transpose()
    }
}

/// Block with its `Precommit` messages.
///
/// This structure contains enough information to prove the correctness of
/// a block. It consists of the block itself and the `Precommit`
/// messages related to this block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockProof {
    /// Block header containing such information as the ID of the node which
    /// proposed the block, the height of the block, the number of transactions
    /// in the block, etc.
    pub block: Block,
    /// List of `Precommit` messages for the block.
    pub precommits: Vec<Verified<Precommit>>,
}

/// Proof of authenticity for a single index within the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProof {
    /// Proof of authenticity for the block header.
    #[serde(flatten)]
    pub block_proof: BlockProof,

    /// Proof of authenticity for the index. Must contain a single key - a full index name
    /// in the form `$service_name.$name_within_service`, e.g., `cryptocurrency.wallets`.
    /// The root hash of the proof must be equal to the `state_hash` mentioned in `block_proof`.
    pub index_proof: MapProof<String, Hash>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::hash;
    use crate::merkledb::ObjectHash;
    use crate::proto::schema;
    use crate::runtime::InstanceId;

    impl BlockHeaderKey for Hash {
        const NAME: &'static str = "HASH";
        type Value = Self;
    }

    #[test]
    fn block() {
        let mut additional_headers = AdditionalHeaders::new();
        additional_headers.insert::<Hash>(hash(&[0u8; 10]));

        let txs = [4, 5, 6];
        let height = Height(123_345);
        let prev_hash = hash(&[1, 2, 3]);
        let tx_hash = hash(&txs);
        let tx_count = txs.len() as u32;
        let state_hash = hash(&[7, 8, 9]);

        let error_hash = hash(&[10, 11]);
        let block = Block {
            height,
            tx_count,
            prev_hash,
            tx_hash,
            state_hash,
            error_hash,
            additional_headers,
        };

        let json_str = ::serde_json::to_string(&block).unwrap();
        let block1: Block = ::serde_json::from_str(&json_str).unwrap();
        assert_eq!(block1, block);

        // protobuf roundtrip
        let pb = block.to_pb();
        let de_block: Block = ProtobufConvert::from_pb(pb).unwrap();
        assert_eq!(block, de_block);
    }

    fn create_block(additional_headers: AdditionalHeaders) -> Block {
        let txs = [4, 5, 6];
        let height = Height(123_345);
        let prev_hash = hash(&[1, 2, 3]);
        let tx_hash = hash(&txs);
        let tx_count = txs.len() as u32;
        let state_hash = hash(&[7, 8, 9]);
        let error_hash = hash(&[10, 11]);

        Block {
            height,
            tx_count,
            prev_hash,
            tx_hash,
            state_hash,
            error_hash,
            additional_headers,
        }
    }

    #[test]
    fn block_object_hash() {
        let block_without_entries = create_block(AdditionalHeaders::new());
        let hash_without_entries = block_without_entries.object_hash();

        let mut entries = AdditionalHeaders::new();
        entries.insert::<Hash>(hash(&[0u8; 10]));

        let block_with_entries = create_block(entries);
        let hash_with_entries = block_with_entries.object_hash();

        assert_ne!(hash_without_entries, hash_with_entries);
    }

    #[derive(Debug, Clone, ProtobufConvert, BinaryValue, Eq, PartialEq)]
    #[protobuf_convert(source = "schema::tests::TestServiceInfo")]
    struct TestServiceInfo {
        pub instance_id: InstanceId,
        pub runtime_id: u32,
        pub name: String,
    }

    #[derive(Debug, Clone, ProtobufConvert, BinaryValue, Eq, PartialEq)]
    #[protobuf_convert(source = "schema::tests::TestActiveServices")]
    struct ActiveServices {
        pub services: Vec<TestServiceInfo>,
    }

    impl BlockHeaderKey for ActiveServices {
        const NAME: &'static str = "active_services";
        type Value = Self;
    }

    #[test]
    fn block_entry_keys() {
        let mut block = create_block(AdditionalHeaders::new());

        assert!(block.get_header::<ActiveServices>().unwrap().is_none());

        let services = ActiveServices {
            services: Vec::new(),
        };

        block.add_header::<ActiveServices>(services.clone());
        let services_2 = block
            .get_header::<ActiveServices>()
            .expect("Active services not found");

        assert_eq!(services, services_2.unwrap());

        let info = TestServiceInfo {
            runtime_id: 0,
            instance_id: 1,
            name: "test".into(),
        };

        let info_2 = TestServiceInfo {
            runtime_id: 2,
            instance_id: 10,
            name: "test service instance".into(),
        };

        let services = ActiveServices {
            services: vec![info, info_2],
        };

        // Should override previous entry for `ActiveServices`.
        block.add_header::<ActiveServices>(services.clone());
        let services_2 = block
            .get_header::<ActiveServices>()
            .expect("Active services not found");

        assert_eq!(services, services_2.unwrap());
    }

    #[test]
    fn block_entry_wrong_type() {
        let mut entries: OrderedMap<String, Vec<u8>> = OrderedMap::default();

        entries.0.insert("active_services".into(), vec![]);
        let block = create_block(entries.clone());
        let services = block.get_header::<ActiveServices>();
        assert!(services.unwrap().unwrap().services.is_empty());

        entries.0.insert("active_services".into(), vec![0_u8; 1024]);
        let block = create_block(entries);
        let services = block.get_header::<ActiveServices>();
        assert!(services.is_err());
    }
}
