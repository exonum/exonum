use serde::{Serialize, Serializer};

use exonum::crypto::{HexValue, PublicKey, Hash, ToHex};
use exonum::storage::{List, Map, MerkleTable, Database, Result as StorageResult};
use exonum::blockchain::Blockchain;
use blockchain_explorer::{BlockchainExplorer, TransactionInfo};

use super::{DigitalRightsTx, DigitalRightsBlockchain};
// use super::wallet::{Wallet, WalletId};

impl Serialize for DigitalRightsTx {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state;
        match *self {
            DigitalRightsTx::CreateOwner(ref tx) => {
                state = ser.serialize_struct("transaction", 3)?;
                ser.serialize_struct_elt(&mut state, "type", "create_owner")?;
                ser.serialize_struct_elt(&mut state, "name", tx.name())?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key())?;
            }
            DigitalRightsTx::CreateDistributor(ref tx) => {
                state = ser.serialize_struct("transaction", 3)?;
                ser.serialize_struct_elt(&mut state, "name", tx.name())?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key())?;
            }
            _ => {
                unimplemented!();
            }
        }
        ser.serialize_struct_end(state)
    }
}

impl TransactionInfo for DigitalRightsTx {}

pub struct HexField<T: AsRef<[u8]>>(T);

impl<T> Serialize for HexField<T>
    where T: AsRef<[u8]>
{
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        ser.serialize_str(&self.0.as_ref().to_hex())
    }
}

#[derive(Serialize)]
pub struct OwnerInfo {
    name: String,
    pub_key: HexField<PublicKey>,
    ownership: HexField<Hash>,
}

#[derive(Serialize)]
pub struct DistributorInfo {
    name: String,
    pub_key: HexField<PublicKey>,
    contracts: HexField<Hash>,
}

// pub struct WalletInfo {
//     inner: Wallet,
//     id: WalletId,
//     history: Vec<CurrencyTx>,
// }

// impl Serialize for WalletInfo {
//     fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
//         where S: Serializer
//     {
//         let mut state = ser.serialize_struct("wallet", 7)?;
//         ser.serialize_struct_elt(&mut state, "id", self.id)?;
//         ser.serialize_struct_elt(&mut state, "balance", self.inner.balance())?;
//         ser.serialize_struct_elt(&mut state, "name", self.inner.name())?;
//         ser.serialize_struct_elt(&mut state, "history", &self.history)?;
//         ser.serialize_struct_elt(&mut state,
//                                   "history_hash",
//                                   self.inner.history_hash().to_hex())?;
//         ser.serialize_struct_end(state)
//     }
// }

pub struct DigitalRightsApi<D: Database> {
    blockchain: DigitalRightsBlockchain<D>,
}

impl<D: Database> DigitalRightsApi<D> {
    pub fn new(b: DigitalRightsBlockchain<D>) -> DigitalRightsApi<D> {
        DigitalRightsApi { blockchain: b }
    }

    pub fn owner_info(&self, owner_id: u64) -> StorageResult<Option<OwnerInfo>> {
        let view = self.blockchain.view();
        if let Some(owner) = view.owners().get(owner_id)? {
            let info = OwnerInfo {
                name: owner.name().to_string(),
                pub_key: HexField(*owner.pub_key()),
                ownership: HexField(*owner.ownership_hash()),
            };
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    pub fn distributor_info(&self, owner_id: u64) -> StorageResult<Option<DistributorInfo>> {
        let view = self.blockchain.view();
        if let Some(distributor) = view.distributors().get(owner_id)? {
            let info = DistributorInfo {
                name: distributor.name().to_string(),
                pub_key: HexField(*distributor.pub_key()),
                contracts: HexField(*distributor.contracts_hash()),
            };
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }
}

// pub struct CurrencyApi<D: Database> {
//     blockchain: CurrencyBlockchain<D>,
// }

// impl<D: Database> CurrencyApi<D> {
//     pub fn new(b: CurrencyBlockchain<D>) -> CurrencyApi<D> {
//         CurrencyApi { blockchain: b }
//     }

//     pub fn wallet_info(&self, pub_key: &PublicKey) -> StorageResult<Option<WalletInfo>> {
//         let view = self.blockchain.view();
//         if let Some((id, wallet)) = view.wallet(pub_key)? {
//             let history = view.wallet_history(id).values()?;
//             let txs = {
//                 let mut v = Vec::new();

//                 let explorer = BlockchainExplorer::<CurrencyBlockchain<D>>::from_view(view);
//                 for hash in history {
//                     if let Some(tx_info) = explorer.tx_info::<CurrencyTx>(&hash)? {
//                         v.push(tx_info)
//                     }
//                 }
//                 v
//             };

//             let info = WalletInfo {
//                 id: id,
//                 inner: wallet,
//                 history: txs,
//             };
//             Ok(Some(info))
//         } else {
//             Ok(None)
//         }
//     }
// }
