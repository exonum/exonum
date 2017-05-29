use serde_json::Value;

use std::cmp;

use storage::{Map, List, Result as StorageResult};
use storage::Proofnode;
use crypto::Hash;
use blockchain::{Schema, Blockchain, Block, TxLocation};
use messages::Precommit;

pub use self::explorer_api::{ExplorerApi};

mod explorer_api;

pub struct BlockchainExplorer<'a> {
    blockchain: &'a Blockchain,
}

#[derive(Debug, Serialize)]
pub struct BlockInfo {
    block: Block,
    precommits: Vec<Precommit>,
    txs: Vec<Hash>,
}

#[derive(Debug, Serialize)]
pub struct TxInfo {
    content: Value,
    location: TxLocation,
    proof_to_block_merkle_root: Proofnode<Hash>,
}

impl<'a> BlockchainExplorer<'a> {
    pub fn new(blockchain: &'a Blockchain) -> BlockchainExplorer {
        BlockchainExplorer { blockchain: blockchain }
    }

    pub fn tx_info(&self, tx_hash: &Hash) -> StorageResult<Option<TxInfo>> {
        let b = self.blockchain.clone();
        let view = b.view();
        let schema = Schema::new(&view);
        let tx = schema.transactions().get(tx_hash)?;
        let res = match tx {
            None => None,
            Some(raw_tx) => {
                //Explicit panic here if no matching service found
                //TODO:Replace with service_not_found error
                let box_transaction = self.blockchain.tx_from_raw(raw_tx).
                    expect("Service not found");
                let content = box_transaction.info();

                let location = schema
                    .tx_location_by_tx_hash()
                    .get(tx_hash)?
                    .expect(&format!("Not found tx_hash location: {:?}", tx_hash));

                let block_height = location.block_height();
                let tx_index = location.position_in_block();
                let proof = schema.block_txs(block_height).
                    construct_path_for_range(tx_index, tx_index+1)?;
                let tx_info = TxInfo {
                    content: content,
                    location: location,
                    proof_to_block_merkle_root: proof,
                };
                Some(tx_info)
            }
        };
        Ok(res)

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
                let bl = BlockInfo {
                    block: proof.block,
                    precommits: proof.precommits,
                    txs: txs,
                };
                Some(bl)
            }
        };
        Ok(res)
    }

    pub fn blocks_range(&self, count: u64, upper: Option<u64>, skip_empty_blocks: bool) -> StorageResult<Vec<Block>> {
        let b = self.blockchain.clone();
        let view = b.view();
        let schema = Schema::new(&view);
        let hashes = schema.block_hashes_by_height();
        let blocks = schema.blocks();

        let max_len = hashes.len()?;
        let upper = upper.map(|x| cmp::min(x, max_len)).unwrap_or(max_len);
        let lower = upper.checked_sub(count).unwrap_or(0);

        let mut v = Vec::new();
        for height in (lower..upper).rev() {
            let block_txs = schema.block_txs(height);
            if skip_empty_blocks && block_txs.is_empty()? {
               continue;
            }
            let block_hash = hashes.get(height)?.
                expect(&format!("Block not found, height:{:?}", height));
            let block = blocks.get(&block_hash)?.
                expect(&format!("Block not found, hash:{:?}", block_hash));
            v.push(block)
        }
        Ok(v)
    }
}
