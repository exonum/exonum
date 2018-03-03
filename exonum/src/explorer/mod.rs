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

use serde::{Serialize, Serializer};

use std::fmt;
use std::ops::Range;

use storage::{ListProof, Snapshot};
use crypto::Hash;
use blockchain::{Schema, Blockchain, Block, TxLocation, Transaction, TransactionResult,
                 TransactionErrorType};
use messages::Precommit;
use helpers::Height;

#[cfg(test)]
mod tests;

/// Block information.
#[derive(Debug, Serialize)]
pub struct BlockInfo<'a> {
    #[serde(skip)]
    explorer: &'a BlockchainExplorer,
    block: Block,
    precommits: Vec<Precommit>,
    txs: Vec<Hash>,
}

impl<'a> BlockInfo<'a> {
    /// Block header from blockchain.
    pub fn block(&self) -> &Block {
        &self.block
    }

    /// Returns the number of transactions in this block.
    pub fn len(&self) -> usize {
        self.txs.len()
    }

    /// Is this block empty (i.e., contains no transactions)?
    pub fn is_empty(&self) -> bool {
        self.txs.is_empty()
    }

    /// List of precommit for this block.
    pub fn precommits(&self) -> &[Precommit] {
        &self.precommits
    }

    /// List of hashes for transactions that was executed into this block.
    pub fn transaction_hashes(&self) -> &[Hash] {
        &self.txs
    }

    /// Returns a transaction with the specified index in the block.
    pub fn transaction(&self, index: usize) -> Option<TransactionInfo> {
        self.txs.get(index).map(|hash| {
            self.explorer.transaction(hash).unwrap()
        })
    }
}

/// Transaction information.
#[derive(Debug, Serialize)]
pub struct TransactionInfo {
    #[serde(serialize_with = "TransactionInfo::serialize_content")]
    content: Box<Transaction>,
    location: TxLocation,
    location_proof: ListProof<Hash>,
    #[serde(serialize_with = "TransactionInfo::serialize_status")]
    status: TransactionResult,
}

impl TransactionInfo {
    /// The content of transaction.
    pub fn content(&self) -> &Transaction {
        self.content.as_ref()
    }

    fn serialize_content<S, T>(tx: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: AsRef<Transaction>,
    {
        use serde::ser::Error;

        let value = tx.as_ref().serialize_field().map_err(|err| {
            S::Error::custom(err.description())
        })?;
        value.serialize(serializer)
    }

    fn serialize_status<S>(result: &TransactionResult, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        /// Transaction execution status. Simplified version of `TransactionResult`.
        #[serde(tag = "type", rename_all = "kebab-case")]
        #[derive(Debug, Serialize)]
        enum TxStatus<'a> {
            Success,
            Panic { description: &'a str },
            Error { code: u8, description: &'a str },
        }

        fn from(result: &TransactionResult) -> TxStatus {
            use self::TransactionErrorType::*;

            match *result {
                Ok(()) => TxStatus::Success,
                Err(ref e) => {
                    let description = e.description().unwrap_or_default();
                    match e.error_type() {
                        Panic => TxStatus::Panic { description },
                        Code(code) => TxStatus::Error { code, description },
                    }
                }
            }
        }

        let status = from(result);
        status.serialize(serializer)
    }

    /// Transaction location in block.
    pub fn location(&self) -> &TxLocation {
        &self.location
    }

    /// Proof that transaction really exist in the database.
    pub fn location_proof(&self) -> &ListProof<Hash> {
        &self.location_proof
    }

    /// Status of the transaction execution.
    pub fn status(&self) -> &TransactionResult {
        &self.status
    }
}


/// Information on blocks coupled with the corresponding range in the blockchain.
#[derive(Debug, Serialize, Deserialize)]
pub struct BlocksRange {
    /// Exclusive range of blocks.
    pub range: Range<Height>,
    /// Blocks in the range.
    pub blocks: Vec<Block>,
}

/// Blockchain explorer.
#[derive(Debug, Clone)]
pub struct BlockchainExplorer {
    blockchain: Blockchain,
}

impl BlockchainExplorer {
    /// Creates a new `BlockchainExplorer` instance.
    pub fn new(blockchain: Blockchain) -> Self {
        BlockchainExplorer { blockchain }
    }

    /// Returns information about the transaction identified by the hash.
    pub fn transaction(&self, tx_hash: &Hash) -> Option<TransactionInfo> {
        let schema = Schema::new(self.blockchain.snapshot());
        let raw_tx = schema.transactions().get(tx_hash)?;

        let content = self.blockchain.tx_from_raw(raw_tx.clone());
        if content.is_none() {
            error!("Service not found for tx: {:?}", raw_tx);
            return None;
        }
        let content = content.unwrap();

        let location = schema.tx_location_by_tx_hash().get(tx_hash).expect(
            &format!(
                "Not found tx_hash location: {:?}",
                tx_hash
            ),
        );

        let location_proof = schema.block_txs(location.block_height()).get_proof(
            location.position_in_block(),
        );

        // Unwrap is OK here, because we already know that transaction is committed.
        let status = schema.transaction_results().get(tx_hash).unwrap();
        Some(TransactionInfo {
            content,
            location,
            location_proof,
            status,
        })
    }

    /// Returns block information for the specified height or `None` if there is no such block.
    pub fn block(&self, height: Height) -> Option<BlockInfo> {
        let schema = Schema::new(self.blockchain.snapshot());
        let txs_table = schema.block_txs(height);
        let block_proof = schema.block_and_precommits(height);

        block_proof.map(|proof| {
            BlockInfo {
                explorer: self,
                block: proof.block,
                precommits: proof.precommits,
                txs: txs_table.iter().collect(),
            }
        })
    }

    /// Returns the list of blocks in the given range.
    pub fn blocks_range(
        &self,
        count: usize,
        upper: Option<Height>,
        skip_empty_blocks: bool,
    ) -> BlocksRange {
        let mut blocks_iter = self.blocks_rev(skip_empty_blocks);
        if let Some(upper) = upper {
            blocks_iter.skip_to(upper);
        }

        // Safe: we haven't iterated yet, and there is at least the genesis block.
        let upper = blocks_iter.height.unwrap();

        let blocks: Vec<_> = blocks_iter.by_ref().take(count).collect();
        let height = blocks_iter.last_seen_height();

        BlocksRange {
            range: height..upper.next(),
            blocks,
        }
    }

    /// Iterator over blocks in the descending order.
    pub fn blocks_rev(&self, skip_empty: bool) -> BlocksIter {
        let schema = Schema::new(self.blockchain.snapshot());
        let height = schema.height();

        BlocksIter {
            schema,
            skip_empty,
            height: Some(height),
        }
    }

    /// Returns transaction result.
    pub fn transaction_result(&self, hash: &Hash) -> Option<TransactionResult> {
        let schema = Schema::new(self.blockchain.snapshot());
        schema.transaction_results().get(hash)
    }
}

/// Iterator over blocks in descending order.
pub struct BlocksIter {
    skip_empty: bool,
    schema: Schema<Box<Snapshot>>,
    height: Option<Height>,
}

impl fmt::Debug for BlocksIter {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        formatter
            .debug_struct("BlocksIter")
            .field("skip_empty", &self.skip_empty)
            .field("height", &self.height)
            .finish()
    }
}

impl BlocksIter {
    /// Skips the iterator to the specified height.
    /// Has no effect if the specified height is greater or equal than the current height
    /// of the iterator.
    pub fn skip_to(&mut self, height: Height) -> &mut Self {
        match self.height {
            Some(ref mut self_height) if *self_height > height => {
                *self_height = height;
            }
            _ => {}
        }

        self
    }

    fn decrease_height(&mut self) {
        self.height = match self.height {
            Some(Height(0)) => None,
            Some(height) => Some(height.previous()),
            None => unreachable!(),
        }
    }

    fn last_seen_height(&self) -> Height {
        self.height.map(|h| h.next()).unwrap_or(Height(0))
    }
}

impl Iterator for BlocksIter {
    type Item = Block;

    fn next(&mut self) -> Option<Block> {
        if self.height.is_none() {
            return None;
        }

        while let Some(height) = self.height {
            let is_empty = self.schema.block_txs(height).is_empty();

            if !self.skip_empty || !is_empty {
                let block = {
                    let hashes = self.schema.block_hashes_by_height();
                    let blocks = self.schema.blocks();

                    let block_hash = hashes.get(height.0).expect(&format!(
                        "Block not found, height:{:?}",
                        height
                    ));
                    blocks.get(&block_hash).expect(&format!(
                        "Block not found, hash:{:?}",
                        block_hash
                    ))
                };

                self.decrease_height();
                return Some(block);
            }

            self.decrease_height();
        }

        None
    }
}
