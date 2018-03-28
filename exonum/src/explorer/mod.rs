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

//! Blockchain explorer module provides API for getting information about blocks and transactions
//! from the blockchain.
//!
//! See the [`BlockchainExplorer`] documentation for examples of usage.
//!
//! [`BlockchainExplorer`]: struct.BlockchainExplorer.html

use serde::{Serialize, Serializer};

use crypto::{CryptoHash, Hash};
use blockchain::{Schema, Blockchain, Block, TxLocation, Transaction, TransactionResult,
                 TransactionError, TransactionErrorType};
use encoding;
use helpers::Height;
use messages::{Precommit, RawMessage};
use storage::{ListProof, Snapshot};

use std::cell::{Ref, RefCell};
use std::collections::Bound;
use std::fmt;
use std::ops::{Index, Range, RangeFrom, RangeFull, RangeTo};

#[cfg(any(test, feature = "doctests"))]
#[doc(hidden)]
pub mod tests;

/// Transaction parsing result.
type ParseResult = Result<Box<Transaction>, encoding::Error>;

/// Range of `Height`s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightRange(pub Bound<Height>, pub Bound<Height>);

impl From<RangeFull> for HeightRange {
    fn from(_: RangeFull) -> HeightRange {
        HeightRange(Bound::Unbounded, Bound::Unbounded)
    }
}

impl From<Range<Height>> for HeightRange {
    fn from(range: Range<Height>) -> HeightRange {
        HeightRange(Bound::Included(range.start), Bound::Excluded(range.end))
    }
}

impl From<RangeFrom<Height>> for HeightRange {
    fn from(range: RangeFrom<Height>) -> HeightRange {
        HeightRange(Bound::Included(range.start), Bound::Unbounded)
    }
}

impl From<RangeTo<Height>> for HeightRange {
    fn from(range: RangeTo<Height>) -> HeightRange {
        HeightRange(Bound::Unbounded, Bound::Excluded(range.end))
    }
}

impl HeightRange {
    /// Ending height of the range (exclusive), given the a priori max height.
    fn end_height(&self, max: Height) -> Height {
        use std::cmp::min;

        let inner_end = match self.1 {
            Bound::Included(height) => height.next(),
            Bound::Excluded(height) => height,
            Bound::Unbounded => max.next(),
        };

        min(inner_end, max.next())
    }

    fn start_height(&self) -> Height {
        match self.0 {
            Bound::Included(height) => height,
            Bound::Excluded(height) => height.next(),
            Bound::Unbounded => Height(0),
        }
    }
}

/// Information about a block in the blockchain.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::explorer::{BlockchainExplorer, BlockInfo};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(&blockchain);
/// let block: BlockInfo = explorer.block(Height(1)).unwrap();
/// assert_eq!(block.height(), Height(1));
/// assert_eq!(block.len(), 3);
///
/// // Iterate over transactions in the block
/// for tx in &block {
///     println!("{:?}: {:?}", tx.location(), tx.content());
/// }
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
/// ```
///
/// # JSON presentation
///
/// ```
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::helpers::Height;
/// # use exonum::explorer::{BlockchainExplorer, BlockInfo};
/// # use exonum::explorer::tests::sample_blockchain;
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(&blockchain);
/// let block: BlockInfo = // ...
/// #                      explorer.block(Height(1)).unwrap();
/// assert_eq!(
///     serde_json::to_value(&block).unwrap(),
///     json!({
///         // `Block` representation
///         "block": block.header(),
///         // Array of `Precommit`s
///         "precommits": *block.precommits(),
///         // Array of transaction hashes
///         "txs": *block.transaction_hashes(),
///     })
/// );
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
/// ```
#[derive(Debug)]
pub struct BlockInfo<'a> {
    header: Block,
    explorer: &'a BlockchainExplorer<'a>,
    precommits: RefCell<Option<Vec<Precommit>>>,
    txs: RefCell<Option<Vec<Hash>>>,
}

impl<'a> BlockInfo<'a> {
    fn new(explorer: &'a BlockchainExplorer, height: Height) -> Self {
        let schema = Schema::new(&explorer.snapshot);
        let header = {
            let hashes = schema.block_hashes_by_height();
            let blocks = schema.blocks();

            let block_hash = hashes.get(height.0).expect(&format!(
                "Block not found, height: {:?}",
                height
            ));
            blocks.get(&block_hash).expect(&format!(
                "Block not found, hash: {:?}",
                block_hash
            ))
        };

        BlockInfo {
            explorer,
            header,
            precommits: RefCell::new(None),
            txs: RefCell::new(None),
        }
    }

    /// Returns block header as recorded in the blockchain.
    pub fn header(&self) -> &Block {
        &self.header
    }

    /// Extracts the header discarding all other information.
    pub fn into_header(self) -> Block {
        self.header
    }

    /// Returns the height of this block.
    ///
    /// This method is equivalent to calling `block.header().height()`.
    pub fn height(&self) -> Height {
        self.header.height()
    }

    /// Returns the number of transactions in this block.
    pub fn len(&self) -> usize {
        self.header.tx_count() as usize
    }

    /// Is this block empty (i.e., contains no transactions)?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a list of precommits for this block.
    pub fn precommits(&self) -> Ref<[Precommit]> {
        if self.precommits.borrow().is_none() {
            let precommits = self.explorer.precommits(&self.header);
            *self.precommits.borrow_mut() = Some(precommits);
        }

        Ref::map(self.precommits.borrow(), |cache| {
            cache.as_ref().unwrap().as_ref()
        })
    }

    /// Lists hashes of transactions included in this block.
    pub fn transaction_hashes(&self) -> Ref<[Hash]> {
        if self.txs.borrow().is_none() {
            let txs = self.explorer.transaction_hashes(&self.header);
            *self.txs.borrow_mut() = Some(txs);
        }

        Ref::map(self.txs.borrow(), |cache| cache.as_ref().unwrap().as_ref())
    }

    /// Returns a transaction with the specified index in the block.
    pub fn transaction(&self, index: usize) -> Option<CommittedTransaction> {
        self.transaction_hashes().get(index).map(|hash| {
            self.explorer.committed_transaction(hash, None)
        })
    }

    /// Iterates over transactions in the block.
    pub fn iter(&self) -> Transactions {
        Transactions {
            block: self,
            ptr: 0,
            len: self.len(),
        }
    }

    /// Loads transactions and precommits for the block.
    pub fn with_transactions(self) -> BlockWithTransactions {
        let (explorer, header, precommits, transactions) =
            (self.explorer, self.header, self.precommits, self.txs);

        let precommits = precommits.into_inner().unwrap_or_else(
            || explorer.precommits(&header),
        );
        let transactions = transactions
            .into_inner()
            .unwrap_or_else(|| explorer.transaction_hashes(&header))
            .iter()
            .map(|tx_hash| explorer.committed_transaction(tx_hash, None))
            .collect();

        BlockWithTransactions {
            header,
            precommits,
            transactions,
        }
    }
}

impl<'a> Serialize for BlockInfo<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut s = serializer.serialize_struct("BlockInfo", 3)?;
        s.serialize_field("block", &self.header)?;
        s.serialize_field("precommits", &*self.precommits())?;
        s.serialize_field("txs", &*self.transaction_hashes())?;
        s.end()
    }
}

/// Iterator over transactions in a block.
#[derive(Debug)]
pub struct Transactions<'r, 'a: 'r> {
    block: &'r BlockInfo<'a>,
    ptr: usize,
    len: usize,
}

impl<'a, 'r> Iterator for Transactions<'a, 'r> {
    type Item = CommittedTransaction;

    fn next(&mut self) -> Option<CommittedTransaction> {
        if self.ptr == self.len {
            None
        } else {
            let transaction = self.block.transaction(self.ptr);
            self.ptr += 1;
            transaction
        }
    }
}

impl<'a, 'r: 'a> IntoIterator for &'r BlockInfo<'a> {
    type Item = CommittedTransaction;
    type IntoIter = Transactions<'a, 'r>;

    fn into_iter(self) -> Transactions<'a, 'r> {
        self.iter()
    }
}

/// Information about a block in the blockchain with info on transactions eagerly loaded.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::explorer::{BlockchainExplorer, BlockWithTransactions, CommittedTransaction};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(&blockchain);
/// let block: BlockWithTransactions = explorer.block_with_txs(Height(1)).unwrap();
/// assert_eq!(block.height(), Height(1));
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
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
/// ```
#[derive(Debug, Serialize)]
pub struct BlockWithTransactions {
    /// Block header as recorded in the blockchain.
    #[serde(rename = "block")]
    pub header: Block,
    /// Precommits.
    pub precommits: Vec<Precommit>,
    /// Transactions in the order they appear in the block.
    pub transactions: Vec<CommittedTransaction>,
}

impl BlockWithTransactions {
    /// Returns the height of this block.
    ///
    /// This method is equivalent to calling `block.header.height()`.
    pub fn height(&self) -> Height {
        self.header.height()
    }

    /// Returns the number of transactions in this block.
    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    /// Is this block empty (i.e., contains no transactions)?
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    /// Iterates over transactions in the block.
    pub fn iter(&self) -> EagerTransactions {
        self.transactions.iter()
    }
}

/// Iterator over transactions in [`BlockWithTransactions`].
///
/// [`BlockWithTransactions`]: struct.BlockWithTransactions.html
pub type EagerTransactions<'a> = ::std::slice::Iter<'a, CommittedTransaction>;

impl Index<usize> for BlockWithTransactions {
    type Output = CommittedTransaction;

    fn index(&self, index: usize) -> &CommittedTransaction {
        self.transactions.get(index).expect(&format!(
            "Index exceeds number of transactions in block {}",
            self.len()
        ))
    }
}

impl<'a> IntoIterator for &'a BlockWithTransactions {
    type Item = &'a CommittedTransaction;
    type IntoIter = EagerTransactions<'a>;

    fn into_iter(self) -> EagerTransactions<'a> {
        self.iter()
    }
}

/// Information about a particular transaction in the blockchain.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "doctests")] fn main() {
/// use exonum::blockchain::{Transaction, TransactionError};
/// # use exonum::explorer::{BlockchainExplorer, CommittedTransaction};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
///
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(&blockchain);
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
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
/// ```
///
/// # JSON presentation
///
/// ```
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::explorer::{BlockchainExplorer, CommittedTransaction};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// use exonum::encoding::serialize::json::ExonumJson;
///
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(&blockchain);
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
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
/// ```
///
/// ## Erroneous transactions
///
/// Transactions which execution has resulted in a user-defined error
/// (i.e., one returned as `Err(..)` from `Transaction::execute`)
/// have `code` and `description` fields in `status` and have `type` set to `"error"`:
///
/// ```
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::encoding::serialize::json::ExonumJson;
/// # use exonum::explorer::{BlockchainExplorer, CommittedTransaction};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// #
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(&blockchain);
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
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
/// ```
///
/// ## Panicking transactions
///
/// If transaction execution resulted in panic, it has `type` set to `"panic"`:
///
/// ```
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::encoding::serialize::json::ExonumJson;
/// # use exonum::explorer::{BlockchainExplorer, CommittedTransaction};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// #
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(&blockchain);
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
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
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
/// ```
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::explorer::{BlockchainExplorer, TransactionInfo};
/// # use exonum::explorer::tests::{sample_blockchain, mempool_transaction};
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(&blockchain);
/// let hash = // ...
/// #          mempool_transaction().hash();
/// let tx: TransactionInfo = explorer.transaction(&hash).unwrap();
/// assert!(tx.is_in_pool());
/// println!("{:?}", tx.content());
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
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
/// ```
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::explorer::{BlockchainExplorer, TransactionInfo};
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// use exonum::encoding::serialize::json::ExonumJson;
///
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(&blockchain);
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
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
/// ```
///
/// ## Transaction in pool
///
/// Transactions in pool are represented with a 2-field object:
///
/// - `type` field contains transaction type (`"in-pool"`).
/// - `content` is JSON serialization of the transaction.
///
/// ```
/// # #[macro_use] extern crate serde_json;
/// # extern crate exonum;
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::explorer::{BlockchainExplorer, TransactionInfo};
/// # use exonum::explorer::tests::{sample_blockchain, mempool_transaction};
/// use exonum::encoding::serialize::json::ExonumJson;
///
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(&blockchain);
/// let tx_in_pool: TransactionInfo = // ...
/// #   explorer.transaction(&mempool_transaction().hash()).unwrap();
/// assert_eq!(
///     serde_json::to_value(&tx_in_pool).unwrap(),
///     json!({
///         "type": "in-pool",
///         "content": tx_in_pool.content().serialize_field().unwrap(),
///     })
/// );
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
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
///
/// # Notes
///
/// The explorer wraps a specific [`Snapshot`] of the blockchain state; that is,
/// all calls to the methods of an explorer instance are guaranteed to be consistent.
///
/// [`Snapshot`]: ../storage/trait.Snapshot.html
///
/// # Examples
///
/// ```
/// # extern crate exonum;
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::crypto::Hash;
/// # use exonum::explorer::BlockchainExplorer;
/// # use exonum::explorer::tests::sample_blockchain;
/// # use exonum::helpers::Height;
/// #
/// let blockchain = // ...
/// #                sample_blockchain();
/// let explorer = BlockchainExplorer::new(&blockchain);
///
/// // Getting a specific block
/// let block = explorer.block(Height(1)).unwrap();
/// // Getting a block together with its transactions
/// let block = explorer.block_with_txs(Height(1)).unwrap();
/// // Getting a specific transaction either in one of blocks,
/// // or in the pool of unconfirmed transactions
/// let tx = explorer.transaction(&Hash::zero());
/// // Iterating over blocks in the blockchain
/// for block in explorer.blocks(Height(1)..).rev().take(2) {
///     println!("{:?}", block);
/// }
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
/// ```
///
/// Some more interesting capabilities:
///
/// ```
/// # extern crate exonum;
/// # #[cfg(feature = "doctests")] fn main() {
/// # use exonum::explorer::BlockchainExplorer;
/// # use exonum::explorer::tests::sample_blockchain;
/// use exonum::helpers::{Height, ValidatorId};
///
/// # let blockchain = sample_blockchain();
/// # let explorer = BlockchainExplorer::new(&blockchain);
/// // Calculate the total number of transactions in the first 10 blocks
/// let tx_count: usize = explorer
///     .blocks(..Height(10))
///     .map(|block| block.len())
///     .sum();
/// # assert_eq!(tx_count, 3);
/// // Determine the number of blocks proposed by a specific validator
/// let block_count = explorer
///     .blocks(Height(1)..) // skip genesis block
///     .filter(|block| block.header().proposer_id() == ValidatorId(0))
///     .count();
/// # assert_eq!(block_count, 1);
/// # } // main
/// # #[cfg(not(feature = "doctests"))] fn main() {}
/// ```
pub struct BlockchainExplorer<'a> {
    snapshot: Box<Snapshot>,
    transaction_parser: Box<'a + Fn(RawMessage) -> ParseResult>,
}

impl<'a> fmt::Debug for BlockchainExplorer<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        formatter.pad("BlockchainExplorer { .. }")
    }
}

impl<'a> BlockchainExplorer<'a> {
    /// Creates a new `BlockchainExplorer` instance.
    pub fn new(blockchain: &'a Blockchain) -> Self {
        BlockchainExplorer {
            snapshot: blockchain.snapshot(),
            transaction_parser: Box::new(move |raw| blockchain.tx_from_raw(raw)),
        }
    }

    /// Returns information about the transaction identified by the hash.
    pub fn transaction(&self, tx_hash: &Hash) -> Option<TransactionInfo> {
        let schema = Schema::new(&self.snapshot);
        let raw_tx = schema.transactions().get(tx_hash)?;

        let content = (self.transaction_parser)(raw_tx.clone());
        if let Err(e) = content {
            error!("Error while parsing transaction {:?}: {}", raw_tx, e);
            return None;
        }
        let content = content.unwrap();

        if schema.transactions_pool().contains(tx_hash) {
            return Some(TransactionInfo::InPool { content });
        }

        let tx = self.committed_transaction(tx_hash, Some(content));
        Some(TransactionInfo::Committed(tx))
    }

    #[cfg_attr(feature = "cargo-clippy", allow(let_and_return))]
    fn precommits(&self, block: &Block) -> Vec<Precommit> {
        let schema = Schema::new(&self.snapshot);
        let precommits_table = schema.precommits(&block.hash());
        let precommits = precommits_table.iter().collect();
        precommits
    }

    #[cfg_attr(feature = "cargo-clippy", allow(let_and_return))]
    fn transaction_hashes(&self, block: &Block) -> Vec<Hash> {
        let schema = Schema::new(&self.snapshot);
        let tx_hashes_table = schema.block_transactions(block.height());
        let tx_hashes = tx_hashes_table.iter().collect();
        tx_hashes
    }

    /// Retrieves a transaction that is known to be committed.
    fn committed_transaction(
        &self,
        tx_hash: &Hash,
        maybe_content: Option<Box<Transaction>>,
    ) -> CommittedTransaction {
        let schema = Schema::new(&self.snapshot);

        let location = schema.transactions_locations().get(tx_hash).expect(
            &format!(
                "Location not found for transaction hash {:?}",
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
                (self.transaction_parser)(raw_tx).unwrap()
            }),

            location,
            location_proof,
            status,
        }
    }

    /// Returns the height of the blockchain.
    pub fn height(&self) -> Height {
        let schema = Schema::new(&self.snapshot);
        schema.height()
    }

    /// Returns block information for the specified height or `None` if there is no such block.
    pub fn block(&self, height: Height) -> Option<BlockInfo> {
        if self.height() >= height {
            Some(BlockInfo::new(self, height))
        } else {
            None
        }
    }

    /// Returns block together with its transactions for the specified height, or `None`
    /// if there is no such block.
    pub fn block_with_txs(&self, height: Height) -> Option<BlockWithTransactions> {
        let schema = Schema::new(&self.snapshot);
        let txs_table = schema.block_transactions(height);
        let block_proof = schema.block_and_precommits(height);

        block_proof.map(|proof| {
            BlockWithTransactions {
                header: proof.block,
                precommits: proof.precommits,
                transactions: txs_table
                    .iter()
                    .map(|tx_hash| self.committed_transaction(&tx_hash, None))
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
        let blocks_iter = if let Some(upper) = upper {
            self.blocks(..upper.next())
        } else {
            self.blocks(..)
        };
        let upper = blocks_iter.back;
        let blocks: Vec<_> = blocks_iter
            .rev()
            .filter(|block| !skip_empty_blocks || !block.is_empty())
            .take(count)
            .map(|block| block.header)
            .collect();

        let height = if blocks.len() < count {
            Height(0)
        } else {
            blocks.last().map_or(Height(0), |block| block.height())
        };

        BlocksRange {
            range: height..upper,
            blocks,
        }
    }

    /// Iterates over blocks in the blockchain.
    pub fn blocks<R: Into<HeightRange>>(&self, heights: R) -> Blocks {
        use std::cmp::max;

        let heights = heights.into();
        let schema = Schema::new(&self.snapshot);
        let max_height = schema.height();

        let ptr = heights.start_height();
        Blocks {
            explorer: self,
            ptr,
            back: max(ptr, heights.end_height(max_height)),
        }
    }
}

/// Iterator over blocks in the blockchain.
pub struct Blocks<'a> {
    explorer: &'a BlockchainExplorer<'a>,
    ptr: Height,
    back: Height,
}

impl<'a> fmt::Debug for Blocks<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        formatter
            .debug_struct("Blocks")
            .field("ptr", &self.ptr)
            .field("back", &self.back)
            .finish()
    }
}

impl<'a> Iterator for Blocks<'a> {
    type Item = BlockInfo<'a>;

    fn next(&mut self) -> Option<BlockInfo<'a>> {
        if self.ptr == self.back {
            return None;
        }

        let block = BlockInfo::new(self.explorer, self.ptr);
        self.ptr = self.ptr.next();
        Some(block)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact = (self.back.0 - self.ptr.0) as usize;
        (exact, Some(exact))
    }

    fn count(self) -> usize {
        (self.back.0 - self.ptr.0) as usize
    }

    fn nth(&mut self, n: usize) -> Option<BlockInfo<'a>> {
        if self.ptr.0 + n as u64 >= self.back.0 {
            self.ptr = self.back;
            None
        } else {
            self.ptr = Height(self.ptr.0 + n as u64);
            let block = BlockInfo::new(self.explorer, self.ptr);
            self.ptr = self.ptr.next();
            Some(block)
        }
    }
}

impl<'a> DoubleEndedIterator for Blocks<'a> {
    fn next_back(&mut self) -> Option<BlockInfo<'a>> {
        if self.ptr == self.back {
            return None;
        }

        self.back = self.back.previous();
        Some(BlockInfo::new(self.explorer, self.back))
    }
}
