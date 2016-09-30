use std::cmp::min;

use serde::{Serialize, Serializer};

use exonum::crypto::{Hash, PublicKey};
use exonum::storage::{Map, Database, List, Result as StorageResult};
use exonum::blockchain::{View, Block};
use utils::HexValue;
use utils::blockchain_explorer::BlockchainExplorer;

use super::{CurrencyView, CurrencyTx};
use super::wallet::{Wallet, WalletId};

pub struct BlockInfo {
    inner: Block,
    txs: Vec<TxInfo>,
}

pub struct TxInfo {
    inner: CurrencyTx,
}

impl Serialize for BlockInfo {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let b = &self.inner;
        // TODO think about timespec serialize
        let tm = ::time::at(b.time()).rfc3339().to_string();
        let mut state = serializer.serialize_struct("block", 7)?;
        serializer.serialize_struct_elt(&mut state, "height", b.height())?;

        serializer.serialize_struct_elt(&mut state, "hash", b.hash().to_hex())?;
        serializer.serialize_struct_elt(&mut state, "prev_hash", b.prev_hash().to_hex())?;
        serializer.serialize_struct_elt(&mut state, "state_hash", b.state_hash().to_hex())?;
        serializer.serialize_struct_elt(&mut state, "tx_hash", b.tx_hash().to_hex())?;

        serializer.serialize_struct_elt(&mut state, "time", tm)?;
        serializer.serialize_struct_elt(&mut state, "txs", &self.txs)?;
        serializer.serialize_struct_end(state)
    }
}

impl Serialize for TxInfo {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let tx = &self.inner;
        let mut state;
        match *tx {
            CurrencyTx::Issue(ref issue) => {
                state = ser.serialize_struct("transaction", 4)?;
                ser.serialize_struct_elt(&mut state, "type", "issue")?;
                ser.serialize_struct_elt(&mut state, "wallet", issue.wallet().to_hex())?;
                ser.serialize_struct_elt(&mut state, "amount", issue.amount())?;
                ser.serialize_struct_elt(&mut state, "seed", issue.seed())?;
            }
            CurrencyTx::Transfer(ref transfer) => {
                state = ser.serialize_struct("transaction", 5)?;
                ser.serialize_struct_elt(&mut state, "type", "transfer")?;
                ser.serialize_struct_elt(&mut state, "from", transfer.from().to_hex())?;
                ser.serialize_struct_elt(&mut state, "to", transfer.to().to_hex())?;
                ser.serialize_struct_elt(&mut state, "amount", transfer.amount())?;
                ser.serialize_struct_elt(&mut state, "seed", transfer.seed())?;
            }
            CurrencyTx::CreateWallet(ref wallet) => {
                state = ser.serialize_struct("transaction", 3)?;
                ser.serialize_struct_elt(&mut state, "type", "create_wallet")?;
                ser.serialize_struct_elt(&mut state, "pub_key", wallet.pub_key().to_hex())?;
                ser.serialize_struct_elt(&mut state, "name", wallet.name())?;
            }
        }
        ser.serialize_struct_end(state)
    }
}


impl<D> BlockchainExplorer<D> for CurrencyView<D::Fork>
    where D: Database
{
    type BlockInfo = BlockInfo;
    type TxInfo = TxInfo;

    fn blocks_range(&self, from: u64, to: Option<u64>) -> StorageResult<Vec<Self::BlockInfo>> {
        let heights = self.heights();
        let blocks = self.blocks();

        let max_len = heights.len()?;
        let len = min(max_len, to.unwrap_or(max_len));

        let mut v = Vec::new();
        for height in from..len {
            if let Some(ref h) = heights.get(height)? {
                if let Some(block) = blocks.get(h)? {
                    let txs = BlockchainExplorer::<D>::get_txs_for_block(self, height)?;
                    v.push(BlockInfo {
                        inner: block,
                        txs: txs,
                    });
                }
            }
        }
        Ok(v)
    }

    fn get_tx_info(&self, hash: &Hash) -> StorageResult<Option<Self::TxInfo>> {
        let tx = self.transactions().get(hash)?;
        Ok(tx.map(|tx| TxInfo { inner: tx }))
    }

    fn get_tx_hashes_from_block(&self, height: u64) -> StorageResult<Vec<Hash>> {
        let txs = self.block_txs(height);
        let tx_count = txs.len()?;
        let mut v = Vec::new();
        for i in 0..tx_count {
            if let Some(tx_hash) = txs.get(i)? {
                v.push(tx_hash);
            }
        }
        Ok(v)
    }
}

pub struct WalletInfo {
    inner: Wallet,
    id: WalletId,
    history: Vec<TxInfo>,
}

impl Serialize for WalletInfo {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state = ser.serialize_struct("wallet", 7)?;
        ser.serialize_struct_elt(&mut state, "id", self.id)?;
        ser.serialize_struct_elt(&mut state, "balance", self.inner.balance())?;
        ser.serialize_struct_elt(&mut state, "name", self.inner.name())?;
        ser.serialize_struct_elt(&mut state, "history", &self.history)?;
        ser.serialize_struct_elt(&mut state, "history_hash", self.inner.history_hash().to_hex())?;
        ser.serialize_struct_end(state)
    }
}

pub trait CryptocurrencyApi<D: Database> {
    type WalletInfo: Serialize;

    fn wallet_info(&self, pub_key: &PublicKey) -> StorageResult<Option<Self::WalletInfo>>;
}

impl<D> CryptocurrencyApi<D> for CurrencyView<D::Fork>
    where D: Database
{
    type WalletInfo = WalletInfo;

    fn wallet_info(&self, pub_key: &PublicKey) -> StorageResult<Option<WalletInfo>> {
        if let Some((id, wallet)) = self.wallet(pub_key)? {
            let history = self.wallet_history(id);
            let hashes = history.values()?;
            let txs = BlockchainExplorer::<D>::get_txs(self, hashes)?;

            let info = WalletInfo {
                id: id,
                inner: wallet,
                history: txs,
            };
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }
}