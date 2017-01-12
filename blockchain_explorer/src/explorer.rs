use std::cmp;

use serde::Serialize;

use exonum::storage::{Map, List, View};
use exonum::storage::Result as StorageResult;
use exonum::crypto::{Hash, PublicKey};
use exonum::blockchain::{Schema, GenesisConfig};
use exonum::messages::RawTransaction;

use super::HexField;

pub struct BlockchainExplorer<'a> {
    view: &'a View,
    validators: Vec<PublicKey>,
}

pub trait TransactionInfo: Serialize {}

#[derive(Debug, Serialize)]
pub struct BlockInfo<T>
    where T: TransactionInfo
{
    height: u64,
    proposer: u32,
    propose_time: i64,

    hash: HexField<Hash>,
    state_hash: HexField<Hash>,
    tx_hash: HexField<Hash>,
    tx_count: u64,
    precommits_count: u64,
    txs: Option<Vec<T>>,
}

impl<'a> BlockchainExplorer<'a> {
    pub fn new(view: &'a View, cfg: GenesisConfig) -> BlockchainExplorer {
        BlockchainExplorer {
            view: view,
            validators: cfg.validators,
        }
    }

    pub fn tx_info<T>(&self, tx_hash: &Hash) -> StorageResult<Option<T>>
        where T: TransactionInfo + From<RawTransaction>
    {
        let tx = Schema::new(self.view).transactions().get(tx_hash)?;
        Ok(tx.and_then(|raw| Some(T::from(raw))))
    }

    pub fn block_info<T>(&self,
                         block_hash: &Hash,
                         full_info: bool)
                         -> StorageResult<Option<BlockInfo<T>>>
        where T: TransactionInfo + From<RawTransaction>
    {
        let schema = Schema::new(self.view);

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

            // TODO Find more common solution
            // FIXME this code was copied from state.rs
            let proposer = ((height + block.propose_round() as u64) %
                            (self.validators.len() as u64)) as u32;

            let precommits_count = schema.precommits(block_hash).len()? as u64;
            let info = BlockInfo {
                height: height,
                proposer: proposer,
                propose_time: block.time().sec,

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

    pub fn block_info_with_height<T>(&self, height: u64) -> StorageResult<Option<BlockInfo<T>>>
        where T: TransactionInfo + From<RawTransaction>
    {
        if let Some(block_hash) = Schema::new(self.view).heights().get(height)? {
            // TODO avoid double unwrap
            self.block_info(&block_hash, true)
        } else {
            Ok(None)
        }
    }

    pub fn blocks_range<T>(&self, count: u64, from: Option<u64>) -> StorageResult<Vec<BlockInfo<T>>>
        where T: TransactionInfo + From<RawTransaction>
    {
        let schema = Schema::new(self.view);
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

    fn block_txs<T>(&self, height: u64) -> StorageResult<Vec<T>>
        where T: TransactionInfo + From<RawTransaction>
    {
        let schema = Schema::new(self.view);
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
}
