use serde::Serialize;

use exonum::storage::Database;
use exonum::storage::Result as StorageResult;
use exonum::crypto::Hash;

pub trait BlockchainExplorer<D: Database> {
    type BlockInfo: Serialize;
    type TxInfo: Serialize;

    fn blocks_range(&self, from: u64, to: Option<u64>) -> StorageResult<Vec<Self::BlockInfo>>;
    fn get_tx_info(&self, hash: &Hash) -> StorageResult<Option<Self::TxInfo>>;
    fn get_tx_hashes_from_block(&self, height: u64) -> StorageResult<Vec<Hash>>;
    fn get_block_info(&self, height: u64) -> StorageResult<Option<Self::BlockInfo>> {
        let range = self.blocks_range(height, Some(height + 1))?;
        Ok(range.into_iter().next())
    }
    fn get_txs<H: AsRef<[Hash]>>(&self, hashes: H) -> StorageResult<Vec<Self::TxInfo>> {
        let mut v = Vec::new();
        for h in hashes.as_ref() {
            if let Some(tx_info) = self.get_tx_info(h)? {
                v.push(tx_info)
            }
        }
        Ok(v)
    }
    fn get_txs_for_block(&self, height: u64) -> StorageResult<Vec<Self::TxInfo>> {
        let hashes = self.get_tx_hashes_from_block(height)?;
        self.get_txs(&hashes)
    }
}