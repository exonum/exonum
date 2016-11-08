use serde::{Serialize, Serializer};
use txs::{ObjectTx, GeoPoint};
use view::{ Object, Owner, ObjectId, TxResult, ObjectHistory };
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
            ObjectTx::Register(ref tx) => {
                state = ser.serialize_struct("transaction", 2)?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key())?;
                ser.serialize_struct_elt(&mut state, "name", tx.name())?;
            }
            ObjectTx::CreateOwner(ref tx) => {
                state = ser.serialize_struct("transaction", 4)?;
                ser.serialize_struct_elt(&mut state, "type", "create_owner")?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key())?;
                ser.serialize_struct_elt(&mut state, "firstname", tx.firstname())?;
                ser.serialize_struct_elt(&mut state, "lastname", tx.lastname())?;
            }
            ObjectTx::CreateObject(ref tx) => {
                state = ser.serialize_struct("transaction", 5)?;
                ser.serialize_struct_elt(&mut state, "type", "create_object")?;
                ser.serialize_struct_elt(&mut state, "pub_key", tx.pub_key())?;
                ser.serialize_struct_elt(&mut state, "title", tx.title())?;
                ser.serialize_struct_elt(&mut state, "points", tx.points())?;
                ser.serialize_struct_elt(&mut state, "owner_id", tx.owner_id())?;
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
                ser.serialize_struct_elt(&mut state, "owner_id", tx.owner_id())?;

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
    pub id: u64,
    pub firstname: String,
    pub lastname: String,
    pub ownership_hash: HexField<Hash>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewOwner {
    pub firstname: String,
    pub lastname: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryInfo {
    pub old_owner: OwnerInfo,
    pub new_owner: OwnerInfo,
    pub operation: u8

}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShortObjectInfo {
    pub id: u64,
    pub title: String,
    pub points: Vec<GeoPoint>,
    pub owner_id: u64,
    pub deleted: bool
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub id: u64,
    pub title: String,
    pub points: Vec<GeoPoint>,
    pub owner_id: u64,
    pub deleted: bool,
    pub owner: OwnerInfo,
    pub history: Vec<HistoryInfo>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewObject {
    pub title: String,
    pub points: Vec<GeoPoint>,
    pub owner_id: u64,
    pub deleted: bool
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResultInfo {
    pub result: u8
}

impl OwnerInfo {
    pub fn from_owner(id: u64, owner: Owner) -> OwnerInfo {
        OwnerInfo {
            id: id,
            firstname: owner.firstname().to_string(),
            lastname: owner.lastname().to_string(),
            ownership_hash: HexField(*owner.ownership_hash()),
        }
    }
}

impl HistoryInfo {
    pub fn new(old_owner: OwnerInfo, new_owner: OwnerInfo, operation: u8) -> HistoryInfo {
        HistoryInfo {
            old_owner: old_owner,
            new_owner: new_owner,
            operation: operation
        }
    }
}

impl ObjectInfo {
    pub fn from_object(id: u64, object: Object, owner: OwnerInfo, history: Vec<HistoryInfo> ) -> ObjectInfo {
        ObjectInfo {
            id: id,
            title: object.title().to_string(),
            points: GeoPoint::from_vec(object.points().iter().map(|x| (*x as f64)).collect::<Vec<f64>>()),
            owner_id: object.owner_id(),
            deleted: object.deleted(),
            owner: owner,
            history: history
        }
    }
}

impl ShortObjectInfo {
    pub fn from_object(id: u64, object: Object) -> ShortObjectInfo {
        ShortObjectInfo {
            id: id,
            title: object.title().to_string(),
            points: GeoPoint::from_vec(object.points().iter().map(|x| (*x as f64)).collect::<Vec<f64>>()),
            owner_id: object.owner_id(),
            deleted: object.deleted()
        }
    }
}

pub struct ObjectsApi<D: Database> {
    blockchain: ObjectsBlockchain<D>,
}

impl<D: Database> ObjectsApi<D> {

    pub fn new(b: ObjectsBlockchain<D>) -> ObjectsApi<D> {
        ObjectsApi { blockchain: b }
    }

    pub fn owners_list(&self) -> StorageResult<Option<Vec<OwnerInfo>>>{

        let view = self.blockchain.view();

        let owners = view.owners();
        let values = owners.values()?;
        let r = values.into_iter()
            .enumerate()
            .map(|(id, owner)| OwnerInfo::from_owner(id as u64, owner)).collect();

        Ok(Some(r))
    }

    pub fn objects_list(&self) -> StorageResult<Option<Vec<ShortObjectInfo>>>{

        let view = self.blockchain.view();

        let objects = view.objects();
        let values = objects.values()?;
        let r = values.into_iter()
            .enumerate()
            .map(|(id, object)| ShortObjectInfo::from_object(id as u64, object)).collect();

        Ok(Some(r))
    }

    pub fn owner_info(&self, owner_id: u64) -> StorageResult<Option<OwnerInfo>> {
        let view = self.blockchain.view();
        if let Some(owner) = view.owners().get(owner_id)? {
            Ok(Some(OwnerInfo::from_owner(owner_id, owner)))
        } else {
            Ok(None)
        }
    }

    pub fn result(&self, tx_hash: Hash) -> StorageResult<Option<ResultInfo>>{
        let view = self.blockchain.view();
        if let Some(result) = view.results().get(&tx_hash)? {
            Ok(Some(ResultInfo{
                result: result.result()
            }))
        } else {
            Ok(None)
        }

    }

    pub fn object_info(&self, object_id: ObjectId) -> StorageResult<Option<ObjectInfo>> {
        let view = self.blockchain.view();
        let owners = view.owners();
        let objects = view.objects();
        if let Some(object) = objects.get(object_id)? {
            if let Some(owner) = owners.get(object.owner_id())? {

                let mut object_history_records = Vec::new();
                for object_history in view.object_history(object_id).values()? {
                    object_history_records.push(
                        HistoryInfo::new(
                            OwnerInfo::from_owner(object_history.old_owner_id(), owners.get(object_history.old_owner_id()).unwrap().unwrap()),
                            OwnerInfo::from_owner(object_history.new_owner_id(), owners.get(object_history.new_owner_id()).unwrap().unwrap()),
                            object_history.operation()
                        )
                    );
                }

                let object_info = ObjectInfo::from_object(object_id, object.clone(), OwnerInfo::from_owner(object.owner_id(), owner), object_history_records);
                return Ok(Some(object_info))
            }
        }
        Ok(None)
    }

}

