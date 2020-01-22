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

//! Types used in the explorer API.
//!
//! The types are bundled together with the explorer (rather than the explorer service)
//! in order to ease dependency management for client apps.

use chrono::{DateTime, Utc};
use exonum::{
    blockchain::Block,
    crypto::Hash,
    helpers::Height,
    merkledb::BinaryValue,
    messages::{Precommit, Verified},
    runtime::{AnyTx, CallInfo, ExecutionStatus, InstanceId},
};
use serde_derive::{Deserialize, Serialize};

use std::ops::Range;

use crate::median_precommits_time;

pub mod websocket;

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
                            .message()
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
    /// If true, then the `time` field in each returned block will contain the median time from the
    /// block precommits.
    #[serde(default)]
    pub add_blocks_time: bool,
    /// If true, then the `precommits` field in each returned block will contain precommits for the
    /// block stored by the node.
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

impl TransactionHex {
    pub fn new(transaction: &Verified<AnyTx>) -> Self {
        Self {
            tx_body: hex::encode(transaction.to_bytes()),
        }
    }
}

/// Response to a request to broadcast a transaction over the blockchain network.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct TransactionResponse {
    /// The hash digest of the transaction.
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

/// Query parameters to check the execution status of a `before_transactions` or
/// `after_transactions` call.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CallStatusQuery {
    /// Height of a block.
    pub height: Height,
    /// Numerical service identifier.
    pub service_id: InstanceId,
}

/// Call status response.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CallStatusResponse {
    /// Execution status of a call.
    pub status: ExecutionStatus,
}
