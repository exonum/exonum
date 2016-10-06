use std::cmp;

use serde::{Serialize};

use exonum::storage::{Map, List};
use exonum::storage::{Result as StorageResult};
use exonum::crypto::{Hash};
use exonum::blockchain::{Blockchain, View};

use super::HexField;

pub struct BlockchainExplorer<B: Blockchain> {
    view: B::View,
}

pub trait TransactionInfo: Serialize {}

#[derive(Debug, Serialize)]
pub struct BlockInfo<T>
    where T: TransactionInfo
{
    height: u64,
    // proposer: PublicKey, // TODO add to block dto
    propose_time: i64,

    prev_hash: HexField<Hash>,
    hash: HexField<Hash>,
    state_hash: HexField<Hash>,
    tx_hash: HexField<Hash>,
    txs: Vec<T>,
}

impl<B: Blockchain> BlockchainExplorer<B> {
    pub fn new(b: B) -> BlockchainExplorer<B> {
        BlockchainExplorer { view: b.view() }
    }

    pub fn from_view(view: B::View) -> BlockchainExplorer<B> {
        BlockchainExplorer { view: view }
    }

    pub fn tx_info<T>(&self, tx_hash: &Hash) -> StorageResult<Option<T>>
        where T: TransactionInfo + From<B::Transaction>
    {
        let tx = self.view.transactions().get(tx_hash)?;
        Ok(tx.map(|tx| T::from(tx)))
    }

    pub fn block_info<T>(&self, block_hash: &Hash) -> StorageResult<Option<BlockInfo<T>>>
        where T: TransactionInfo + From<B::Transaction>
    {
        let block = self.view.blocks().get(block_hash)?;
        if let Some(block) = block {
            let block_txs = self.block_txs(block.height())?;
            let info = BlockInfo {
                height: block.height(),
                // proposer: block.proposer(),
                propose_time: block.time().sec,

                prev_hash: HexField(*block.prev_hash()),
                hash: HexField(*block_hash),
                state_hash: HexField(*block.state_hash()),
                tx_hash: HexField(*block.tx_hash()),
                txs: block_txs,
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
            self.block_info(&block_hash)
        } else {
            Ok(None)
        }
    }

    pub fn blocks_range<T>(&self, from: u64, to: Option<u64>) -> StorageResult<Vec<BlockInfo<T>>>
        where T: TransactionInfo + From<B::Transaction>
    {
        let heights = self.view.heights();

        let max_len = heights.len()?;
        let len = cmp::min(max_len, to.unwrap_or(max_len));

        let mut v = Vec::new();
        for height in from..len {
            if let Some(ref h) = heights.get(height)? {
                if let Some(block_info) = self.block_info(h)? {
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