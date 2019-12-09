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
use exonum_proto::ProtobufConvert;

use crate::{
    crypto::Hash,
    helpers::{Height, ValidatorId},
    messages::{Precommit, Verified},
    proto::{self, BinaryMap},
};
use exonum_merkledb::BinaryValue;
use std::borrow::Cow;

/// Trait that represents key in block header entry map. Provide
/// mapping between `NAME` of the entry and its value.
///
/// # Usage
///
/// ```no_run
/// use exonum::blockchain::Block;
///
/// struct SomeData {}
///
/// impl BlockHeaderKey for SomeData {
///    const NAME: &'static str = "data";
///    type Value = Self;
/// }
///
/// let mut block = Block::default();
///
/// let data = SomeData {};
/// block.insert::<SomeData>(data);
/// ```
pub trait BlockHeaderKey {
    const NAME: &'static str;
    type Value: BinaryValue;
}

/// Expandable set of entries allowed to be added to the block.
pub type BlockHeaderEntries = BinaryMap<String, Vec<u8>>;

impl BlockHeaderEntries {
    /// New instance of `BlockHeaderEntries`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert new entry to the map.
    pub fn insert<K: Into<String>, V: BinaryValue>(&mut self, key: K, value: V) {
        self.0.insert(key.into(), value.to_bytes());
    }

    /// Get entry from map.
    pub fn get<K: Into<String>>(&self, key: K) -> Option<&Vec<u8>> {
        self.0.get(&key.into())
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
#[derive(
    Clone,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
    Debug,
    Serialize,
    Deserialize,
    ProtobufConvert,
    BinaryValue,
    ObjectHash,
)]
#[protobuf_convert(source = "proto::Block")]
pub struct Block {
    /// Identifier of the leader node which has proposed the block.
    pub proposer_id: ValidatorId,
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
    /// Some additional entries that can be added into the block.
    pub entries: BlockHeaderEntries,
}

impl Block {
    /// Insert new entry to the block header.
    ///
    /// # Usage
    ///
    /// ```no_run
    /// use exonum::blockchain::Block;
    ///
    /// let mut block = Block::default();
    ///
    /// let services = ActiveServices::new();
    /// block.insert::<ActiveServices>(services);
    ///
    /// ```
    pub fn insert<K: BlockHeaderKey>(&mut self, value: K::Value) {
        self.entries.insert(K::NAME, value.to_bytes());
    }

    /// Get block header entry for specified key type.
    ///
    /// # Usage
    ///
    /// ```no_run
    /// use exonum::blockchain::Block;
    ///
    /// let mut block = Block::default();
    ///
    /// let services = block.get::<ActiveServices>();
    ///
    /// ```
    pub fn get<K: BlockHeaderKey>(&self) -> Result<Option<K::Value>, failure::Error>
    where
        K::Value: BinaryValue,
    {
        self.entries
            .get(K::NAME)
            .map(|bytes: &Vec<u8>| K::Value::from_bytes(Cow::Borrowed(bytes)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::hash;
    use crate::merkledb::ObjectHash;
    use crate::proto::schema;
    use crate::runtime::InstanceId;

    #[test]
    fn block() {
        let mut entries = BlockHeaderEntries::new();
        entries.insert("key", hash(&[0u8; 10]));

        let proposer_id = ValidatorId(1024);
        let txs = [4, 5, 6];
        let height = Height(123_345);
        let prev_hash = hash(&[1, 2, 3]);
        let tx_hash = hash(&txs);
        let tx_count = txs.len() as u32;
        let state_hash = hash(&[7, 8, 9]);

        let error_hash = hash(&[10, 11]);
        let block = Block {
            proposer_id,
            height,
            tx_count,
            prev_hash,
            tx_hash,
            state_hash,
            error_hash,
            entries,
        };

        let json_str = ::serde_json::to_string(&block).unwrap();
        let block1: Block = ::serde_json::from_str(&json_str).unwrap();
        assert_eq!(block1, block);

        // protobuf roundtrip
        let pb = block.to_pb();
        let de_block: Block = ProtobufConvert::from_pb(pb).unwrap();
        assert_eq!(block, de_block);
    }

    fn create_block(entries: BlockHeaderEntries) -> Block {
        let proposer_id = ValidatorId(1024);
        let txs = [4, 5, 6];
        let height = Height(123_345);
        let prev_hash = hash(&[1, 2, 3]);
        let tx_hash = hash(&txs);
        let tx_count = txs.len() as u32;
        let state_hash = hash(&[7, 8, 9]);
        let error_hash = hash(&[10, 11]);

        Block {
            proposer_id,
            height,
            tx_count,
            prev_hash,
            tx_hash,
            state_hash,
            error_hash,
            entries,
        }
    }

    #[test]
    fn block_object_hash() {
        let block_without_entries = create_block(BlockHeaderEntries::new());
        let hash_without_entries = block_without_entries.object_hash();

        let mut entries = BlockHeaderEntries::new();
        entries.insert("key", hash(&[0u8; 10]));

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
        const NAME: &'static str = "ACTIVE_SERVICES";
        type Value = Self;
    }

    #[test]
    fn block_entry_keys() {
        let mut block = create_block(BlockHeaderEntries::new());

        assert!(block.get::<ActiveServices>().unwrap().is_none());

        let services = ActiveServices {
            services: Vec::new(),
        };

        block.insert::<ActiveServices>(services.clone());
        let services_2 = block
            .get::<ActiveServices>()
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
        block.insert::<ActiveServices>(services.clone());
        let services_2 = block
            .get::<ActiveServices>()
            .expect("Active services not found");

        assert_eq!(services, services_2.unwrap());
    }

    #[test]
    fn block_entry_wrong_type() {
        let mut entries = BlockHeaderEntries::new();

        entries.insert("ACTIVE_SERVICES", vec![]);
        let block = create_block(entries.clone());
        let services = block.get::<ActiveServices>();
        assert!(services.unwrap().unwrap().services.is_empty());

        entries.insert("ACTIVE_SERVICES", vec![0_u8; 1024]);
        let block = create_block(entries);
        let services = block.get::<ActiveServices>();
        assert!(services.is_err());
    }
}
