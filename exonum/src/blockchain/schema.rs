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
// WITHOUT WARRANTIES OR CONDITIONS OF ANY owner, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use anyhow::format_err;
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    impl_binary_key_for_binary_value,
    indexes::{Entries, Values},
    Entry, KeySetIndex, ListIndex, MapIndex, ObjectHash, ProofEntry, ProofListIndex, ProofMapIndex,
};
use exonum_proto::ProtobufConvert;

use std::fmt;

use super::{Block, BlockProof, CallProof, ConsensusConfig};
use crate::{
    crypto::{Hash, PublicKey},
    helpers::{Height, ValidatorId},
    messages::{AnyTx, Precommit, Verified},
    proto::schema::blockchain as pb_blockchain,
    runtime::{ExecutionError, ExecutionErrorAux, InstanceId},
};

/// Defines `&str` constants with given name and value.
macro_rules! define_names {
    (
        $(
            $name:ident => $value:expr;
        )+
    ) => (
        $(const $name: &str = concat!("core.", $value);)*
    )
}

define_names!(
    TRANSACTIONS => "transactions";
    CALL_ERRORS => "call_errors";
    CALL_ERRORS_AUX => "call_errors_aux";
    TRANSACTIONS_LEN => "transactions_len";
    TRANSACTIONS_POOL => "transactions_pool";
    TRANSACTIONS_POOL_LEN => "transactions_pool_len";
    TRANSACTIONS_LOCATIONS => "transactions_locations";
    BLOCKS => "blocks";
    BLOCK_HASHES_BY_HEIGHT => "block_hashes_by_height";
    BLOCK_TRANSACTIONS => "block_transactions";
    BLOCK_SKIP => "block_skip";
    PRECOMMITS => "precommits";
    CONSENSUS_CONFIG => "consensus_config";
);

/// Transaction location in a block. Defines the block where the transaction was
/// included and the position of this transaction in the block.
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "pb_blockchain::TxLocation")]
pub struct TxLocation {
    /// Height of the block where the transaction was included.
    block_height: Height,
    /// Zero-based position of this transaction in the block.
    position_in_block: u32,
}

impl TxLocation {
    /// Creates a new transaction location.
    pub fn new(block_height: Height, position_in_block: u32) -> Self {
        Self {
            block_height,
            position_in_block,
        }
    }

    /// Height of the block where the transaction was included.
    pub fn block_height(&self) -> Height {
        self.block_height
    }

    /// Zero-based position of this transaction in the block.
    pub fn position_in_block(&self) -> u32 {
        self.position_in_block
    }
}

/// Information schema for indexes maintained by the Exonum core logic.
///
/// Indexes defined by this schema are present in the blockchain regardless of
/// the deployed services and store general-purpose information, such as
/// committed transactions.
#[derive(Debug, Clone, Copy)]
pub struct Schema<T> {
    // For performance reasons, we don't use the field-per-index schema pattern.
    // Indeed, the core schema has many indexes, most of which are never accessed
    // for any particular `Schema` instance.
    access: T,
}

impl<T: Access> Schema<T> {
    /// Constructs information schema based on the given `access`.
    #[doc(hidden)]
    pub fn new(access: T) -> Self {
        Self { access }
    }

    /// Returns a table that represents a map with a key-value pair of a
    /// transaction hash and raw transaction message.
    pub fn transactions(&self) -> MapIndex<T::Base, Hash, Verified<AnyTx>> {
        self.access.get_map(TRANSACTIONS)
    }

    pub(crate) fn call_errors_map(
        &self,
        block_height: Height,
    ) -> ProofMapIndex<T::Base, CallInBlock, ExecutionError> {
        self.access.get_proof_map((CALL_ERRORS, &block_height.0))
    }

    /// Returns auxiliary information about an error that does not influence blockchain state hash.
    fn call_errors_aux(
        &self,
        block_height: Height,
    ) -> MapIndex<T::Base, CallInBlock, ExecutionErrorAux> {
        self.access.get_map((CALL_ERRORS_AUX, &block_height.0))
    }

    /// Returns a record of errors that occurred during execution of a particular block.
    /// If the block is not committed, returns `None`.
    pub fn call_records(&self, block_height: Height) -> Option<CallRecords<T>> {
        self.block_hash_by_height(block_height)?;
        Some(CallRecords {
            height: block_height,
            errors: self.call_errors_map(block_height),
            errors_aux: self.call_errors_aux(block_height),
            access: self.access.clone(),
        })
    }

    /// Returns the result of the execution for a transaction with the specified location.
    /// If the location does not correspond to a transaction, returns `None`.
    pub fn transaction_result(&self, location: TxLocation) -> Option<Result<(), ExecutionError>> {
        let records = self.call_records(location.block_height)?;
        let txs_in_block = self.block_transactions(location.block_height).len();
        if txs_in_block <= u64::from(location.position_in_block) {
            return None;
        }
        let status = records.get(CallInBlock::transaction(location.position_in_block));
        Some(status)
    }

    /// Returns an entry that represents a count of committed transactions in the blockchain.
    fn transactions_len_index(&self) -> Entry<T::Base, u64> {
        self.access.get_entry(TRANSACTIONS_LEN)
    }

    /// Returns the number of committed transactions in the blockchain.
    pub fn transactions_len(&self) -> u64 {
        // TODO: Change a count of tx logic after replacement storage to MerkleDB. ECR-3087
        let pool = self.transactions_len_index();
        pool.get().unwrap_or(0)
    }

    /// Returns a table that represents a set of uncommitted transactions hashes.
    ///
    /// # Stability
    ///
    /// Since a signature of this method could be changed in the future due to performance reasons,
    /// this method is considered unstable.
    pub fn transactions_pool(&self) -> KeySetIndex<T::Base, Hash> {
        self.access.get_key_set(TRANSACTIONS_POOL)
    }

    /// Returns an entry that represents count of uncommitted transactions.
    #[doc(hidden)]
    pub fn transactions_pool_len_index(&self) -> Entry<T::Base, u64> {
        self.access.get_entry(TRANSACTIONS_POOL_LEN)
    }

    /// Returns the number of transactions in the pool.
    pub fn transactions_pool_len(&self) -> u64 {
        let pool = self.transactions_pool_len_index();
        pool.get().unwrap_or(0)
    }

    /// Returns a table that keeps the block height and transaction position inside the block for every
    /// transaction hash.
    pub fn transactions_locations(&self) -> MapIndex<T::Base, Hash, TxLocation> {
        self.access.get_map(TRANSACTIONS_LOCATIONS)
    }

    /// Returns a table that stores a block object for every block height.
    pub fn blocks(&self) -> MapIndex<T::Base, Hash, Block> {
        self.access.get_map(BLOCKS)
    }

    /// Returns a table that keeps block hashes for corresponding block heights.
    pub fn block_hashes_by_height(&self) -> ListIndex<T::Base, Hash> {
        self.access.get_list(BLOCK_HASHES_BY_HEIGHT)
    }

    /// Returns a table that keeps a list of transactions for each block.
    pub fn block_transactions(&self, height: Height) -> ProofListIndex<T::Base, Hash> {
        let height: u64 = height.into();
        self.access.get_proof_list((BLOCK_TRANSACTIONS, &height))
    }

    /// Returns an entry storing the latest skip block for the node.
    fn block_skip_entry(&self) -> Entry<T::Base, Block> {
        self.access.get_entry(BLOCK_SKIP)
    }

    /// Returns the recorded [block skip], if any.
    ///
    /// [block skip]: enum.BlockContents.html#variant.Skip
    pub fn block_skip(&self) -> Option<Block> {
        self.block_skip_entry().get()
    }

    /// Returns the recorded [block skip] together with authenticating information.
    ///
    /// [block skip]: enum.BlockContents.html#variant.Skip
    pub fn block_skip_and_precommits(&self) -> Option<BlockProof> {
        let block = self.block_skip_entry().get()?;
        let precommits = self.precommits(&block.object_hash()).iter().collect();
        Some(BlockProof::new(block, precommits))
    }

    /// Returns a table that keeps a list of precommits for the block with the given hash.
    pub fn precommits(&self, hash: &Hash) -> ListIndex<T::Base, Verified<Precommit>> {
        self.access.get_list((PRECOMMITS, hash))
    }

    /// Returns an actual consensus configuration entry.
    #[doc(hidden)]
    pub fn consensus_config_entry(&self) -> ProofEntry<T::Base, ConsensusConfig> {
        self.access.get_proof_entry(CONSENSUS_CONFIG)
    }

    /// Returns the block hash for the given height.
    pub fn block_hash_by_height(&self, height: Height) -> Option<Hash> {
        self.block_hashes_by_height().get(height.into())
    }

    /// Returns the block for the given height with the proof of its inclusion.
    pub fn block_and_precommits(&self, height: Height) -> Option<BlockProof> {
        let block_hash = self.block_hash_by_height(height)?;
        let block = self.blocks().get(&block_hash).unwrap();
        let precommits = self.precommits(&block_hash).iter().collect();
        Some(BlockProof::new(block, precommits))
    }

    /// Returns the latest committed block.
    ///
    /// # Panics
    ///
    /// Panics if the genesis block was not created.
    pub fn last_block(&self) -> Block {
        let hash = self
            .block_hashes_by_height()
            .last()
            .expect("An attempt to get the `last_block` during creating the genesis block.");
        self.blocks().get(&hash).unwrap()
    }

    /// Returns the height of the latest committed block.
    ///
    /// # Panics
    ///
    /// Panics if invoked before the genesis block was created, e.g. within
    /// `after_transactions` hook for genesis block.
    pub fn height(&self) -> Height {
        let len = self.block_hashes_by_height().len();
        assert!(
            len > 0,
            "An attempt to get the actual `height` during creating the genesis block."
        );
        Height(len - 1)
    }

    /// Returns the height of the block to be committed.
    ///
    /// Unlike `height`, this method never panics.
    pub fn next_height(&self) -> Height {
        let len = self.block_hashes_by_height().len();
        Height(len)
    }

    /// Returns an actual consensus configuration of the blockchain.
    ///
    /// # Panics
    ///
    /// Panics if the genesis block was not created.
    pub fn consensus_config(&self) -> ConsensusConfig {
        self.consensus_config_entry()
            .get()
            .expect("Consensus configuration is absent")
    }

    /// Attempts to find a `ValidatorId` by the provided service public key.
    pub fn validator_id(&self, service_public_key: PublicKey) -> Option<ValidatorId> {
        self.consensus_config()
            .find_validator(|validator_keys| service_public_key == validator_keys.service_key)
    }
}

impl<T: Access> Schema<T>
where
    T::Base: RawAccessMut,
{
    /// Adds a transaction into the persistent pool. The caller must ensure that the transaction
    /// is not already in the pool.
    ///
    /// This method increments the number of transactions in the pool,
    /// be sure to decrement it when the transaction committed.
    #[doc(hidden)]
    pub fn add_transaction_into_pool(&mut self, tx: Verified<AnyTx>) {
        self.transactions_pool().insert(&tx.object_hash());
        let x = self.transactions_pool_len_index().get().unwrap_or(0);
        self.transactions_pool_len_index().set(x + 1);
        self.transactions().put(&tx.object_hash(), tx);
    }

    /// Changes the transaction status from `in_pool`, to `committed`.
    ///
    /// **NB.** This method does not remove transactions from the `transactions_pool`.
    /// The pool is updated during block commit in `update_transaction_count` in order to avoid
    /// data race between commit and adding transactions into the pool.
    pub(crate) fn commit_transaction(&mut self, hash: &Hash, height: Height, tx: Verified<AnyTx>) {
        if !self.transactions().contains(hash) {
            self.transactions().put(hash, tx)
        }

        self.block_transactions(height).push(*hash);
    }

    /// Updates transaction count of the blockchain.
    pub(crate) fn update_transaction_count(&mut self) {
        let block_transactions = self.block_transactions(self.height());
        let count = block_transactions.len();

        let mut len_index = self.transactions_len_index();
        let new_len = len_index.get().unwrap_or(0) + count;
        len_index.set(new_len);

        // Determine the number of committed transactions present in the pool (besides the pool,
        // transactions can be taken from the non-persistent cache). Remove the committed transactions
        // from the pool.
        let mut pool = self.transactions_pool();
        let pool_count = block_transactions
            .iter()
            .filter(|tx_hash| {
                if pool.contains(tx_hash) {
                    pool.remove(tx_hash);
                    true
                } else {
                    false
                }
            })
            .count();

        let mut pool_len_index = self.transactions_pool_len_index();
        let new_pool_len = pool_len_index.get().unwrap_or(0) - pool_count as u64;
        pool_len_index.set(new_pool_len);
    }

    /// Saves an error to the blockchain.
    pub(crate) fn save_error(
        &mut self,
        height: Height,
        call: CallInBlock,
        mut err: ExecutionError,
    ) {
        let aux = err.split_aux();
        self.call_errors_map(height).put(&call, err);
        self.call_errors_aux(height).put(&call, aux);
    }

    pub(super) fn clear_block_skip(&mut self) {
        if let Some(block_skip) = self.block_skip_entry().take() {
            let block_hash = block_skip.object_hash();
            self.precommits(&block_hash).clear();
        }
    }

    pub(super) fn store_block_skip(&mut self, block_skip: Block) {
        // TODO: maybe it makes sense to use a circular buffer here.
        self.clear_block_skip();
        self.block_skip_entry().set(block_skip);
    }
}

/// Information about call errors within a specific block.
///
/// This data type can be used to get information or build proofs that execution
/// of a certain call ended up with a particular status.
#[derive(Debug)]
pub struct CallRecords<T: Access> {
    height: Height,
    errors: ProofMapIndex<T::Base, CallInBlock, ExecutionError>,
    errors_aux: MapIndex<T::Base, CallInBlock, ExecutionErrorAux>,
    access: T,
}

impl<T: Access> CallRecords<T> {
    /// Iterates over errors in a block.
    pub fn errors(&self) -> CallErrorsIter<'_> {
        CallErrorsIter {
            errors_iter: self.errors.iter(),
            aux_iter: self.errors_aux.values(),
        }
    }

    /// Returns a result of a call execution.
    ///
    /// # Return value
    ///
    /// This method will return `Ok(())` both if the call completed successfully, or if
    /// was not performed at all. The caller is responsible to distinguish these two outcomes.
    pub fn get(&self, call: CallInBlock) -> Result<(), ExecutionError> {
        match self.errors.get(&call) {
            Some(mut err) => {
                let aux = self
                    .errors_aux
                    .get(&call)
                    .expect("BUG: Aux info is not saved for an error");
                err.recombine_with_aux(aux);
                Err(err)
            }
            None => Ok(()),
        }
    }

    /// Returns a cryptographic proof of authenticity for a top-level call within a block.
    pub fn get_proof(&self, call: CallInBlock) -> CallProof {
        let block_proof = Schema::new(self.access.clone())
            .block_and_precommits(self.height)
            .unwrap();
        let error_description = self.errors_aux.get(&call).map(|aux| aux.description);
        let call_proof = self.errors.get_proof(call);
        CallProof::new(block_proof, call_proof, error_description)
    }
}

/// Iterator over errors in a block returned by `CallRecords::errors()`.
#[derive(Debug)]
pub struct CallErrorsIter<'a> {
    errors_iter: Entries<'a, CallInBlock, ExecutionError>,
    aux_iter: Values<'a, ExecutionErrorAux>,
}

impl Iterator for CallErrorsIter<'_> {
    type Item = (CallInBlock, ExecutionError);

    fn next(&mut self) -> Option<Self::Item> {
        let (call, mut error) = self.errors_iter.next()?;
        let aux = self
            .aux_iter
            .next()
            .expect("BUG: Aux info is not saved for an error");
        error.recombine_with_aux(aux);
        Some((call, error))
    }
}

/// Location of an isolated call within a block.
///
/// Exonum isolates execution of the transactions included into the the block,
/// and `before_transactions` / `after_transactions` hooks that are executed for each active service.
/// If an isolated call ends with an error, all changes to the blockchain state made within a call
/// are rolled back.
///
/// `CallInBlock` objects are ordered in the same way the corresponding calls would be performed
/// within a block:
///
/// ```rust
/// # use exonum::blockchain::CallInBlock;
/// assert!(CallInBlock::before_transactions(3) < CallInBlock::transaction(0));
/// assert!(CallInBlock::transaction(0) < CallInBlock::transaction(1));
/// assert!(CallInBlock::transaction(1) < CallInBlock::after_transactions(0));
/// assert!(CallInBlock::after_transactions(0) < CallInBlock::after_transactions(1));
/// ```
///
/// # See also
///
/// Not to be confused with [`CallSite`], which provides information about a call in which
/// an error may occur. Since Exonum services may call each other's methods, `CallSite` is
/// richer than `CallInBlock`.
///
/// One example of difference between these types is [`CallType::Constructor`]: since services
/// are constructed outside of the block processing routine, this kind of errors cannot be
/// represented as `CallInBlock`.
///
/// [`CallSite`]: ../runtime/error/struct.CallSite.html
/// [`CallType::Constructor`]: ../runtime/error/enum.CallType.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] // builtin traits
#[derive(Serialize, Deserialize, BinaryValue, ObjectHash)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum CallInBlock {
    /// Call of `before_transactions` hook in a service.
    BeforeTransactions {
        /// Numerical service identifier.
        id: InstanceId,
    },
    /// Call of a transaction within the block.
    Transaction {
        /// Zero-based transaction index.
        index: u32,
    },
    /// Call of `after_transactions` hook in a service.
    AfterTransactions {
        /// Numerical service identifier.
        id: InstanceId,
    },
}

impl ProtobufConvert for CallInBlock {
    type ProtoStruct = pb_blockchain::CallInBlock;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut pb = Self::ProtoStruct::new();
        match self {
            Self::BeforeTransactions { id } => pb.set_before_transactions(*id),
            Self::Transaction { index } => pb.set_transaction(*index),
            Self::AfterTransactions { id } => pb.set_after_transactions(*id),
        }
        pb
    }

    fn from_pb(pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        if pb.has_before_transactions() {
            Ok(Self::BeforeTransactions {
                id: pb.get_before_transactions(),
            })
        } else if pb.has_transaction() {
            Ok(Self::Transaction {
                index: pb.get_transaction(),
            })
        } else if pb.has_after_transactions() {
            Ok(Self::AfterTransactions {
                id: pb.get_after_transactions(),
            })
        } else {
            Err(format_err!("Invalid location format"))
        }
    }
}

impl CallInBlock {
    /// Creates a location corresponding to a `before_transactions` call.
    pub fn before_transactions(id: InstanceId) -> Self {
        Self::BeforeTransactions { id }
    }

    /// Creates a location corresponding to a transaction.
    pub fn transaction(index: u32) -> Self {
        Self::Transaction { index }
    }

    /// Creates a location corresponding to a `after_transactions` call.
    pub fn after_transactions(id: InstanceId) -> Self {
        Self::AfterTransactions { id }
    }
}

impl_binary_key_for_binary_value!(CallInBlock);

impl fmt::Display for CallInBlock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeTransactions { id } => write!(
                formatter,
                "`before_transactions` for service with ID {}",
                id
            ),
            Self::Transaction { index } => write!(formatter, "transaction #{}", index + 1),
            Self::AfterTransactions { id } => {
                write!(formatter, "`after_transactions` for service with ID {}", id)
            }
        }
    }
}

#[test]
fn location_json_serialization() {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    let location = CallInBlock::transaction(1);
    assert_eq!(
        serde_json::to_value(location).unwrap(),
        json!({ "type": "transaction", "index": 1 })
    );

    let location = CallInBlock::after_transactions(1_000);
    assert_eq!(
        serde_json::to_value(location).unwrap(),
        json!({ "type": "after_transactions", "id": 1_000 })
    );
}
