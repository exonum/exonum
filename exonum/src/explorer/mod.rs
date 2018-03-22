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

extern crate linked_hash_map;

use self::linked_hash_map::LinkedHashMap;
use serde::{Serialize, Serializer};

use std::fmt;
use std::ops::{Index, Range};

use storage::{ListProof, Snapshot};
use crypto::Hash;
use blockchain::{Schema, Blockchain, Block, TxLocation, Transaction, TransactionResult,
                 TransactionError, TransactionErrorType};
use messages::Precommit;
use helpers::Height;

#[cfg(any(test, feature = "doctests"))]
#[doc(hidden)]
pub mod tests;

/// Information about a block in the blockchain.
///
/// # Examples
///
/// ```ignore
/// # use exonum::explorer::{BlockchainExplorer, BlockInfo};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(blockchain);
/// let block: BlockInfo = explorer.block(Height(1)).unwrap();
/// assert_eq!(block.block().height(), Height(1));
/// assert_eq!(block.len(), 3);
///
/// // Iterate over transactions in the block
/// for tx in &block {
///     println!("{:?}: {:?}", tx.location(), tx.content());
/// }
/// ```
///
/// # JSON presentation
///
/// ```ignore
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # use exonum::explorer::{BlockchainExplorer, BlockInfo};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// # fn main() {
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(blockchain);
/// let block: BlockInfo = // ...
/// #                      explorer.block(Height(1)).unwrap();
/// assert_eq!(
///     serde_json::to_value(&block).unwrap(),
///     json!({
///         // `Block` representation
///         "block": block.block(),
///         // Array of `Precommit`s
///         "precommits": block.precommits(),
///         // Array of transaction hashes
///         "txs": block.transaction_hashes(),
///     })
/// );
/// # }
/// ```
#[derive(Debug, Serialize)]
pub struct BlockInfo<'a> {
    #[serde(skip)]
    explorer: &'a BlockchainExplorer,
    block: Block,
    precommits: Vec<Precommit>,
    txs: Vec<Hash>,
}

impl<'a> BlockInfo<'a> {
    /// Returns the block header as recorded in the blockchain.
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

    /// Returns a list of precommits for this block.
    pub fn precommits(&self) -> &[Precommit] {
        &self.precommits
    }

    /// List of hashes for transactions that was executed into this block.
    pub fn transaction_hashes(&self) -> &[Hash] {
        &self.txs
    }

    /// Returns a transaction with the specified index in the block.
    pub fn transaction(&self, index: usize) -> Option<CommittedTransaction> {
        self.txs.get(index).map(|hash| {
            self.explorer.committed_transaction(hash, None)
        })
    }

    /// Iterates over transactions in the block.
    pub fn iter(&self) -> TransactionsIter {
        TransactionsIter {
            explorer: self.explorer,
            inner: self.txs.iter(),
        }
    }
}

/// Iterator over transactions in a block.
#[derive(Debug)]
pub struct TransactionsIter<'a> {
    explorer: &'a BlockchainExplorer,
    inner: ::std::slice::Iter<'a, Hash>,
}

impl<'a> Iterator for TransactionsIter<'a> {
    type Item = CommittedTransaction;

    fn next(&mut self) -> Option<CommittedTransaction> {
        self.inner.next().map(|hash| {
            self.explorer.committed_transaction(hash, None)
        })
    }
}

impl<'a, 'r: 'a> IntoIterator for &'r BlockInfo<'a> {
    type Item = CommittedTransaction;
    type IntoIter = TransactionsIter<'a>;

    fn into_iter(self) -> TransactionsIter<'a> {
        self.iter()
    }
}

/// Information about a block in the blockchain with info on transactions eagerly loaded.
///
/// # Examples
///
/// ```ignore
/// # use exonum::explorer::{BlockchainExplorer, BlockWithTransactions, CommittedTransaction};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(blockchain);
/// let block: BlockWithTransactions = explorer.block_with_txs(Height(1)).unwrap();
/// assert_eq!(block.block().height(), Height(1));
/// assert_eq!(block.len(), 3);
///
/// // Iterate over transactions in the block
/// for tx in &block {
///     println!("{:?}: {:?}", tx.location(), tx.content());
/// }
///
/// // Compared to `BlockInfo`, you can access transactions in a block using indexes
/// let tx: &CommittedTransaction = &block[1];
/// assert_eq!(tx.location().position_in_block(), 1);
/// let tx_copy = &block[&tx.content().hash()];
/// assert_eq!(tx.content().raw(), tx_copy.content().raw());
/// ```
#[derive(Debug)]
pub struct BlockWithTransactions {
    block: Block,
    precommits: Vec<Precommit>,
    txs: LinkedHashMap<Hash, CommittedTransaction>,
}

impl BlockWithTransactions {
    /// Returns the block header as recorded in the blockchain.
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

    /// Returns a list of precommits for this block.
    pub fn precommits(&self) -> &[Precommit] {
        &self.precommits
    }

    /// Returns a transaction with the specified index in the block.
    pub fn transaction(&self, index: usize) -> Option<&CommittedTransaction> {
        self.txs.values().nth(index)
    }

    /// Returns a transaction with the specified hash in the block.
    pub fn transaction_by_hash(&self, hash: &Hash) -> Option<&CommittedTransaction> {
        self.txs.get(hash)
    }

    /// Iterates over transactions in the block.
    pub fn iter(&self) -> EagerTransactionsIter {
        self.txs.values()
    }
}

/// Iterator over transactions in [`BlockWithTransactions`].
///
/// [`BlockWithTransactions`]: struct.BlockWithTransactions.html
pub type EagerTransactionsIter<'a> = self::linked_hash_map::Values<'a, Hash, CommittedTransaction>;

impl Index<usize> for BlockWithTransactions {
    type Output = CommittedTransaction;

    fn index(&self, index: usize) -> &CommittedTransaction {
        self.transaction(index).expect(&format!(
            "Index exceeds number of transactions in block {}",
            self.len()
        ))
    }
}

impl<'a> Index<&'a Hash> for BlockWithTransactions {
    type Output = CommittedTransaction;

    fn index(&self, tx_hash: &'a Hash) -> &CommittedTransaction {
        self.transaction_by_hash(tx_hash).expect(&format!(
            "Transaction with hash {:?} not in block",
            tx_hash
        ))
    }
}

impl<'a> IntoIterator for &'a BlockWithTransactions {
    type Item = &'a CommittedTransaction;
    type IntoIter = EagerTransactionsIter<'a>;

    fn into_iter(self) -> EagerTransactionsIter<'a> {
        self.iter()
    }
}

/// Information about a particular transaction in the blockchain.
///
/// # Examples
///
/// ```ignore
/// use exonum::blockchain::{Transaction, TransactionError};
/// # use exonum::explorer::{BlockchainExplorer, CommittedTransaction};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
///
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(blockchain);
/// let tx = explorer.block(Height(1)).unwrap().transaction(0).unwrap();
/// assert_eq!(tx.location().block_height(), Height(1));
/// assert_eq!(tx.location().position_in_block(), 0);
///
/// // It is possible to access transaction content
/// let content: &Transaction = tx.content();
/// println!("{:?}", content);
///
/// // ...and transaction status as well
/// let status: Result<(), &TransactionError> = tx.status();
/// assert!(status.is_ok());
/// ```
///
/// # JSON presentation
///
/// ```ignore
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # use exonum::explorer::{BlockchainExplorer, CommittedTransaction};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// use exonum::encoding::serialize::json::ExonumJson;
///
/// # fn main() {
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(blockchain);
/// let tx = explorer.block(Height(1)).unwrap().transaction(0).unwrap();
/// assert_eq!(
///     serde_json::to_value(&tx).unwrap(),
///     json!({
///         // `Transaction` JSON presentation
///         "content": tx.content().serialize_field().unwrap(),
///         // Position in block
///         "location": {
///             "block_height": "1",
///             "position_in_block": "0",
///         },
///         // `ListProof` of the transaction inclusion in block
///         "location_proof": tx.location_proof(),
///         // Execution status
///         "status": { "type": "success" },
///     })
/// );
/// # }
/// ```
///
/// ## Erroneous transactions
///
/// Transactions which execution has resulted in a user-defined error
/// (i.e., one returned as `Err(..)` from `Transaction::execute`)
/// have `code` and `description` fields in `status` and have `type` set to `"error"`:
///
/// ```ignore
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # use exonum::encoding::serialize::json::ExonumJson;
/// # use exonum::explorer::{BlockchainExplorer, CommittedTransaction};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// #
/// # fn main() {
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(blockchain);
/// let erroneous_tx: CommittedTransaction = // ...
/// #   explorer.block(Height(1)).unwrap().transaction(1).unwrap();
/// assert_eq!(
///     serde_json::to_value(&erroneous_tx).unwrap(),
///     json!({
///         "status": {
///             "type": "error",
///             "code": 1,
///             "description": "Not allowed",
///         },
///         // Other fields...
/// #       "content": erroneous_tx.content().serialize_field().unwrap(),
/// #       "location": erroneous_tx.location(),
/// #       "location_proof": erroneous_tx.location_proof(),
///     })
/// );
/// # }
/// ```
///
/// ## Panicking transactions
///
/// If transaction execution resulted in panic, it has `type` set to `"panic"`:
///
/// ```ignore
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # use exonum::encoding::serialize::json::ExonumJson;
/// # use exonum::explorer::{BlockchainExplorer, CommittedTransaction};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// #
/// # fn main() {
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(blockchain);
/// let panicked_tx: CommittedTransaction = // ...
/// #   explorer.block(Height(1)).unwrap().transaction(2).unwrap();
/// assert_eq!(
///     serde_json::to_value(&panicked_tx).unwrap(),
///     json!({
///         "status": { "type": "panic", "description": "oops" },
///         // Other fields...
/// #       "content": panicked_tx.content().serialize_field().unwrap(),
/// #       "location": panicked_tx.location(),
/// #       "location_proof": panicked_tx.location_proof(),
///     })
/// );
/// # }
/// ```
#[derive(Debug, Serialize)]
pub struct CommittedTransaction {
    #[serde(serialize_with = "CommittedTransaction::serialize_content")]
    content: Box<Transaction>,
    location: TxLocation,
    location_proof: ListProof<Hash>,
    #[serde(serialize_with = "CommittedTransaction::serialize_status")]
    status: TransactionResult,
}

impl CommittedTransaction {
    /// Returns the content of the transaction.
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

    /// Returns the transaction location in block.
    pub fn location(&self) -> &TxLocation {
        &self.location
    }

    /// Returns a proof that transaction is recorded in the blockchain.
    pub fn location_proof(&self) -> &ListProof<Hash> {
        &self.location_proof
    }

    /// Returns the status of the transaction execution.
    pub fn status(&self) -> Result<(), &TransactionError> {
        self.status.as_ref().map(|_| ())
    }
}

/// Information about the transaction.
///
/// Values of this type are returned by the [`transaction()`] method of the `BlockchainExplorer`.
///
/// [`transaction()`]: struct.BlockchainExplorer.html#method.transaction
///
/// # Examples
///
/// ```ignore
/// # use exonum::explorer::{BlockchainExplorer, TransactionInfo};
/// # use exonum::explorer::tests::{sample_blockchain, mempool_transaction};
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(blockchain);
/// let hash = // ...
/// #          mempool_transaction().hash();
/// let tx: TransactionInfo = explorer.transaction(&hash).unwrap();
/// assert!(tx.is_in_pool());
/// println!("{:?}", tx.content());
/// ```
///
/// # JSON presentation
///
/// ## Committed transactions
///
/// Committed transactions are represented just like a [`CommittedTransaction`],
/// with the additional `type` field equal to `"committed"`.
///
/// [`CommittedTransaction`]: struct.CommittedTransaction.html#json-presentation
///
/// ```ignore
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # use exonum::explorer::{BlockchainExplorer, TransactionInfo};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// use exonum::encoding::serialize::json::ExonumJson;
///
/// # fn main() {
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(blockchain);
/// # let block = explorer.block(Height(1)).unwrap();
/// let committed_tx: TransactionInfo = // ...
/// #   explorer.transaction(&block.transaction_hashes()[0]).unwrap();
/// # let tx_ref = committed_tx.as_committed().unwrap();
/// assert_eq!(
///     serde_json::to_value(&committed_tx).unwrap(),
///     json!({
///         "type": "committed",
///         "content": committed_tx.content().serialize_field().unwrap(),
///         "status": { "type": "success" },
///         // Other fields...
/// #       "location": tx_ref.location(),
/// #       "location_proof": tx_ref.location_proof(),
///     })
/// );
/// # }
/// ```
///
/// ## Transaction in pool
///
/// Transactions in pool are represented with a 2-field object:
///
/// - `type` field contains transaction type (`"in-pool"`).
/// - `content` is JSON serialization of the transaction.
///
/// ```ignore
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # use exonum::explorer::{BlockchainExplorer, TransactionInfo};
/// # use exonum::explorer::tests::{sample_blockchain, mempool_transaction};
/// use exonum::encoding::serialize::json::ExonumJson;
///
/// # fn main() {
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(blockchain);
/// let tx_in_pool: TransactionInfo = // ...
/// #   explorer.transaction(&mempool_transaction().hash()).unwrap();
/// assert_eq!(
///     serde_json::to_value(&tx_in_pool).unwrap(),
///     json!({
///         "type": "in-pool",
///         "content": tx_in_pool.content().serialize_field().unwrap(),
///     })
/// );
/// # }
/// ```
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TransactionInfo {
    /// Transaction is in the memory pool, but not yet committed to the blockchain.
    InPool {
        /// Transaction contents.
        #[serde(serialize_with = "CommittedTransaction::serialize_content")]
        content: Box<Transaction>,
    },

    /// Transaction is already committed to the blockchain.
    Committed(CommittedTransaction),
}

impl TransactionInfo {
    /// Returns the content of this transaction.
    pub fn content(&self) -> &Transaction {
        match *self {
            TransactionInfo::InPool { ref content } => content.as_ref(),
            TransactionInfo::Committed(ref tx) => tx.content(),
        }
    }

    /// Is this in-pool transaction?
    pub fn is_in_pool(&self) -> bool {
        match *self {
            TransactionInfo::InPool { .. } => true,
            _ => false,
        }
    }

    /// Is this a committed transaction?
    pub fn is_committed(&self) -> bool {
        match *self {
            TransactionInfo::Committed(_) => true,
            _ => false,
        }
    }

    /// Returns a reference to the inner committed transaction if this transaction is committed.
    /// For transactions in pool, returns `None`.
    pub fn as_committed(&self) -> Option<&CommittedTransaction> {
        match *self {
            TransactionInfo::Committed(ref tx) => Some(tx),
            _ => None,
        }
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

        if schema.transactions_pool().contains(tx_hash) {
            return Some(TransactionInfo::InPool { content });
        }

        let tx = self.committed_transaction(tx_hash, Some(content));
        Some(TransactionInfo::Committed(tx))
    }

    /// Retrieves a transaction that is known to be committed.
    fn committed_transaction(
        &self,
        tx_hash: &Hash,
        maybe_content: Option<Box<Transaction>>,
    ) -> CommittedTransaction {
        let schema = Schema::new(self.blockchain.snapshot());

        let location = schema.transactions_locations().get(tx_hash).expect(
            &format!(
                "Not found tx_hash location: {:?}",
                tx_hash
            ),
        );

        let location_proof = schema
            .block_transactions(location.block_height())
            .get_proof(location.position_in_block());

        // Unwrap is OK here, because we already know that transaction is committed.
        let status = schema.transaction_results().get(tx_hash).unwrap();

        CommittedTransaction {
            content: maybe_content.unwrap_or_else(|| {
                let raw_tx = schema.transactions().get(tx_hash).unwrap();
                self.blockchain.tx_from_raw(raw_tx).unwrap()
            }),

            location,
            location_proof,
            status,
        }
    }

    /// Returns block information for the specified height or `None` if there is no such block.
    pub fn block(&self, height: Height) -> Option<BlockInfo> {
        let schema = Schema::new(self.blockchain.snapshot());
        let txs_table = schema.block_transactions(height);
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

    /// Returns block information for the specified height or `None` if there is no such block.
    pub fn block_with_txs(&self, height: Height) -> Option<BlockWithTransactions> {
        let schema = Schema::new(self.blockchain.snapshot());
        let txs_table = schema.block_transactions(height);
        let block_proof = schema.block_and_precommits(height);

        block_proof.map(|proof| {
            BlockWithTransactions {
                block: proof.block,
                precommits: proof.precommits,
                txs: txs_table
                    .iter()
                    .map(|tx_hash| {
                        (tx_hash, self.committed_transaction(&tx_hash, None))
                    })
                    .collect(),
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

    /// Iterates over blocks in the descending order, optionally skipping empty blocks.
    pub fn blocks_rev(&self, skip_empty: bool) -> BlocksIter {
        let schema = Schema::new(self.blockchain.snapshot());
        let height = schema.height();

        BlocksIter {
            schema,
            skip_empty,
            height: Some(height),
        }
    }

    /// Returns transaction result for a certain transaction.
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
            let is_empty = self.schema.block_transactions(height).is_empty();

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
