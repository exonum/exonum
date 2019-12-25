//! Types used in the explorer API.
//!
//! The types are bundled together with the explorer (rather than the explorer service)
//! in order to ease dependency management for client apps.

use chrono::{DateTime, Utc};
use exonum::{
    blockchain::Block,
    crypto::Hash,
    helpers::Height,
    messages::{Precommit, Verified},
    runtime::{CallInfo, ExecutionStatus, InstanceId},
};
use serde_derive::*;

use std::ops::Range;

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
