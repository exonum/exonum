// Copyright 2020 The Exonum Team
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

//! Blockchain explorer allows to get information about blocks and transactions in the blockchain.
//! It allows to request transactions from a block together with the execution statuses,
//! iterate over blocks, etc.
//!
//! This crate is distinct from the [explorer *service*][explorer-service] crate. While this crate
//! provides Rust language APIs for retrieving info from the blockchain, the explorer service
//! translates these APIs into REST and WebSocket endpoints. Correspondingly, this crate is
//! primarily useful for Rust-language client apps. Another use case is testing; the [testkit]
//! returns [`BlockWithTransactions`] from its `create_block*` methods and re-exports the entire
//! crate as `explorer`.
//!
//! See the examples in the crate for examples of usage.
//!
//! [explorer-service]: https://docs.rs/exonum-explorer-service/
//! [`BlockWithTransactions`]: struct.BlockWithTransactions.html
//! [testkit]: https://docs.rs/exonum-testkit/latest/exonum_testkit/struct.TestKit.html

use chrono::{DateTime, Utc};
use exonum::{
    blockchain::{Block, CallInBlock, Schema, TxLocation},
    crypto::Hash,
    helpers::Height,
    merkledb::{ListProof, MapProof, ObjectHash, Snapshot},
    messages::{AnyTx, Precommit, Verified},
    runtime::{ExecutionError, ExecutionErrorSerde, ExecutionStatus},
};
use serde::{Serialize, Serializer};
use serde_derive::*;

use std::{
    cell::{Ref, RefCell},
    collections::{BTreeMap, Bound},
    fmt,
    ops::{Index, RangeBounds},
    slice,
    time::UNIX_EPOCH,
};

pub mod api;

/// Ending height of the range (exclusive), given the a priori max height.
fn end_height(bound: Bound<&Height>, max: Height) -> Height {
    use std::cmp::min;

    let inner_end = match bound {
        Bound::Included(height) => height.next(),
        Bound::Excluded(height) => *height,
        Bound::Unbounded => max.next(),
    };

    min(inner_end, max.next())
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
/// [`Block`]: https://docs.rs/exonum/latest/exonum/blockchain/struct.Block.html
/// [`Precommit`]: https://docs.rs/exonum/latest/exonum/messages/struct.Precommit.html
/// [`Hash`]: https://docs.rs/exonum-crypto/latest/exonum_crypto/struct.Hash.html
#[derive(Debug)]
pub struct BlockInfo<'a> {
    header: Block,
    explorer: &'a BlockchainExplorer<'a>,
    precommits: RefCell<Option<Vec<Verified<Precommit>>>>,
    txs: RefCell<Option<Vec<Hash>>>,
}

impl<'a> BlockInfo<'a> {
    fn new(explorer: &'a BlockchainExplorer<'_>, height: Height) -> Self {
        let schema = explorer.schema;
        let hashes = schema.block_hashes_by_height();
        let blocks = schema.blocks();

        let block_hash = hashes
            .get(height.0)
            .unwrap_or_else(|| panic!("Block not found, height: {:?}", height));
        let header = blocks
            .get(&block_hash)
            .unwrap_or_else(|| panic!("Block not found, hash: {:?}", block_hash));

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
        self.header.height
    }

    /// Returns the number of transactions in this block.
    pub fn len(&self) -> usize {
        self.header.tx_count as usize
    }

    /// Is this block empty (i.e., contains no transactions)?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a list of precommits for this block.
    pub fn precommits(&self) -> Ref<'_, [Verified<Precommit>]> {
        if self.precommits.borrow().is_none() {
            let precommits = self.explorer.precommits(&self.header);
            *self.precommits.borrow_mut() = Some(precommits);
        }

        Ref::map(self.precommits.borrow(), |cache| {
            cache.as_ref().unwrap().as_ref()
        })
    }

    /// Lists hashes of transactions included in this block.
    pub fn transaction_hashes(&self) -> Ref<'_, [Hash]> {
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

    /// Returns the proof for the execution status of a call within this block.
    ///
    /// Note that if the call did not result in an error or did not happen at all, the returned
    /// proof will not contain entries. To distinguish between two cases, one can inspect
    /// the number of transactions in the block or IDs of the active services when the block
    /// was executed.
    pub fn error_proof(&self, call_location: CallInBlock) -> MapProof<CallInBlock, ExecutionError> {
        self.explorer
            .schema
            .call_errors(self.header.height)
            .get_proof(call_location)
    }

    /// Iterates over transactions in the block.
    pub fn iter(&self) -> Transactions<'_, '_> {
        Transactions {
            block: self,
            ptr: 0,
            len: self.len(),
        }
    }

    /// Loads transactions, errors and precommits for the block.
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
        let errors: Vec<_> = self
            .explorer
            .schema
            .call_errors(header.height)
            .iter()
            .map(|(location, error)| ErrorWithLocation { location, error })
            .collect();

        BlockWithTransactions {
            header,
            precommits,
            transactions,
            errors,
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
pub struct Transactions<'r, 'a> {
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
#[derive(Debug, Serialize, Deserialize)]
pub struct BlockWithTransactions {
    /// Block header as recorded in the blockchain.
    #[serde(rename = "block")]
    pub header: Block,
    /// Precommits.
    pub precommits: Vec<Verified<Precommit>>,
    /// Transactions in the order they appear in the block.
    pub transactions: Vec<CommittedTransaction>,
    /// Errors that have occurred within the block.
    pub errors: Vec<ErrorWithLocation>,
}

/// Execution error together with its location within the block.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorWithLocation {
    /// Location of the error.
    pub location: CallInBlock,
    /// Error data.
    #[serde(with = "ExecutionErrorSerde")]
    pub error: ExecutionError,
}

impl fmt::Display for ErrorWithLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "In {}: {}", self.location, self.error)
    }
}

impl BlockWithTransactions {
    /// Returns the height of this block.
    ///
    /// This method is equivalent to calling `block.header.height()`.
    pub fn height(&self) -> Height {
        self.header.height
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
    pub fn iter(&self) -> EagerTransactions<'_> {
        self.transactions.iter()
    }

    /// Returns errors converted into a map. Note that this is potentially a costly operation.
    pub fn error_map(&self) -> BTreeMap<CallInBlock, &ExecutionError> {
        self.errors.iter().map(|e| (e.location, &e.error)).collect()
    }
}

/// Iterator over transactions in [`BlockWithTransactions`].
///
/// [`BlockWithTransactions`]: struct.BlockWithTransactions.html
pub type EagerTransactions<'a> = slice::Iter<'a, CommittedTransaction>;

impl Index<usize> for BlockWithTransactions {
    type Output = CommittedTransaction;

    fn index(&self, index: usize) -> &CommittedTransaction {
        self.transactions.get(index).unwrap_or_else(|| {
            panic!(
                "Index exceeds number of transactions in block {}",
                self.len()
            );
        })
    }
}

/// Returns a transaction in the block by its hash. Beware that this is a slow operation
/// (linear w.r.t. the number of transactions in a block).
impl Index<Hash> for BlockWithTransactions {
    type Output = CommittedTransaction;

    fn index(&self, index: Hash) -> &CommittedTransaction {
        self.transactions
            .iter()
            .find(|&tx| tx.message.object_hash() == index)
            .unwrap_or_else(|| {
                panic!("No transaction with hash {} in the block", index);
            })
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
/// | `message` | `Verified<AnyTx>` | Transaction as recorded in the blockchain |
/// | `location` | [`TxLocation`] | Location of the transaction in the block |
/// | `location_proof` | [`ListProof`]`<`[`Hash`]`>` | Proof of transaction inclusion into a block |
/// | `status` | (custom; see below) | Execution status |
/// | `time` | [`DateTime`]`<`[`Utc`]`>` | Commitment time* |
///
/// \* By commitment time we mean an approximate commitment time of the block
/// which includes the transaction. This time is a median time of the precommit local times
/// of each validator.
///
/// ## `status` field
///
/// The `status` field is a more readable representation of the [`ExecutionStatus`] type.
///
/// For successfully executed transactions, `status` is equal to
///
/// ```json
/// { "type": "success" }
/// ```
///
/// For transactions that cause an [`ExecutionError`], `status` contains the error code
/// and an optional description, i.e., has the following type in the [TypeScript] notation:
///
/// ```typescript
/// type Error = {
///   type: 'service_error' | 'core_error' | 'common_error' | 'runtime_error' | 'unexpected_error',
///   code?: number,
///   description?: string,
///   runtime_id: number,
///   call_site?: CallSite,
/// };
///
/// type CallSite = MethodCallSite | HookCallSite;
///
/// type MethodCallSite = {
///   call_type: 'method',
///   instance_id: number,
///   interface?: string,
///   method_id: number,
/// };
///
/// type HookCallSite = {
///   call_type: 'constructor' | 'before_transactions' | 'after_transactions',
///   instance_id: number,
/// };
/// ```
///
/// Explanations:
///
/// - `Error.type` determines the component responsible for the error. Usually, errors
///   are generated by the service code, but they can also be caused by the dispatch logic,
///   runtime associated with the service, or come from another source (`unexpected_error`s).
/// - `Error.code` is the error code. For service errors, this code is specific
///   to the service instance (which can be obtained from `call_site`), and for runtime errors -
///   to the runtime. For core errors, the codes are fixed; their meaning can be found
///   in the [`CoreError`] docs. The code is present for all error types except
///   `unexpected_error`s, in which the code is always absent.
///   Besides types listed above, there is also a set of errors that can occur within any context,
///   which are organized in the [`CommonError`].
/// - `Error.description` is an optional human-readable description of the error.
/// - `Error.runtime_id` is the numeric ID of the runtime in which the error has occurred. Note
///   that the runtime is defined for all error types, not just `runtime_error`s, since
///   for any request it's possible to say which runtime is responsible for its processing.
/// - `Error.call_site` provides most precise known location of the call in which the error
///   has occurred.
///
/// [`TxLocation`]: https://docs.rs/exonum/latest/exonum/blockchain/struct.TxLocation.html
/// [`ListProof`]: https://docs.rs/exonum-merkledb/latest/exonum_merkledb/indexes/proof_list/struct.ListProof.html
/// [`Hash`]: https://docs.rs/exonum-crypto/latest/exonum_crypto/struct.Hash.html
/// [`ExecutionStatus`]: https://docs.rs/exonum/latest/exonum/runtime/struct.ExecutionStatus.html
/// [`ExecutionError`]: https://docs.rs/exonum/latest/exonum/runtime/struct.ExecutionError.html
/// [`CoreError`]: https://docs.rs/exonum/latest/exonum/runtime/enum.CoreError.html
/// [`CommonError`]: https://docs.rs/exonum/latest/exonum/runtime/enum.CommonError.html
/// [TypeScript]: https://www.typescriptlang.org/
/// [`DateTime`]: https://docs.rs/chrono/0.4.10/chrono/struct.DateTime.html
/// [`Utc`]: https://docs.rs/chrono/0.4.10/chrono/offset/struct.Utc.html
#[derive(Debug, Serialize, Deserialize)]
pub struct CommittedTransaction {
    message: Verified<AnyTx>,
    location: TxLocation,
    location_proof: ListProof<Hash>,
    status: ExecutionStatus,
    time: DateTime<Utc>,
}

impl CommittedTransaction {
    /// Returns the content of the transaction.
    pub fn message(&self) -> &Verified<AnyTx> {
        &self.message
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
    pub fn status(&self) -> Result<(), &ExecutionError> {
        self.status.0.as_ref().map(drop)
    }

    /// Returns an approximate commit time of the block which includes this transaction.
    pub fn time(&self) -> &DateTime<Utc> {
        &self.time
    }
}

/// Information about the transaction.
///
/// Values of this type are returned by the `transaction()` method of the `BlockchainExplorer`.
///
/// # JSON presentation
///
/// ## Committed transactions
///
/// Committed transactions are represented just like a `CommittedTransaction`,
/// with the additional `type` field equal to `"committed"`.
///
/// ## Transaction in pool
///
/// Transactions in pool are represented with a 2-field object:
///
/// - `type` field contains transaction type (`"in-pool"`).
/// - `message` is the full transaction message serialized to the hexadecimal form.
///
/// # Examples
///
/// ```
/// use exonum_explorer::TransactionInfo;
/// use exonum::{crypto::KeyPair, runtime::InstanceId};
/// # use exonum_derive::*;
/// # use serde_derive::*;
/// # use serde_json::json;
///
/// /// Service interface.
/// #[exonum_interface]
/// trait ServiceInterface<Ctx> {
///     type Output;
///     #[interface_method(id = 0)]
///     fn create_wallet(&self, ctx: Ctx, username: String) -> Self::Output;
/// }
///
/// # fn main() {
/// // Create a signed transaction.
/// let keypair = KeyPair::random();
/// const SERVICE_ID: InstanceId = 100;
/// let tx = keypair.create_wallet(SERVICE_ID, "Alice".to_owned());
/// // This transaction in pool will be represented as follows:
/// let json = json!({
///     "type": "in_pool",
///     "message": tx,
/// });
/// let parsed: TransactionInfo = serde_json::from_value(json).unwrap();
/// assert!(parsed.is_in_pool());
/// # }
/// ```
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransactionInfo {
    /// Transaction is in the memory pool, but not yet committed to the blockchain.
    InPool {
        /// A content of the uncommitted transaction.
        message: Verified<AnyTx>,
    },

    /// Transaction is already committed to the blockchain.
    Committed(CommittedTransaction),
}

impl TransactionInfo {
    /// Returns the content of this transaction.
    pub fn message(&self) -> &Verified<AnyTx> {
        match *self {
            TransactionInfo::InPool { ref message } => message,
            TransactionInfo::Committed(ref tx) => tx.message(),
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
/// [`Snapshot`]: https://docs.rs/exonum-merkledb/latest/exonum_merkledb/trait.Snapshot.html
#[derive(Debug, Copy, Clone)]
pub struct BlockchainExplorer<'a> {
    schema: Schema<&'a dyn Snapshot>,
}

impl<'a> BlockchainExplorer<'a> {
    /// Creates a new `BlockchainExplorer` instance from the provided snapshot.
    pub fn new(snapshot: &'a dyn Snapshot) -> Self {
        BlockchainExplorer {
            schema: Schema::new(snapshot),
        }
    }

    /// Creates a new `BlockchainExplorer` instance from the core schema.
    pub fn from_schema(schema: Schema<&'a dyn Snapshot>) -> Self {
        BlockchainExplorer { schema }
    }

    /// Returns information about the transaction identified by the hash.
    pub fn transaction(&self, tx_hash: &Hash) -> Option<TransactionInfo> {
        let message = self.transaction_without_proof(tx_hash)?;
        if self.schema.transactions_pool().contains(tx_hash) {
            return Some(TransactionInfo::InPool { message });
        }

        let tx = self.committed_transaction(tx_hash, Some(message));
        Some(TransactionInfo::Committed(tx))
    }

    /// Returns the status of a call in a block.
    ///
    /// # Return value
    ///
    /// This method will return `Ok(())` both if the call completed successfully, or if
    /// was not performed at all. The caller is responsible to distinguish these two outcomes.
    pub fn call_status(
        &self,
        block_height: Height,
        call_location: CallInBlock,
    ) -> Result<(), ExecutionError> {
        match self.schema.call_errors(block_height).get(&call_location) {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    /// Return transaction message without proof.
    pub fn transaction_without_proof(&self, tx_hash: &Hash) -> Option<Verified<AnyTx>> {
        self.schema.transactions().get(tx_hash)
    }

    fn precommits(&self, block: &Block) -> Vec<Verified<Precommit>> {
        self.schema
            .precommits(&block.object_hash())
            .iter()
            .collect()
    }

    fn transaction_hashes(&self, block: &Block) -> Vec<Hash> {
        let tx_hashes_table = self.schema.block_transactions(block.height);
        tx_hashes_table.iter().collect()
    }

    /// Retrieves a transaction that is known to be committed.
    fn committed_transaction(
        &self,
        tx_hash: &Hash,
        maybe_content: Option<Verified<AnyTx>>,
    ) -> CommittedTransaction {
        let location = self
            .schema
            .transactions_locations()
            .get(tx_hash)
            .unwrap_or_else(|| panic!("Location not found for transaction hash {:?}", tx_hash));

        let location_proof = self
            .schema
            .block_transactions(location.block_height())
            .get_proof(u64::from(location.position_in_block()));

        let block_precommits = self
            .schema
            .block_and_precommits(location.block_height())
            .unwrap();
        let time = median_precommits_time(&block_precommits.precommits);

        // Unwrap is OK here, because we already know that transaction is committed.
        let status = self.schema.transaction_result(location).unwrap();

        CommittedTransaction {
            message: maybe_content.unwrap_or_else(|| {
                self.schema
                    .transactions()
                    .get(tx_hash)
                    .expect("BUG: Cannot find transaction in database")
            }),
            location,
            location_proof,
            status: ExecutionStatus(status),
            time,
        }
    }

    /// Return the height of the blockchain.
    pub fn height(&self) -> Height {
        self.schema.height()
    }

    /// Returns block information for the specified height or `None` if there is no such block.
    pub fn block(&self, height: Height) -> Option<BlockInfo<'_>> {
        if self.height() >= height {
            Some(BlockInfo::new(self, height))
        } else {
            None
        }
    }

    /// Return a block together with its transactions at the specified height, or `None`
    /// if there is no such block.
    pub fn block_with_txs(&self, height: Height) -> Option<BlockWithTransactions> {
        let txs_table = self.schema.block_transactions(height);
        let block_proof = self.schema.block_and_precommits(height);
        let errors = self.schema.call_errors(height);

        block_proof.map(|proof| BlockWithTransactions {
            header: proof.block,
            precommits: proof.precommits,
            transactions: txs_table
                .iter()
                .map(|tx_hash| self.committed_transaction(&tx_hash, None))
                .collect(),
            errors: errors
                .iter()
                .map(|(location, error)| ErrorWithLocation { location, error })
                .collect(),
        })
    }

    /// Iterates over blocks in the blockchain.
    pub fn blocks<R: RangeBounds<Height>>(&self, heights: R) -> Blocks<'_> {
        use std::cmp::max;

        let max_height = self.schema.height();
        let ptr = match heights.start_bound() {
            Bound::Included(height) => *height,
            Bound::Excluded(height) => height.next(),
            Bound::Unbounded => Height(0),
        };
        Blocks {
            explorer: self,
            ptr,
            back: max(ptr, end_height(heights.end_bound(), max_height)),
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
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
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

/// Calculates a median time from precommits.
pub fn median_precommits_time(precommits: &[Verified<Precommit>]) -> DateTime<Utc> {
    if precommits.is_empty() {
        UNIX_EPOCH.into()
    } else {
        let mut times: Vec<_> = precommits.iter().map(|p| p.payload().time()).collect();
        times.sort();
        times[times.len() / 2]
    }
}
