use serde::{Serialize, Serializer};
use txs::{ObjectTx, Point};
use view::ObjectId;
use super::ObjectsBlockchain;
use exonum::storage::{Map, List, Database, Result as StorageResult};
use exonum::crypto::{PublicKey, Hash, HexValue};
use exonum::blockchain::Blockchain;
use blockchain_explorer::{TransactionInfo, HexField};

impl Serialize for ObjectTx {

    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error> where S: Serializer
    {
        let mut state;
        match *self {
            ObjectTx::CreateOwner(ref tx) => {
                state = ser.serialize_struct("transaction", 3)?;
                ser.serialize_struct_elt(&mut state, "type", "create_owner")?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key())?;
                ser.serialize_struct_elt(&mut state, "name", tx.name())?;
            }
            ObjectTx::CreateObject(ref tx) => {
                state = ser.serialize_struct("transaction", 5)?;
                ser.serialize_struct_elt(&mut state, "type", "create_object")?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key())?;
                ser.serialize_struct_elt(&mut state, "title", tx.title())?;
                ser.serialize_struct_elt(&mut state, "points", tx.points())?;
                ser.serialize_struct_elt(&mut state, "owner_pub_key", tx.owner_pub_key())?;
            }
            ObjectTx::ModifyObject(ref tx) => {
                state = ser.serialize_struct("transaction", 5)?;
                ser.serialize_struct_elt(&mut state, "type", "modify_object")?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key())?;
                ser.serialize_struct_elt(&mut state, "object_id", tx.object_id())?;
                ser.serialize_struct_elt(&mut state, "title", tx.title())?;
                ser.serialize_struct_elt(&mut state, "points", tx.points())?;
            }
            ObjectTx::TransferObject(ref tx) => {
                state = ser.serialize_struct("transaction", 4)?;
                ser.serialize_struct_elt(&mut state, "type", "transfer_object")?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key())?;
                ser.serialize_struct_elt(&mut state, "object_id", tx.object_id())?;
                ser.serialize_struct_elt(&mut state, "owner_pub_key", tx.owner_pub_key())?;

            }
            ObjectTx::RemoveObject(ref tx) => {
                state = ser.serialize_struct("transaction", 3)?;
                ser.serialize_struct_elt(&mut state, "type", "remove_object")?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key())?;
                ser.serialize_struct_elt(&mut state, "object_id", tx.object_id())?;
            }
        }
        ser.serialize_struct_end(state)
    }
}

impl TransactionInfo for ObjectTx {}

#[derive(Debug, Serialize, Deserialize)]
pub struct OwnerInfo {
    pub pub_key: HexField<PublicKey>,
    pub name: String,
    pub ownership_hash: HexField<Hash>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub pub_key: HexField<PublicKey>,
    pub title: String,
    pub points: Vec<Point>,
    pub owner_pub_key: HexField<PublicKey>,
    pub deleted: bool,
    pub history_hash: HexField<Hash>,
}

pub struct ObjectsApi<D: Database> {
    blockchain: ObjectsBlockchain<D>,
}

impl<D: Database> ObjectsApi<D> {

    pub fn new(b: ObjectsBlockchain<D>) -> ObjectsApi<D> {
        ObjectsApi { blockchain: b }
    }

    pub fn owner_info(&self, pub_key: &PublicKey) -> StorageResult<Option<OwnerInfo>> {
        let view = self.blockchain.view();
        if let Some(owner) = view.owners().get(pub_key)? {
            let info = OwnerInfo {
                name: owner.name().to_string(),
                pub_key: HexField(*pub_key),
                ownership_hash: HexField(*owner.ownership_hash()),
            };
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    pub fn object_info(&self, object_id: ObjectId) -> StorageResult<Option<ObjectInfo>> {
        let view = self.blockchain.view();
        if let Some(object) = view.objects().get(object_id)? {
            let info = ObjectInfo {
                pub_key: HexField(*object.pub_key()),
                title: object.title().to_string(),
                points: object.points().iter().map(|x| (*x as u64).into()).collect::<Vec<Point>>(),
                owner_pub_key: HexField(*object.owner_pub_key()),
                deleted: object.deleted(),
                history_hash: HexField(*object.history_hash()),
            };
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    // pub fn distributor_info(&self, id: u16) -> StorageResult<Option<DistributorInfo>> {
    //     let view = self.blockchain.view();
    //     if let Some(distributor) = view.distributors().get(id as u64)? {
    //         let info = DistributorInfo {
    //             name: distributor.name().to_string(),
    //             pub_key: HexField(*distributor.pub_key()),
    //             contracts: HexField(*distributor.contracts_hash()),
    //         };
    //         Ok(Some(info))
    //     } else {
    //         Ok(None)
    //     }
    // }

    // pub fn available_contents(&self, distributor_id: u16) -> StorageResult<Vec<ContentInfo>> {
    //     let v = self.blockchain
    //         .view()
    //         .list_content()?
    //         .into_iter()
    //         .filter(|&(_, ref content)| !content.distributors().contains(&distributor_id))
    //         .map(|(fingerprint, content)| {
    //             ContentInfo {
    //                 title: content.title().to_string(),
    //                 fingerprint: HexField(fingerprint),
    //                 additional_conditions: content.additional_conditions().to_string(),
    //                 price_per_listen: content.price_per_listen(),
    //                 min_plays: content.min_plays(),
    //             }
    //         })
    //         .collect();
    //     Ok(v)
    // }
}

