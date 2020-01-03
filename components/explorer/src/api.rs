//! Types used in the explorer API.
//!
//! The types are bundled together with the explorer (rather than the explorer service)
//! in order to ease dependency management for client apps.

use chrono::{DateTime, Utc};
use exonum::{
    blockchain::{Block, Schema, TxLocation},
    crypto::Hash,
    helpers::Height,
    merkledb::{access::Access, ListProof},
    messages::{Precommit, Verified},
    runtime::{CallInfo, ExecutionStatus, InstanceId},
};
use serde_derive::*;

use std::ops::Range;

use crate::median_precommits_time;

/// The maximum number of blocks to return per blocks request, in this way
/// the parameter limits the maximum execution time for such requests.
pub const MAX_BLOCKS_PER_REQUEST: usize = 1000;

/// Information on blocks coupled with the corresponding range in the blockchain.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BlocksRange {
    /// Exclusive range of blocks.
    pub range: Range<Height>,
    /// Blocks in the range.
    pub blocks: Vec<BlockInfo>,
}

/// Information about a transaction included in the block.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TxInfo {
    /// Transaction hash.
    pub tx_hash: Hash,
    /// Information to call.
    pub call_info: CallInfo,
}

/// Information about a block in the blockchain.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockInfo {
    /// Block header as recorded in the blockchain.
    #[serde(flatten)]
    pub block: Block,

    /// Precommits authorizing the block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precommits: Option<Vec<Verified<Precommit>>>,

    /// Info of transactions in the block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txs: Option<Vec<TxInfo>>,

    /// Median time from the block precommits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<DateTime<Utc>>,
}

impl<'a> From<crate::BlockInfo<'a>> for BlockInfo {
    fn from(inner: crate::BlockInfo<'a>) -> Self {
        Self {
            block: inner.header().clone(),
            precommits: Some(inner.precommits().to_vec()),
            txs: Some(
                inner
                    .transaction_hashes()
                    .iter()
                    .enumerate()
                    .map(|(idx, &tx_hash)| TxInfo {
                        tx_hash,
                        call_info: inner
                            .transaction(idx)
                            .unwrap()
                            .content()
                            .payload()
                            .call_info
                            .clone(),
                    })
                    .collect(),
            ),
            time: Some(median_precommits_time(&inner.precommits())),
        }
    }
}

/// Blocks in range parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct BlocksQuery {
    /// The number of blocks to return. Should not be greater than `MAX_BLOCKS_PER_REQUEST`.
    pub count: usize,
    /// The maximum height of the returned blocks.
    ///
    /// The blocks are returned in reverse order,
    /// starting from the latest and at least up to the `latest - count + 1`.
    /// The default value is the height of the latest block in the blockchain.
    pub latest: Option<Height>,
    /// The minimum height of the returned blocks. The default value is `Height(0)` (the genesis
    /// block).
    ///
    /// Note that `earliest` has the least priority compared to `latest` and `count`;
    /// it can only truncate the list of otherwise returned blocks if some of them have a lesser
    /// height.
    pub earliest: Option<Height>,
    /// If true, then only non-empty blocks are returned. The default value is false.
    #[serde(default)]
    pub skip_empty_blocks: bool,
    /// If true, then the returned `BlocksRange`'s `times` field will contain median time from the
    /// corresponding blocks precommits.
    #[serde(default)]
    pub add_blocks_time: bool,
    /// If true, then the returned `BlocksRange.precommits` will contain precommits for the
    /// corresponding returned blocks.
    #[serde(default)]
    pub add_precommits: bool,
}

/// Block query parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BlockQuery {
    /// The height of the desired block.
    pub height: Height,
}

impl BlockQuery {
    /// Creates a new block query with the given height.
    pub fn new(height: Height) -> Self {
        Self { height }
    }
}

/// Raw Transaction in hex representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionHex {
    /// The hex value of the transaction to be broadcasted.
    pub tx_body: String,
}

/// Transaction response.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct TransactionResponse {
    /// The hex value of the transaction to be broadcasted.
    pub tx_hash: Hash,
}

/// Transaction query parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct TransactionQuery {
    /// The hash of the transaction to be searched.
    pub hash: Hash,
}

impl TransactionQuery {
    /// Creates a new transaction query with the given height.
    pub fn new(hash: Hash) -> Self {
        Self { hash }
    }
}

impl AsRef<str> for TransactionHex {
    fn as_ref(&self) -> &str {
        self.tx_body.as_ref()
    }
}

impl AsRef<[u8]> for TransactionHex {
    fn as_ref(&self) -> &[u8] {
        self.tx_body.as_ref()
    }
}

/// Call status response.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CallStatusResponse {
    /// Call status
    pub status: ExecutionStatus,
}

/// Call status query parameters to check `before_transactions` or `after_transactions` call.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CallStatusQuery {
    /// Height of a block.
    pub height: Height,
    /// Numerical service identifier.
    pub service_id: InstanceId,
}

/// Summary about a particular transaction in the blockchain (without transaction content).
#[derive(Debug, Serialize, Deserialize)]
pub struct CommittedTransactionSummary {
    /// Transaction identifier.
    pub tx_hash: Hash,
    /// ID of service.
    pub service_id: u16,
    /// ID of transaction in service.
    pub message_id: u16,
    /// Result of transaction execution.
    pub status: ExecutionStatus,
    /// Transaction location in the blockchain.
    pub location: TxLocation,
    /// Proof of existence.
    pub location_proof: ListProof<Hash>,
    /// Approximate finalization time.
    pub time: DateTime<Utc>,
}

impl CommittedTransactionSummary {
    /// Constructs a transaction summary from the core schema.
    pub fn new(schema: &Schema<impl Access>, tx_hash: &Hash) -> Option<Self> {
        let tx = schema.transactions().get(tx_hash)?;
        let tx = tx.as_ref();
        let service_id = tx.call_info.instance_id as u16;
        let tx_id = tx.call_info.method_id as u16;
        let location = schema.transactions_locations().get(tx_hash)?;
        let tx_result = schema.transaction_result(location)?;
        let location_proof = schema
            .block_transactions(location.block_height())
            .get_proof(location.position_in_block());
        let time = median_precommits_time(
            &schema
                .block_and_precommits(location.block_height())
                .unwrap()
                .precommits,
        );
        Some(Self {
            tx_hash: *tx_hash,
            service_id,
            message_id: tx_id,
            status: ExecutionStatus(tx_result),
            location,
            location_proof,
            time,
        })
    }
}

/// Websocket notification message. This enum describes data which is sent
/// to a WebSocket listener.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Notification {
    /// Notification about new block.
    Block(Block),
    /// Notification about new transaction.
    Transaction(CommittedTransactionSummary),
}
