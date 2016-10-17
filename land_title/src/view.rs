use exonum::messages::Field;
use exonum::blockchain::{View};
use exonum::crypto::{ PublicKey, Hash, hash };
use byteorder::{ByteOrder, LittleEndian};
use exonum::storage::{Fork, MerklePatriciaTable, MapTable, MerkleTable, List, Result as StorageResult};
use txs::{ObjectTx, Point, MBR};
use std::ops::Deref;


pub type ObjectId = u64;

storage_value! {
    Owner {
        const SIZE = 64;
        name:                  &str            [00 => 32]
        ownership_hash:        &Hash           [32 => 64]
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
    Object {
        const SIZE = 137;
        pub_key:               &PublicKey      [00  => 32]
        title:                 &str            [32  => 64]
        points:                &[u64]          [64  => 72]
        owner_pub_key:         &PublicKey      [72  => 104]
        deleted:               bool            [104 => 105]
        history_hash:          &Hash           [105 => 137]
    }
}

impl Owner {
    pub fn set_name(&mut self, name: &str){
        Field::write(&name, &mut self.raw, 00, 32);
    }
    pub fn set_ownership_hash(&mut self, hash: &Hash) {
        Field::write(&hash, &mut self.raw, 32, 64);
    }
}

impl Object {
    pub fn set_title(&mut self, title: &str) {
        Field::write(&title, &mut self.raw, 32, 64);
    }
    pub fn set_owner(&mut self, owner_pub_key: &PublicKey) {
        Field::write(&owner_pub_key, &mut self.raw, 72, 104);
    }
    pub fn set_history_hash(&mut self, hash: &Hash) {
        Field::write(&hash, &mut self.raw, 105, 137);
    }
    pub fn set_points(&mut self, points: &[u64]) {
        Field::write(&points, &mut self.raw, 64, 72);
    }
    pub fn set_deleted(&mut self, deleted: bool) {
        Field::write(&deleted, &mut self.raw, 104, 105);
    }
    pub fn transfer_to(&mut self, other: &PublicKey) {
        self.set_owner(other);
    }
    pub fn in_mbr(&self, mbr: &MBR) -> bool {
        for point_data in self.points() {
            let point: Point = (*point_data).into();
            if point.in_mbr(mbr) {
                return true;
            }
        }
        false
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
    pub fn  owners(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, PublicKey, Owner> {
        MerklePatriciaTable::new(MapTable::new(vec![50], &self))
    }
    pub fn  objects(&self) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Object> {
        MerkleTable::new(MapTable::new(vec![51], &self))
    }
    pub fn owner_objects(&self, owner_pub_key: &PublicKey) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u16, Ownership> {
        let mut prefix = vec![52; 33];
        prefix[1..].copy_from_slice(owner_pub_key.as_ref());
        MerkleTable::new(MapTable::new(prefix, &self))
    }
    pub fn object_history(&self, object_id: u64) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Hash> {
        let mut prefix = vec![52; 9];
        LittleEndian::write_u64(&mut prefix[1..], object_id);
        MerkleTable::new(MapTable::new(prefix, &self))
    }
    pub fn find_objects_for_owner(&self, owner_pub_key: &PublicKey) -> StorageResult<Vec<(ObjectId, Object)>> {
        let mut v = Vec::new();
        for ownership in self.owner_objects(owner_pub_key).values()? {
            if let Some(object) = self.objects().get(ownership.object_id())? {
                if object.owner_pub_key() == owner_pub_key && ownership.operation() && !object.deleted() {
                    v.push((ownership.object_id(), object));
                }
            }
        }
        Ok(v)

    }
    pub fn find_objects_in_mbr(&self, mbr: &MBR) -> StorageResult<Option<(u16, Object)>> {
        let objects = self.objects();
        let values = objects.values()?;
        let r = values.into_iter()
            .enumerate()
            .find(|&(_, ref c)| c.in_mbr(mbr))
            .map(|(x, y)| (x as u16, y));
        Ok(r)
    }
}

#[cfg(test)]
mod tests {

    use exonum::crypto::{gen_keypair, hash};
    use super::{Owner, Object, Ownership};
    use txs::{Point,MBR};

    #[test]
    fn test_create_owner() {
        // Arrange
        let hash = hash(&[]);
        // Act
        let owner = Owner::new("test owner name", &hash);
        // Assert
        assert_eq!(owner.name(), "test owner name");
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
        let (p, _) = gen_keypair();
        let (p1, _) = gen_keypair();

        let points = [Point::new(1, 2).into(), Point::new(3, 4).into(), Point::new(5, 6).into()];

        // Act
        let object = Object::new(&p, "test object title", points.as_ref(), &p1, false, &hash);

        // Assert
        assert_eq!(object.pub_key(), &p);
        assert_eq!(object.title(), "test object title");
        assert_eq!(object.points(), [Point::new(1,2).into(), Point::new(3,4).into(), Point::new(5,6).into()]);
        assert_eq!(object.owner_pub_key(), &p1);
        assert_eq!(object.history_hash(), &hash);
        assert_eq!(object.in_mbr(&MBR::new(Point::new(0, 0), Point::new(1, 1))), false);
        assert_eq!(object.in_mbr(&MBR::new(Point::new(0, 0), Point::new(2, 2))), true);
        assert_eq!(object.in_mbr(&MBR::new(Point::new(2, 2), Point::new(3, 3))), false);
        assert_eq!(object.in_mbr(&MBR::new(Point::new(2, 2), Point::new(4, 4))), true);

    }

}