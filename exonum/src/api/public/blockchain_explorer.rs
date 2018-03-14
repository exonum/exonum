// Copyright 2018 The Exonum Team
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

use router::Router;
use serde_json::Value as JsonValue;
use iron::prelude::*;

use std::ops::Range;
use std::cmp;

use api::{Api, ApiError};
use blockchain::{Transaction, Block, Blockchain, TxLocation, Schema, TransactionErrorType,
                 TransactionResult};
use crypto::Hash;
use helpers::Height;
use messages::Precommit;
use storage::{ListProof, Snapshot};

const MAX_BLOCKS_PER_REQUEST: u64 = 1000;

/// Block information.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BlockInfo {
    /// Block header from blockchain.
    pub block: Block,
    /// List of precommit for this block.
    pub precommits: Vec<Precommit>,
    /// List of hashes for transactions that was executed into this block.
    pub txs: Vec<Hash>,
}

/// Transaction information.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TxInfo {
    /// `JSON` serialized transaction.
    pub content: JsonValue,
    /// Transaction location in block.
    pub location: TxLocation,
    /// Proof that transaction really exist in the database.
    pub location_proof: ListProof<Hash>,
    /// Status of the transaction execution.
    pub status: TxStatus,
}

/// Transaction execution status. Simplified version of `TransactionResult`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TxStatus {
    /// Successful transaction execution.
    Success,
    /// Panic during transaction execution.
    Panic {
        /// Panic description.
        description: String,
    },
    /// Error during transaction execution.
    Error {
        /// User-defined error code.
        code: u8,
        /// Error description.
        description: String,
    },
}

/// Information on blocks coupled with the corresponding range in the blockchain.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BlocksRange {
    /// Exclusive range of blocks.
    pub range: Range<u64>,
    /// Blocks in the range.
    pub blocks: Vec<Block>,
}

/// Information about the transaction.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TransactionInfo {
    /// Transaction with the given id is absent in the blockchain.
    Unknown,
    /// Transaction is in the memory pool, but not yet committed to the blockchain.
    InPool {
        /// Json representation of the given transaction.
        content: JsonValue,
    },
    /// Transaction is already committed to the blockchain.
    Committed(TxInfo),
}

/// Public explorer API.
#[derive(Clone, Debug)]
pub struct ExplorerApi {
    blockchain: Blockchain,
}

impl ExplorerApi {
    /// Creates a new `ExplorerApi` instance.
    pub fn new(blockchain: Blockchain) -> Self {
        ExplorerApi { blockchain }
    }

    fn explorer(&self) -> BlockchainExplorer {
        BlockchainExplorer::new(&self.blockchain)
    }

    fn blocks(
        &self,
        count: u64,
        from: Option<u64>,
        skip_empty_blocks: bool,
    ) -> Result<BlocksRange, ApiError> {
        if count > MAX_BLOCKS_PER_REQUEST {
            return Err(ApiError::BadRequest(format!(
                "Max block count per request exceeded ({})",
                MAX_BLOCKS_PER_REQUEST
            )));
        }
        Ok(self.explorer().blocks_range(count, from, skip_empty_blocks))
    }

    fn block(&self, height: Height) -> Option<BlockInfo> {
        self.explorer().block_info(height)
    }

    fn tx_from_raw(
        &self,
        schema: &Schema<Box<Snapshot>>,
        hash: &Hash,
    ) -> Result<Box<Transaction>, ApiError> {
        let raw_tx = schema.transactions().get(hash).expect(
            "Expected tx in database",
        );

        Ok(self.blockchain.tx_from_raw(raw_tx.clone()).ok_or_else(|| {
            ApiError::InternalError(format!("Service not found for tx: {:?}", raw_tx).into())
        })?)
    }

    fn transaction_info(&self, hash: &Hash) -> Result<TransactionInfo, ApiError> {
        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(snapshot);
        if schema.transactions_pool().contains(hash) {
            let content = self.tx_from_raw(&schema, hash)?.serialize_field().map_err(
                ApiError::InternalError,
            )?;
            Ok(TransactionInfo::InPool { content })
        } else if let Some(tx_info) = self.explorer().tx_info(hash)? {
            Ok(TransactionInfo::Committed(tx_info))
        } else {
            Ok(TransactionInfo::Unknown)
        }
    }

    fn set_blocks_response(self, router: &mut Router) {
        let blocks = move |req: &mut Request| -> IronResult<Response> {
            let count: u64 = self.required_param(req, "count")?;
            let latest: Option<u64> = self.optional_param(req, "latest")?;
            let skip_empty_blocks: bool = self.optional_param(req, "skip_empty_blocks")?
                .unwrap_or(false);
            let info = self.blocks(count, latest, skip_empty_blocks)?;
            self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/blocks", blocks, "blocks");
    }

    fn set_block_response(self, router: &mut Router) {
        let block = move |req: &mut Request| -> IronResult<Response> {
            let height: Height = self.url_fragment(req, "height")?;
            let info = self.block(height);
            self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/blocks/:height", block, "height");
    }

    fn set_transaction_info_response(self, router: &mut Router) {
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            let hash: Hash = self.url_fragment(req, "hash")?;
            let info = self.transaction_info(&hash)?;
            let result = match info {
                TransactionInfo::Unknown => Self::not_found_response,
                _ => Self::ok_response,
            };
            result(&self, &::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/transactions/:hash", transaction, "hash");
    }
}

impl Api for ExplorerApi {
    fn wire(&self, router: &mut Router) {
        self.clone().set_blocks_response(router);
        self.clone().set_block_response(router);
        self.clone().set_transaction_info_response(router);
    }
}

/// Blockchain explorer.
#[derive(Debug)]
pub struct BlockchainExplorer<'a> {
    blockchain: &'a Blockchain,
}

impl<'a> BlockchainExplorer<'a> {
    /// Creates a new `BlockchainExplorer` instance.
    pub fn new(blockchain: &'a Blockchain) -> Self {
        BlockchainExplorer { blockchain }
    }

    /// Returns information about the transaction identified by the hash.
    pub fn tx_info(&self, tx_hash: &Hash) -> Result<Option<TxInfo>, ApiError> {
        let schema = Schema::new(self.blockchain.snapshot());
        let raw_tx = match schema.transactions().get(tx_hash) {
            Some(val) => val,
            None => {
                return Ok(None);
            }
        };

        let box_transaction = self.blockchain.tx_from_raw(raw_tx.clone()).ok_or_else(|| {
            ApiError::InternalError(format!("Service not found for tx: {:?}", raw_tx).into())
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

        let location_proof = schema.block_txs(location.block_height()).get_proof(
            location.position_in_block(),
        );

        // Unwrap is OK here, because we already know that transaction is committed.
        let status = match schema.transaction_results().get(tx_hash).unwrap() {
            Ok(()) => TxStatus::Success,
            Err(e) => {
                let description = e.description().unwrap_or_default().to_owned();
                match e.error_type() {
                    TransactionErrorType::Panic => TxStatus::Panic { description },
                    TransactionErrorType::Code(code) => TxStatus::Error { code, description },
                }
            }
        };

        Ok(Some(TxInfo {
            content,
            location,
            location_proof,
            status,
        }))
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
    ) -> BlocksRange {
        let schema = Schema::new(self.blockchain.snapshot());
        let hashes = schema.block_hashes_by_height();
        let blocks = schema.blocks();

        // max_height >=0, as there is at least the genesis block.
        let max_height = hashes.len() - 1;

        let upper = upper.map(|x| cmp::min(x, max_height)).unwrap_or(max_height);

        let mut height = upper + 1;
        let mut genesis = false;

        let mut v = Vec::new();
        let mut collected: u64 = 0;

        // It is safe to do at least one iteration, because height >= 1.
        loop {
            if genesis || (collected == count) {
                break;
            }

            height -= 1;
            genesis = height == 0;

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

            v.push(block);
            collected += 1;
        }

        BlocksRange {
            range: height..upper + 1,
            blocks: v,
        }
    }

    /// Returns transaction result.
    pub fn transaction_result(&self, hash: &Hash) -> Option<TransactionResult> {
        let schema = Schema::new(self.blockchain.snapshot());
        schema.transaction_results().get(hash)
    }
}
