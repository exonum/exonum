use std::cmp;

use serde::Serialize;

use exonum::storage::{Map, List};
use exonum::storage::Result as StorageResult;
use exonum::crypto::{Hash, PublicKey};
use exonum::blockchain::{Blockchain, View};
use exonum::node::Configuration;

use super::HexField;

pub struct BlockchainExplorer<B: Blockchain> {
    view: B::View,
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

impl<B: Blockchain> BlockchainExplorer<B> {
    pub fn new(b: B, cfg: Configuration) -> BlockchainExplorer<B> {
        BlockchainExplorer {
            view: b.view(),
            validators: cfg.validators,
        }
    }

    pub fn from_view(view: B::View, cfg: Configuration) -> BlockchainExplorer<B> {
        BlockchainExplorer {
            view: view,
            validators: cfg.validators,
        }
    }

    pub fn tx_info<T>(&self, tx_hash: &Hash) -> StorageResult<Option<T>>
        where T: TransactionInfo + From<B::Transaction>
    {
        let tx = self.view.transactions().get(tx_hash)?;
        Ok(tx.map(|tx| T::from(tx)))
    }

    pub fn block_info<T>(&self,
                         block_hash: &Hash,
                         full_info: bool)
                         -> StorageResult<Option<BlockInfo<T>>>
        where T: TransactionInfo + From<B::Transaction>
    {
        let block = self.view.blocks().get(block_hash)?;
        if let Some(block) = block {
            let height = block.height();
            let (txs, txs_count) = {
                if full_info {
                    let txs = self.block_txs(block.height())?;
                    let txs_count = txs.len() as u64;
                    (Some(txs), txs_count)
                } else {
                    (None, self.view.block_txs(height).len()? as u64)
                }
            };

            // TODO Find more common solution
            // FIXME this code was copied from state.rs
            let proposer = ((height + block.propose_round() as u64) %
                            (self.validators.len() as u64)) as u32;

            let precommits_count = self.view.precommits(block_hash).len()? as u64;
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
        where T: TransactionInfo + From<B::Transaction>
    {
        if let Some(block_hash) = self.view.heights().get(height)? {
            // TODO avoid double unwrap
            self.block_info(&block_hash, true)
        } else {
            Ok(None)
        }
    }

    pub fn blocks_range<T>(&self, count: u64, from: Option<u64>) -> StorageResult<Vec<BlockInfo<T>>>
        where T: TransactionInfo + From<B::Transaction>
    {
        let heights = self.view.heights();

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
        where T: TransactionInfo + From<B::Transaction>
    {
        let txs = self.view.block_txs(height);
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
