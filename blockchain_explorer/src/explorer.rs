use serde::Serialize;
use serde_json::Value;

use std::cmp;

use exonum::storage::{Map, List, Result as StorageResult};
use exonum::crypto::Hash;
use exonum::blockchain::{Schema, Blockchain};

use api::HexField;

pub struct BlockchainExplorer<'a> {
    blockchain: &'a Blockchain,
}

pub trait TransactionInfo: Serialize {}

#[derive(Debug, Serialize)]
pub struct BlockInfo {
    height: u64,
    proposer: u32,

    hash: HexField<Hash>,
    state_hash: HexField<Hash>,
    tx_hash: HexField<Hash>,
    tx_count: u64,
    precommits_count: u64,
    txs: Option<Vec<Value>>,
}

impl<'a> BlockchainExplorer<'a> {
    pub fn new(blockchain: &'a Blockchain) -> BlockchainExplorer {
        BlockchainExplorer { blockchain: blockchain }
    }

    pub fn tx_info(&self, tx_hash: &Hash) -> StorageResult<Option<Value>> {
        let tx = Schema::new(&self.blockchain.view())
            .transactions()
            .get(tx_hash)?;
        match tx {
            Some(raw_tx) => {
                Ok(self.blockchain
                       .tx_from_raw(raw_tx)
                       .and_then(|t| Some(t.info())))
            }
            None => Ok(None),
        }

    }

    pub fn block_info(&self,
                      block_hash: &Hash,
                      full_info: bool)
                      -> StorageResult<Option<BlockInfo>> {
        let b = self.blockchain.clone();
        let view = b.view();
        let schema = Schema::new(&view);
        let block = schema.blocks().get(block_hash)?;
        if let Some(block) = block {
            let height = block.height();
            let (txs, txs_count) = {
                if full_info {
                    let txs = self.block_txs(block.height())?;
                    let txs_count = txs.len() as u64;
                    (Some(txs), txs_count)
                } else {
                    (None, schema.block_txs(height).len()? as u64)
                }
            };

            let config = schema.get_actual_configuration()?;
            // TODO Find more common solution
            // FIXME this code was copied from state.rs
            let proposer = ((height + block.propose_round() as u64) %
                            (config.validators.len() as u64)) as u32;

            let precommits_count = schema.precommits(block_hash).len()? as u64;
            let info = BlockInfo {
                height: height,
                proposer: proposer,

                hash: HexField(*block_hash),
                state_hash: HexField(*block.state_hash()),
                tx_hash: HexField(*block.tx_hash()),
                tx_count: txs_count,
                precommits_count: precommits_count,
                txs: txs,
            };
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    fn block_txs(&self, height: u64) -> StorageResult<Vec<Value>> {
        let b = self.blockchain.clone();
        let view = b.view();
        let schema = Schema::new(&view);
        let txs = schema.block_txs(height);
        let tx_count = txs.len()?;

        let mut v = Vec::new();
        for i in 0..tx_count {
            if let Some(tx_hash) = txs.get(i)? {
                if let Some(tx_info) = self.tx_info(&tx_hash)? {
                    v.push(tx_info);
                }
            }
        }
        Ok(v)
    }

    pub fn block_info_with_height(&self, height: u64) -> StorageResult<Option<BlockInfo>> {
        if let Some(block_hash) = Schema::new(&self.blockchain.view())
               .heights()
               .get(height)? {
            self.block_info(&block_hash, true)
        } else {
            Ok(None)
        }
    }

    pub fn blocks_range(&self, count: u64, from: Option<u64>) -> StorageResult<Vec<BlockInfo>> {
        let b = self.blockchain.clone();
        let view = b.view();
        let schema = Schema::new(&view);
        let heights = schema.heights();

        let max_len = heights.len()?;
        let to = from.map(|x| cmp::min(x, max_len)).unwrap_or(max_len);
        let from = to.checked_sub(count).unwrap_or(0);

        let mut v = Vec::new();
        for height in (from..to).rev() {
            if let Some(ref h) = heights.get(height)? {
                if let Some(block_info) = self.block_info(h, false)? {
                    v.push(block_info);
                }
            }
        }
        Ok(v)
    }
}
