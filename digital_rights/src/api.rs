use std::marker::PhantomData;

use serde::{Serialize, Serializer};
use serde::de;
use serde::de::{Visitor, Deserialize, Deserializer};

use exonum::crypto::{HexValue, PublicKey, Hash, ToHex};
use exonum::storage::{List, Database, Result as StorageResult};
use exonum::blockchain::Blockchain;
use blockchain_explorer::TransactionInfo;

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

#[derive(Debug)]
pub struct HexField<T: AsRef<[u8]>>(pub T);

impl<T> Serialize for HexField<T>
    where T: AsRef<[u8]>
{
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        ser.serialize_str(&self.0.as_ref().to_hex())
    }
}

struct HexVisitor<T>
    where T: AsRef<[u8]> + HexValue
{
    _p: PhantomData<T>,
}

impl<T> Visitor for HexVisitor<T>
    where T: AsRef<[u8]> + HexValue
{
    type Value = HexField<T>;

    fn visit_str<E>(&mut self, s: &str) -> Result<HexField<T>, E>
        where E: de::Error
    {
        let v = T::from_hex(s).map_err(|_| de::Error::custom("Invalid hex"))?;
        Ok(HexField(v))
    }
}

impl<T> Deserialize for HexField<T>
    where T: AsRef<[u8]> + HexValue
{
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_str(HexVisitor { _p: PhantomData })
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentInfo {
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
