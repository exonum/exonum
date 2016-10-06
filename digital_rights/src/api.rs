use serde::{Serialize, Serializer};

use exonum::crypto::{PublicKey, Hash, HexValue};
use exonum::storage::{Map, List, Database, Result as StorageResult};
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

#[derive(Serialize)]
pub enum UserRole<O, D>
    where D: Serialize,
          O: Serialize
{
    Distributor(D),
    Owner(O),
}

#[derive(Debug, Serialize)]
pub struct OwnerInfo {
    pub name: String,
    pub pub_key: HexField<PublicKey>,
    pub ownership: HexField<Hash>,
}

#[derive(Debug, Serialize)]
pub struct DistributorInfo {
    pub name: String,
    pub pub_key: HexField<PublicKey>,
    pub contracts: HexField<Hash>,
}

#[derive(Debug, Serialize)]
pub struct ContentInfo {
    pub title: String,
    pub fingerprint: HexField<Hash>,
    pub additional_conditions: String,
    pub price_per_listen: u32,
    pub min_plays: u32,
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

    pub fn participant_id(&self, pub_key: &PublicKey) -> StorageResult<Option<u16>> {
        let view = self.blockchain.view();
        let r = view.participants().get(pub_key)?;
        r.map(|i| i as u16);
        Ok(r)
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

    pub fn available_contents(&self, distributor_id: u64) -> StorageResult<Vec<ContentInfo>> {
        let view = self.blockchain.view();
        let distributor_id = distributor_id as u16;

        let v = view.list_content()?
            .into_iter()
            .filter(|&(_, ref content)| !content.distributors().contains(&distributor_id))
            .map(|(fingerprint, content)| {
                ContentInfo {
                    title: content.title().to_string(),
                    fingerprint: HexField(fingerprint),
                    additional_conditions: content.additional_conditions().to_string(),
                    price_per_listen: content.price_per_listen(),
                    min_plays: content.min_plays(),
                }
            })
            .collect();
        Ok(v)
    }
}
