use serde_json::Value;

use std::cmp;

use storage::{Map, List, Result as StorageResult};
use crypto::Hash;
use blockchain::{Schema, Blockchain, Block};
use messages::Precommit;

pub use self::explorer_api::{ExplorerApi, BlocksRequest};

mod explorer_api;

pub struct BlockchainExplorer<'a> {
    blockchain: &'a Blockchain,
}

#[derive(Debug, Serialize)]
pub struct BlockInfo {
    pub block: Block,
    pub precommits: Vec<Precommit>,
    txs: Vec<Hash>,
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

    pub fn block_info(&self, height: u64) -> StorageResult<Option<BlockInfo>> {
        let b = self.blockchain.clone();
        let view = b.view();
        let schema = Schema::new(&view);
        let txs_table = schema.block_txs(height);
        let block_proof = schema.block_and_precommits(height)?;
        let res = match block_proof {
            None => None,
            Some(proof) => {
               let txs = txs_table.values()?;
               let bl =  BlockInfo {
                    block: proof.block,
                    precommits: proof.precommits,
                    txs: txs,
               };
               Some(bl)
            }
        };
        Ok(res)
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

    pub fn blocks_range(&self, count: u64, from: Option<u64>) -> StorageResult<Vec<BlockInfo>> {
        let b = self.blockchain.clone();
        let view = b.view();
        let schema = Schema::new(&view);
        let hashes = schema.block_hashes_by_height();

        let max_len = hashes.len()?;
        let to = from.map(|x| cmp::min(x, max_len)).unwrap_or(max_len);
        let from = to.checked_sub(count).unwrap_or(0);

        let mut v = Vec::new();
        for height in (from..to).rev() {
            if let Some(ref h) = hashes.get(height)? {
                unimplemented!();
            }

        }
        Ok(v)
    }
}
