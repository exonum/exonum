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

use crypto::Hash;
use helpers::{Height, ValidatorId};
use messages::{Precommit, Signed};
use proto;

/// Exonum block header data structure.
///
/// A block is essentially a list of transactions, which is
/// a result of the consensus algorithm (thus authenticated by the supermajority of validators)
/// and is applied atomically to the blockchain state.
///
/// The header only contains the amount of transactions and the transactions root hash as well as
/// other information, but not the transactions themselves.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, Serialize, Deserialize, ProtobufConvert)]
#[exonum(pb = "proto::Block", crate = "crate")]
pub struct Block {
    /// Identifier of the leader node which has proposed the block.
    proposer_id: ValidatorId,
    /// Height of the block, which is also the number of this particular
    /// block in the blockchain.
    height: Height,
    /// Number of transactions in this block.
    tx_count: u32,
    /// Hash link to the previous block in the blockchain.
    prev_hash: Hash,
    /// Root hash of the Merkle tree of transactions in this block.
    tx_hash: Hash,
    /// Hash of the blockchain state after applying transactions in the block.
    state_hash: Hash,
}

impl Block {
    /// Create new `Block`.
    pub fn new(
        proposer_id: ValidatorId,
        height: Height,
        tx_count: u32,
        prev_hash: &Hash,
        tx_hash: &Hash,
        state_hash: &Hash,
    ) -> Self {
        Self {
            proposer_id,
            height,
            tx_count,
            prev_hash: *prev_hash,
            tx_hash: *tx_hash,
            state_hash: *state_hash,
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
    pub precommits: Vec<Signed<Precommit>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::hash;

    #[test]
    fn test_block() {
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
            &prev_hash,
            &tx_hash,
            &state_hash,
        );

        assert_eq!(block.proposer_id(), proposer_id);
        assert_eq!(block.height(), height);
        assert_eq!(block.tx_count(), tx_count);
        assert_eq!(block.prev_hash(), &prev_hash);
        assert_eq!(block.tx_hash(), &tx_hash);
        assert_eq!(block.state_hash(), &state_hash);
        let json_str = ::serde_json::to_string(&block).unwrap();
        let block1: Block = ::serde_json::from_str(&json_str).unwrap();
        assert_eq!(block1, block);
    }
}
