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
//! See the `explorer` example in the crate for examples of usage.

use serde::{Serialize, Serializer};

use std::cell::{Ref, RefCell};
use std::collections::Bound;
use std::fmt;
use std::ops::{Index, Range, RangeFrom, RangeFull, RangeTo};

use crypto::{CryptoHash, Hash};
use blockchain::{Block, Blockchain, Schema, Transaction, TransactionError, TransactionErrorType,
                 TransactionResult, TxLocation};
use encoding;
use helpers::Height;
use messages::{Precommit, RawMessage};
use storage::{ListProof, Snapshot};

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
/// # JSON presentation
///
/// JSON object with the following fields:
///
/// | Name | Equivalent type | Description |
/// |------|-------|--------|
/// | `block` | [`Block`] | Block header as recorded in the blockchain |
/// | `precommits` | `Vec<`[`Precommit`]`>` | Precommits authorizing the block |
/// | `txs` | `Vec<`[`Hash`]`>` | Hashes of transactions in the block |
///
/// [`Block`]: ../blockchain/struct.Block.html
/// [`Precommit`]: ../messages/struct.Precommit.html
/// [`Hash`]: ../crypto/struct.Hash.html
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

            let block_hash = hashes
                .get(height.0)
                .expect(&format!("Block not found, height: {:?}", height));
            blocks
                .get(&block_hash)
                .expect(&format!("Block not found, hash: {:?}", block_hash))
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
        self.transaction_hashes()
            .get(index)
            .map(|hash| self.explorer.committed_transaction(hash, None))
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

        let precommits = precommits
            .into_inner()
            .unwrap_or_else(|| explorer.precommits(&header));
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
/// # JSON presentation
///
/// | Name | Equivalent type | Description |
/// |------|-------|--------|
/// | `content` | `Box<`[`Transaction`]`>` | Transaction as recorded in the blockchain |
/// | `location` | [`TxLocation`] | Location of the transaction in the block |
/// | `location_proof` | [`ListProof`]`<`[`Hash`]`>` | Proof of transaction inclusion into a block |
/// | `status` | (custom; see below) | Execution status |
///
/// ## `status` field
///
/// The `status` field is a more readable version of the [`TransactionResult`] type.
///
/// For successfully executed transactions, `status` is equal to
///
/// ```json
/// { "type": "success" }
/// ```
///
/// For transactions that return an [`ExecutionError`], `status` contains the error code
/// and an optional description, i.e., has the following type in the
/// [`Flow`] / [`TypeScript`] notation:
///
/// ```javascript
/// { type: 'error', code: number, description?: string }
/// ```
///
/// For transactions that have resulted in a panic, `status` contains an optional description
/// as well:
///
/// ```javascript
/// { type: 'panic', description?: string }
/// ```
///
/// [`Transaction`]: ../blockchain/trait.Transaction.html
/// [`TxLocation`]: ../blockchain/struct.TxLocation.html
/// [`ListProof`]: ../storage/enum.ListProof.html
/// [`Hash`]: ../crypto/struct.Hash.html
/// [`TransactionResult`]: ../blockchain/type.TransactionResult.html
/// [`ExecutionError`]: ../blockchain/struct.ExecutionError.html
/// [`Flow`]: https://flow.org/
/// [`TypeScript`]: https://www.typescriptlang.org/
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

        let value = tx.as_ref()
            .serialize_field()
            .map_err(|err| S::Error::custom(err.description()))?;
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
/// # JSON presentation
///
/// ## Committed transactions
///
/// Committed transactions are represented just like a [`CommittedTransaction`],
/// with the additional `type` field equal to `"committed"`.
///
/// [`CommittedTransaction`]: struct.CommittedTransaction.html#json-presentation
///
/// ## Transaction in pool
///
/// Transactions in pool are represented with a 2-field object:
///
/// - `type` field contains transaction type (`"in-pool"`).
/// - `content` is JSON serialization of the transaction.
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

/// Blockchain explorer.
///
/// # Notes
///
/// The explorer wraps a specific [`Snapshot`] of the blockchain state; that is,
/// all calls to the methods of an explorer instance are guaranteed to be consistent.
///
/// [`Snapshot`]: ../storage/trait.Snapshot.html
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

        let location = schema
            .transactions_locations()
            .get(tx_hash)
            .expect(&format!(
                "Location not found for transaction hash {:?}",
                tx_hash
            ));

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

        block_proof.map(|proof| BlockWithTransactions {
            header: proof.block,
            precommits: proof.precommits,
            transactions: txs_table
                .iter()
                .map(|tx_hash| self.committed_transaction(&tx_hash, None))
                .collect(),
        })
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
