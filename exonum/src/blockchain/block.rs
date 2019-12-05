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
    proto,
};
use exonum_merkledb::BinaryValue;

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

    /// TODO: add doc
    pub entries: Vec<BlockHeaderEntry>,
}

/// TODO: add doc
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
#[protobuf_convert(source = "proto::BlockHeaderEntry")]
pub struct BlockHeaderEntry {
    /// TODO: add doc
    pub key: String,
    /// TODO: add doc
    pub value: Vec<u8>,
}

/// TODO: add doc
impl BlockHeaderEntry {
    /// TODO: add doc
    pub fn from<T>(key: String, value: T) -> Self
    where
        T: BinaryValue,
    {
        let value = value.to_bytes();
        Self { key, value }
    }
}

impl Block {
    /// Create new `Block`.
    pub fn new(
        proposer_id: ValidatorId,
        height: Height,
        tx_count: u32,
        prev_hash: Hash,
        tx_hash: Hash,
        state_hash: Hash,
        entries: Vec<BlockHeaderEntry>,
    ) -> Self {
        Self {
            proposer_id,
            height,
            tx_count,
            prev_hash,
            tx_hash,
            state_hash,
            entries,
        }
    }
    /// Identifier of the leader node which has proposed the block.
    pub fn proposer_id(&self) -> ValidatorId {
        self.proposer_id
    }
    /// Height of the block, which is also the number of this particular
    /// block in the blockchain.
    pub fn height(&self) -> Height {
        self.height
    }
    /// Number of transactions in this block.
    pub fn tx_count(&self) -> u32 {
        self.tx_count
    }
    /// Hash link to the previous block in the blockchain.
    pub fn prev_hash(&self) -> &Hash {
        &self.prev_hash
    }
    /// Root hash of the Merkle tree of transactions in this block.
    pub fn tx_hash(&self) -> &Hash {
        &self.tx_hash
    }
    /// Hash of the blockchain state after applying transactions in the block.
    pub fn state_hash(&self) -> &Hash {
        &self.state_hash
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

    #[test]
    fn block() {
        let entry = BlockHeaderEntry::from("key".to_owned(), hash(&[0u8; 10]));

        let proposer_id = ValidatorId(1024);
        let txs = [4, 5, 6];
        let height = Height(123_345);
        let prev_hash = hash(&[1, 2, 3]);
        let tx_hash = hash(&txs);
        let tx_count = txs.len() as u32;
        let state_hash = hash(&[7, 8, 9]);

        let block = Block::new(
            proposer_id,
            height,
            tx_count,
            prev_hash,
            tx_hash,
            state_hash,
            vec![entry.clone()],
        );

        assert_eq!(block.proposer_id(), proposer_id);
        assert_eq!(block.height(), height);
        assert_eq!(block.tx_count(), tx_count);
        assert_eq!(block.prev_hash(), &prev_hash);
        assert_eq!(block.tx_hash(), &tx_hash);
        assert_eq!(block.state_hash(), &state_hash);
        assert_eq!(block.entries, vec![entry]);

        // json roundtrip
        let json_str = ::serde_json::to_string(&block).unwrap();
        let block1: Block = ::serde_json::from_str(&json_str).unwrap();
        assert_eq!(block1, block);

        // protobuf roundtrip
        let pb = block.to_pb();
        let de_block: Block = ProtobufConvert::from_pb(pb).unwrap();
        assert_eq!(block, de_block);
    }

    fn create_block(entries: Vec<BlockHeaderEntry>) -> Block {
        let proposer_id = ValidatorId(1024);
        let txs = [4, 5, 6];
        let height = Height(123_345);
        let prev_hash = hash(&[1, 2, 3]);
        let tx_hash = hash(&txs);
        let tx_count = txs.len() as u32;
        let state_hash = hash(&[7, 8, 9]);

        Block::new(
            proposer_id,
            height,
            tx_count,
            prev_hash,
            tx_hash,
            state_hash,
            entries,
        )
    }

    #[test]
    fn block_object_hash() {
        let block_without_entries = create_block(vec![]);
        let hash_without_entries = block_without_entries.object_hash();

        let entry = BlockHeaderEntry::from("key".to_owned(), hash(&[0u8; 10]));

        let block_with_entries = create_block(vec![entry]);
        let hash_with_entries = block_with_entries.object_hash();

        assert_ne!(hash_without_entries, hash_with_entries);
    }
}
