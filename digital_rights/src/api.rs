use serde::{Serialize, Serializer};

use exonum::crypto::{PublicKey, Hash, HexValue};
use exonum::storage::{List, Database, Result as StorageResult};
use exonum::blockchain::Blockchain;
use blockchain_explorer::{TransactionInfo, HexField};

use super::{DigitalRightsTx, DigitalRightsBlockchain, ContentShare};

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
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key().to_hex())?;
            }
            DigitalRightsTx::CreateDistributor(ref tx) => {
                state = ser.serialize_struct("transaction", 3)?;
                ser.serialize_struct_elt(&mut state, "type", "create_distributor")?;
                ser.serialize_struct_elt(&mut state, "name", tx.name())?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key().to_hex())?;
            }
            DigitalRightsTx::AddContent(ref tx) => {
                state = ser.serialize_struct("transaction", 3)?;
                ser.serialize_struct_elt(&mut state, "type", "create_distributor")?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key().to_hex())?;
                ser.serialize_struct_elt(&mut state, "fingerprint", tx.fingerprint().to_hex())?;
                ser.serialize_struct_elt(&mut state, "title", tx.title())?;                
                ser.serialize_struct_elt(&mut state, "price_per_listen", tx.price_per_listen())?;                
                ser.serialize_struct_elt(&mut state, "min_plays", tx.min_plays())?;  
                ser.serialize_struct_elt(&mut state, "additional_conditions", tx.title())?; 
                ser.serialize_struct_elt(&mut state, "owners", tx.owner_shares())?;                
            }
            _ => {
                unimplemented!();
            }
        }
        ser.serialize_struct_end(state)
    }
}

impl TransactionInfo for DigitalRightsTx {}

#[derive(Debug, Serialize, Deserialize)]
pub struct OwnerInfo {
    name: String,
    pub_key: HexField<PublicKey>,
    ownership: HexField<Hash>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DistributorInfo {
    name: String,
    pub_key: HexField<PublicKey>,
    contracts: HexField<Hash>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewContent {
    pub title: String,
    pub fingerprint: HexField<Hash>,
    pub additional_conditions: String,
    pub price_per_listen: u64,
    pub min_plays: u64,
    pub owners: Vec<ContentShare>,
}

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
