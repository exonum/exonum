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

//! Blockchain explorer module provides api for getting information about blocks and transactions
//! from the blockchain.

use std::cmp;

use serde_json::Value;

use storage::ListProof;
use crypto::Hash;
use blockchain::{Schema, Blockchain, Block, TxLocation, TransactionResult};
use messages::Precommit;
// TODO: if explorer is usable anywhere else, remove `ApiError` dependencies (ECR-163).
use api::ApiError;
use helpers::Height;

/// Blockchain explorer.
#[derive(Debug)]
pub struct BlockchainExplorer<'a> {
    blockchain: &'a Blockchain,
}

/// Block information.
#[derive(Debug, Serialize, Deserialize)]
pub struct BlockInfo {
    /// Block header from blockchain.
    pub block: Block,
    /// List of precommit for this block.
    pub precommits: Vec<Precommit>,
    /// List of hashes for transactions that was executed into this block.
    pub txs: Vec<Hash>,
}

/// Transaction information.
#[derive(Debug, Serialize, Deserialize)]
pub struct TxInfo {
    /// `JSON` serialized transaction
    pub content: Value,
    /// Transaction location in block
    pub location: TxLocation,
    /// Proof that transaction really exist in database
    pub proof_to_block_merkle_root: ListProof<Hash>,
    /// Status of the transaction execution.
    pub status: TxStatus,
}

/// Transaction execution status. Simplified version of `TransactionResult`.
#[derive(Debug, Serialize, Deserialize)]
pub enum TxStatus {
    /// Successful transaction execution.
    Success,
    /// Error during transaction execution.
    Error(String),
}

impl<'a> BlockchainExplorer<'a> {
    /// Creates a new `BlockchainExplorer` instance.
    pub fn new(blockchain: &'a Blockchain) -> Self {
        BlockchainExplorer { blockchain }
    }

    /// Returns information about the transaction identified by the hash.
    pub fn tx_info(&self, tx_hash: &Hash) -> Result<Option<TxInfo>, ApiError> {
        let schema = Schema::new(self.blockchain.snapshot());
        Ok(if let Some(raw_tx) = schema.transactions().get(tx_hash) {
            let box_transaction = self.blockchain.tx_from_raw(raw_tx.clone()).ok_or_else(|| {
                ApiError::InternalError(
                    format!("Service not found for tx: {:?}", raw_tx).into(),
                )
            })?;

            let content = box_transaction.serialize_field().map_err(
                ApiError::InternalError,
            )?;

            let location = schema.tx_location_by_tx_hash().get(tx_hash).expect(
                &format!(
                    "Not found tx_hash location: {:?}",
                    tx_hash
                ),
            );

            let proof_to_block_merkle_root = schema.block_txs(location.block_height())
                .get_proof(location.position_in_block());

            // Unwrap is OK here, because we already know that transaction is committed.
            let status = match schema.transaction_results().get(tx_hash).unwrap() {
                Ok(()) => TxStatus::Success,
                Err(e) => TxStatus::Error(format!("{}", e))
            };

            Some(TxInfo {
                content,
                location,
                proof_to_block_merkle_root,
                status,
            })
        } else {
            None
        })
    }

    /// Returns block information for the specified height or `None` if there is no such block.
    pub fn block_info(&self, height: Height) -> Option<BlockInfo> {
        let schema = Schema::new(self.blockchain.snapshot());
        let txs_table = schema.block_txs(height);
        let block_proof = schema.block_and_precommits(height);
        match block_proof {
            None => None,
            Some(proof) => {
                let bl = BlockInfo {
                    block: proof.block,
                    precommits: proof.precommits,
                    txs: txs_table.iter().collect(),
                };
                Some(bl)
            }
        }
    }

    /// Returns the list of blocks in the given range.
    pub fn blocks_range(
        &self,
        count: u64,
        upper: Option<u64>,
        skip_empty_blocks: bool,
    ) -> Vec<Block> {
        let schema = Schema::new(self.blockchain.snapshot());
        let hashes = schema.block_hashes_by_height();
        let blocks = schema.blocks();

        let max_len = hashes.len();
        let upper = upper.map(|x| cmp::min(x, max_len)).unwrap_or(max_len);
        let lower = upper.checked_sub(count).unwrap_or(0);

        let mut v = Vec::new();
        for height in (lower..upper).rev() {
            let block_txs = schema.block_txs(Height(height));
            if skip_empty_blocks && block_txs.is_empty() {
                continue;
            }
            let block_hash = hashes.get(height).expect(&format!(
                "Block not found, height:{:?}",
                height
            ));
            let block = blocks.get(&block_hash).expect(&format!(
                "Block not found, hash:{:?}",
                block_hash
            ));
            v.push(block)
        }
        v
    }

    /// Returns transaction result.
    pub fn transaction_result(&self, hash: &Hash) -> Option<TransactionResult> {
        let schema = Schema::new(self.blockchain.snapshot());
        schema.transaction_results().get(hash)
    }

    //pub fn transaction_
}
