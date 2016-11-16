use exonum::messages::Field;
use exonum::blockchain::{View};
use exonum::crypto::{ PublicKey, Hash, hash };
use byteorder::{ByteOrder, LittleEndian};
use exonum::storage::{Fork, MerklePatriciaTable, MapTable, MerkleTable, List, Result as StorageResult};
use txs::ObjectTx;
use std::ops::Deref;


pub type ObjectId = u64;

storage_value! {
    User {
        const SIZE = 32;
        name:                 &str             [00 => 32]
    }
}

storage_value! {
    Owner {
        const SIZE = 128;
        firstname:             &str            [00 => 32]
        lastname:              &str            [32 => 64]
        ownership_hash:        &Hash           [64 => 96]
    }
}

storage_value! {
    Ownership {
        const SIZE = 09;
        object_id:             ObjectId        [00 => 08]
        operation:             bool            [08 => 09]
    }
}

storage_value! {
    ObjectHistory {
        const SIZE = 57;
        operation:             u8              [00 => 01]
        old_owner_id:          u64             [01 => 09]
        new_owner_id:          u64             [09 => 17]
        timestamp:             u64             [17 => 25]
        tx_hash:              &Hash            [25 => 57]
    }
}

storage_value! {
    Object {
        const SIZE = 81;
        title:                 &str            [00 => 32]
        points:                &[f64]          [32 => 40]
        owner_id:              u64             [40 => 48]
        deleted:               bool            [48 => 49]
        history_hash:          &Hash           [49 => 81]
    }
}

storage_value! {
    TxResult {
        const SIZE = 01;
        result:               u8               [00 => 01]
    }
}

impl ObjectHistory{
    pub fn new_create_action(old_owner_id: u64, new_owner_id: u64, timestamp: u64, tx_hash: &Hash) -> ObjectHistory {
        ObjectHistory::new(1, old_owner_id, new_owner_id, timestamp, tx_hash)
    }
    pub fn new_modify_action(old_owner_id: u64, new_owner_id: u64, timestamp: u64, tx_hash: &Hash) -> ObjectHistory {
        ObjectHistory::new(2, old_owner_id, new_owner_id, timestamp, tx_hash)
    }
    pub fn new_transfer_action(old_owner_id: u64, new_owner_id: u64, timestamp: u64, tx_hash: &Hash) -> ObjectHistory {
        ObjectHistory::new(3, old_owner_id, new_owner_id, timestamp, tx_hash)
    }
    pub fn new_remove_action(old_owner_id: u64, new_owner_id: u64, timestamp: u64, tx_hash: &Hash) -> ObjectHistory {
        ObjectHistory::new(4, old_owner_id, new_owner_id, timestamp, tx_hash)
    }
    pub fn new_restore_action(old_owner_id: u64, new_owner_id: u64, timestamp: u64, tx_hash: &Hash) -> ObjectHistory {
        ObjectHistory::new(5, old_owner_id, new_owner_id, timestamp, tx_hash)
    }
}

impl User {
    pub fn set_name(&mut self, name: &str){
        Field::write(&name, &mut self.raw, 00, 32);
    }
}

impl Owner {
    pub fn set_firstname(&mut self, name: &str){
        Field::write(&name, &mut self.raw, 00, 32);
    }
    pub fn set_lastname(&mut self, name: &str){
        Field::write(&name, &mut self.raw, 32, 64);
    }
    pub fn set_ownership_hash(&mut self, hash: &Hash) {
        Field::write(&hash, &mut self.raw, 64, 96);
    }
}

impl Object {
    pub fn set_title(&mut self, title: &str) {
        Field::write(&title, &mut self.raw, 00, 32);
    }
    pub fn set_points(&mut self, points: &[f64]) {
        Field::write(&points, &mut self.raw, 32, 40);
    }
    pub fn set_owner(&mut self, owner_id: u64) {
        Field::write(&owner_id, &mut self.raw, 40, 48);
    }
    pub fn set_deleted(&mut self, deleted: bool) {
        Field::write(&deleted, &mut self.raw, 48, 49);
    }
    pub fn set_history_hash(&mut self, hash: &Hash) {
        Field::write(&hash, &mut self.raw, 49, 81);
    }
    pub fn transfer_to(&mut self, other: u64) {
        self.set_owner(other);
    }

}

impl TxResult {

    pub const RESULT_OK: u8 = 1;
    pub const RESULT_WRONG_POINTS: u8 = 2;
    pub const RESULT_WRONG_NEIGHBOURS: u8 = 3;
    pub const RESULT_CREATE_WRONG_OWNER: u8 = 4;
    pub const RESULT_TRANSFER_WRONG_OWNER: u8 = 5;
    pub const RESULT_TRANSFER_WRONG_OBJECT: u8 = 6;
    pub const RESULT_REMOVE_WRONG_OBJECT: u8 = 7;
    pub const RESULT_RESTORE_WRONG_OBJECT: u8 = 8;
    pub const RESULT_MODIFY_WRONG_OBJECT: u8 = 9;
    pub const RESULT_ALREADY_REGISTERED: u8 = 10;

    pub fn set_result(&mut self, result: u8) {
        Field::write(&result, &mut self.raw, 00, 01);
    }
    pub fn ok() -> TxResult {
        TxResult::new(TxResult::RESULT_OK)
    }
    pub fn create_object_wrong_points() -> TxResult {
        TxResult::new(TxResult::RESULT_WRONG_POINTS)
    }
    pub fn create_object_cross_neighbours() -> TxResult {
        TxResult::new(TxResult::RESULT_WRONG_NEIGHBOURS)
    }
    pub fn create_object_wrong_owner() -> TxResult {
        TxResult::new(TxResult::RESULT_CREATE_WRONG_OWNER)
    }
    pub fn transfer_object_wrong_owner() -> TxResult {
        TxResult::new(TxResult::RESULT_TRANSFER_WRONG_OWNER)
    }
    pub fn transfer_object_wrong_object() -> TxResult {
        TxResult::new(TxResult::RESULT_TRANSFER_WRONG_OBJECT)
    }
    pub fn remove_object_wrong_object() -> TxResult {
        TxResult::new(TxResult::RESULT_REMOVE_WRONG_OBJECT)
    }
    pub fn restore_object_wrong_object() -> TxResult {
        TxResult::new(TxResult::RESULT_RESTORE_WRONG_OBJECT)
    }
    pub fn modify_object_wrong_object() -> TxResult {
        TxResult::new(TxResult::RESULT_MODIFY_WRONG_OBJECT)
    }
    pub fn register_already_registered() -> TxResult {
        TxResult::new(TxResult::RESULT_ALREADY_REGISTERED)
    }

}

pub struct ObjectsView<F: Fork> {
    pub fork: F,
}

impl<F> View<F> for ObjectsView<F> where F: Fork
{
    type Transaction = ObjectTx;

    fn from_fork(fork: F) -> Self {
        ObjectsView { fork: fork }
    }
}

impl<F> Deref for ObjectsView<F> where F: Fork
{
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.fork
    }
}

impl<F> ObjectsView<F> where F: Fork
{
    pub fn users(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, PublicKey, User> {
        MerklePatriciaTable::new(MapTable::new(vec![50], &self))
    }

    pub fn  owners(&self) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Owner> {
        MerkleTable::new(MapTable::new(vec![51], &self))
    }
    pub fn  objects(&self) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Object> {
        MerkleTable::new(MapTable::new(vec![52], &self))
    }
    pub fn owner_objects(&self, owner_id: u64) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u16, Ownership> {
        let mut prefix = vec![53; 9];
        LittleEndian::write_u64(&mut prefix[1..], owner_id);
        MerkleTable::new(MapTable::new(prefix, &self))
    }

    pub fn object_history(&self, object_id: u64) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, ObjectHistory> {
        let mut prefix = vec![54; 9];
        LittleEndian::write_u64(&mut prefix[1..], object_id);
        MerkleTable::new(MapTable::new(prefix, &self))
    }

    pub fn results(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, Hash, TxResult> {
        MerklePatriciaTable::new(MapTable::new(vec![55], &self))
    }

    pub fn find_objects_for_owner(&self, owner_id: u64) -> StorageResult<Vec<(ObjectId, Object)>> {
        let mut v = Vec::new();
        for ownership in self.owner_objects(owner_id).values()? {
            if let Some(object) = self.objects().get(ownership.object_id())? {
                if object.owner_id() == owner_id && ownership.operation() && !object.deleted() {
                    v.push((ownership.object_id(), object));
                }
            }
        }
        Ok(v)

    }
}

#[cfg(test)]
mod tests {

    use exonum::crypto::hash;
    use super::{Owner, Object, Ownership, User};
    use txs::{GeoPoint};


    #[test]
    fn test_user(){
        // Arrange
        let name = "test user";

        // Act
        let user = User::new(name);

        // Assert
        assert_eq!(user.name(), "test user");

    }

    #[test]
    fn test_create_owner() {
        // Arrange
        let hash = hash(&[]);
        // Act
        let owner = Owner::new("firstname", "lastname", &hash);
        // Assert
        assert_eq!(owner.firstname(), "firstname");
        assert_eq!(owner.lastname(), "lastname");
        assert_eq!(owner.ownership_hash(), &hash);
    }

    #[test]
    fn test_create_ownership(){
        // Arrange
        let object_id = 1_u64;
        let operation = true;
        // Act
        let ownership = Ownership::new(object_id, operation);
        // Assert
        assert_eq!(ownership.object_id(), 1_u64);
        assert_eq!(ownership.operation(), true);
    }

    #[test]
    fn test_create_object(){

        // Arrange
        let hash = hash(&[]);
        let owner_id = 0_u64;
        let points = GeoPoint::to_vec(&vec![GeoPoint::new(1.0, 2.0), GeoPoint::new(3.0, 4.0), GeoPoint::new(5.0, 6.0)]);

        // Act
        let object = Object::new("test object title", points.as_ref(), owner_id, false, &hash);

        // Assert
        assert_eq!(object.title(), "test object title");
        assert_eq!(object.points(), &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(object.owner_id(), owner_id);
        assert_eq!(object.history_hash(), &hash);

    }

}