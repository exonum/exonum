use exonum::crypto::{Hash, PublicKey};
use byteorder::{ByteOrder, LittleEndian};
use exonum::messages::{RawMessage, Message, Error as MessageError};

pub const TX_CREATE_OWNER_ID: u16 = 128;
pub const TX_CREATE_OBJECT_ID: u16 = 129;
pub const TX_MODIFY_OBJECT_ID: u16 = 130;
pub const TX_TRANSFER_OBJECT_ID: u16 = 131;
pub const TX_REMOVE_OBJECT_ID: u16 = 132;

pub struct Point {
    pub x: f32,
    pub y: f32,
}

pub struct MBR {
    pub left: Point,
    pub right: Point
}

impl Point {
    pub fn new(x: f32, y: f32) -> Point {
        Point { x: x, y: y }
    }
    pub fn in_mbr(&self, mbr: &MBR) -> bool {
        self.x >= mbr.left.x && self.x <= mbr.right.x && self.y >= mbr.left.y && self.y <= mbr.right.y
    }
}

impl MBR{
    pub fn new(left: Point, right: Point) -> MBR {
        MBR{
            left: left,
            right: right
        }
    }
}

impl PartialEq for Point{
    fn eq(&self, other: &Point) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl From<Point> for f64{
    fn from(point: Point) -> f64 {
        let mut v = vec![0; 8];
        LittleEndian::write_f32(&mut v[0..4], point.x);
        LittleEndian::write_f32(&mut v[4..8], point.y);
        let u = LittleEndian::read_f64(&v[0..8]);
        u
    }
}

impl From<f64> for Point {
    fn from(n: f64) -> Point {
        let mut v = vec![0; 8];
        LittleEndian::write_f64(&mut v[0..8], n);
        Point {
            x: LittleEndian::read_f32(&v[0..4]),
            y: LittleEndian::read_f32(&v[4..8]),
        }
    }
}

// impl From<Point> for Vec<f64>{
//     fn from(point: Point) -> Vec<f64> {
//         let mut v = vec![0; 16];
//         LittleEndian::write_f64(&mut v[0..8], point.x);
//         LittleEndian::write_f64(&mut v[8..16], point.y);
//         vec![LittleEndian::read_f64(&v[0..8]), LittleEndian::read_f64(&v[8..16])]
//     }
// }

// impl From<Vec<f64>> for Point {
//     fn from(n: Vec<f64>) -> Point {
//         let mut v = vec![0; 16];
//         LittleEndian::write_f64(&mut v[0..8], n[0]);
//         LittleEndian::write_f64(&mut v[8..16], n[1]);
//         Point {
//             x: LittleEndian::read_f64(&v[0..8]),
//             y: LittleEndian::read_f64(&v[8..16]),
//         }
//     }
// }

// impl Into<u64> for Point {
//     fn into(self) -> u64 {
//         let mut v = vec![0; 8];
//         LittleEndian::write_u32(&mut v[0..4], self.x);
//         LittleEndian::write_u32(&mut v[4..8], self.y);
//         let u = LittleEndian::read_u64(&v[0..8]);
//         u
//     }
// }

// impl Into<Point> for u64 {
//     fn into(self) -> Point {
//         let mut v = vec![0; 8];
//         LittleEndian::write_u64(&mut v[0..8], self);
//         Point {
//             x: LittleEndian::read_u32(&v[0..4]),
//             y: LittleEndian::read_u32(&v[4..8]),
//         }
//     }
// }

message! {
    TxCreateOwner {
        const ID = TX_CREATE_OWNER_ID;
        const SIZE = 96;
        pub_key:               &PublicKey      [00 => 32]
        firstname:             &str            [32 => 64]
        lastname:              &str            [64 => 96]
    }
}

message! {
    TxCreateObject {
        const ID = TX_CREATE_OBJECT_ID;
        const SIZE = 80;
        pub_key:               &PublicKey      [00 => 32]
        title:                 &str            [32 => 64]
        points:                &[f64]          [64 => 72]
        owner_id:              u64             [72 => 80]
    }
}

message! {
    TxModifyObject {
        const ID = TX_MODIFY_OBJECT_ID;
        const SIZE = 80;
        pub_key:               &PublicKey      [00 => 32]
        object_id:             u64             [32 => 40]
        title:                 &str            [40 => 72]
        points:                &[f64]          [72 => 80]
    }
}

message! {
    TxTransferObject {
        const ID = TX_TRANSFER_OBJECT_ID;
        const SIZE = 48;
        pub_key:               &PublicKey      [00 => 32]
        object_id:             u64             [32 => 40]
        owner_id:              u64             [40 => 48]
    }
}

message! {
    TxRemoveObject {
        const ID = TX_REMOVE_OBJECT_ID;
        const SIZE = 40;
        pub_key:               &PublicKey      [00 => 32]
        object_id:             u64             [32 => 40]
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum ObjectTx {
    CreateOwner(TxCreateOwner),
    CreateObject(TxCreateObject),
    ModifyObject(TxModifyObject),
    TransferObject(TxTransferObject),
    RemoveObject(TxRemoveObject),
}

impl ObjectTx {
    pub fn pub_key(&self) -> &PublicKey {
        match *self {
            ObjectTx::CreateOwner(ref msg) => msg.pub_key(),
            ObjectTx::CreateObject(ref msg) => msg.pub_key(),
            ObjectTx::ModifyObject(ref msg) => msg.pub_key(),
            ObjectTx::TransferObject(ref msg) => msg.pub_key(),
            ObjectTx::RemoveObject(ref msg) => msg.pub_key(),
        }
    }
}

impl Message for ObjectTx {
    fn raw(&self) -> &RawMessage {
        match *self {
            ObjectTx::CreateOwner(ref msg) => msg.raw(),
            ObjectTx::CreateObject(ref msg) => msg.raw(),
            ObjectTx::ModifyObject(ref msg) => msg.raw(),
            ObjectTx::TransferObject(ref msg) => msg.raw(),
            ObjectTx::RemoveObject(ref msg) => msg.raw(),
        }
    }
    fn from_raw(raw: RawMessage) -> Result<Self, MessageError> {
        Ok(match raw.message_type() {
            TX_CREATE_OWNER_ID => ObjectTx::CreateOwner(TxCreateOwner::from_raw(raw)?),
            TX_CREATE_OBJECT_ID => ObjectTx::CreateObject(TxCreateObject::from_raw(raw)?),
            TX_MODIFY_OBJECT_ID => ObjectTx::ModifyObject(TxModifyObject::from_raw(raw)?),
            TX_TRANSFER_OBJECT_ID => ObjectTx::TransferObject(TxTransferObject::from_raw(raw)?),
            TX_REMOVE_OBJECT_ID => ObjectTx::RemoveObject(TxRemoveObject::from_raw(raw)?),
            _ => panic!("Undefined message type"),
        })
    }

    fn hash(&self) -> Hash {
        match *self {
            ObjectTx::CreateOwner(ref msg) => msg.hash(),
            ObjectTx::CreateObject(ref msg) => msg.hash(),
            ObjectTx::ModifyObject(ref msg) => msg.hash(),
            ObjectTx::TransferObject(ref msg) => msg.hash(),
            ObjectTx::RemoveObject(ref msg) => msg.hash(),
        }
    }

    fn verify(&self, pub_key: &PublicKey) -> bool {
        match *self {
            ObjectTx::CreateOwner(ref msg) => msg.verify(pub_key),
            ObjectTx::CreateObject(ref msg) => msg.verify(pub_key),
            ObjectTx::ModifyObject(ref msg) => msg.verify(pub_key),
            ObjectTx::TransferObject(ref msg) => msg.verify(pub_key),
            ObjectTx::RemoveObject(ref msg) => msg.verify(pub_key),
        }
    }
}

#[cfg(test)]
mod tests {

    use exonum::crypto::gen_keypair;
    use super::{TxCreateOwner, TxCreateObject, TxModifyObject, TxTransferObject, TxRemoveObject,
                Point};
    use exonum::messages::Message;
    use byteorder::{ByteOrder, LittleEndian};

    #[test]
    fn test_f64(){
        let mut v = vec![0; 8];
        LittleEndian::write_f64(&mut v[0..8], 41.85123450747319_f64);
        println!("v={:?}", v);
        let vv = LittleEndian::read_f64(&v[0..8]);
        println!("vv={:?}", vv);
    }
    #[test]
    fn test_tx_create_owner() {
        // Arrange
        let (p, s) = gen_keypair();
        // Act
        let tx = TxCreateOwner::new(&p, "firstname", "lastname", &s);
        // Assert
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.firstname(), "firstname");
        assert_eq!(tx.lastname(), "lastname");
    }

    #[test]
    fn test_point() {
        // Arrange
        let p = Point::new(1.0, 2.0);
        // Act
        let u: f64 = p.into();
        let point: Point = u.into();
        // Assert
        //assert_eq!(u, 0x0000000200000001);
        assert_eq!(point.x, 1.0);
        assert_eq!(point.y, 2.0);
    }

    #[test]
    fn test_tx_create_object() {
        // Arrange
        let (p, s) = gen_keypair();
        let owner_id = 5_u64;
        let title = "test object title";
        let points = [Point::new(1.0, 2.0).into(), Point::new(3.0, 4.0).into()];
        // Act
        let tx = TxCreateObject::new(&p, title, &points, owner_id, &s);
        // Assert
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.title(), "test object title");
        //assert_eq!(tx.points(), &[0x0000000200000001, 0x0000000400000003]);
        assert_eq!(tx.owner_id(), owner_id);
        // Act
        let tx2 = TxCreateObject::from_raw(tx.raw().clone()).unwrap();
        // Assert
        assert_eq!(tx2.pub_key(), &p);
        assert_eq!(tx2.title(), "test object title");
        //assert_eq!(tx2.points(), &[0x0000000200000001, 0x0000000400000003]);
        assert_eq!(tx2.owner_id(), owner_id);
    }

    #[test]
    fn test_tx_modify_object() {
        // Arrange
        let (p, s) = gen_keypair();
        let object_id = 1_u64;
        let title = "test object title";
        let points = [Point::new(1.0, 2.0).into(), Point::new(3.0, 4.0).into()];
        // Act
        let tx = TxModifyObject::new(&p, object_id, title, &points, &s);
        // Assert
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.object_id(), 1_u64);
        assert_eq!(tx.title(), "test object title");
        //assert_eq!(tx.points(), &[0x0000000200000001, 0x0000000400000003]);
        // Act
        let tx2 = TxModifyObject::from_raw(tx.raw().clone()).unwrap();
        // Assert
        assert_eq!(tx2.pub_key(), &p);
        assert_eq!(tx2.object_id(), 1_u64);
        assert_eq!(tx2.title(), "test object title");
        //assert_eq!(tx2.points(), &[0x0000000200000001, 0x0000000400000003]);
    }

    #[test]
    fn test_tx_transfer_object() {
        // Arrange
        let (p, s) = gen_keypair();
        let object_id = 1_u64;
        let owner_id = 1_u64;
        let (owner_pub_key, _) = gen_keypair();
        // Act
        let tx = TxTransferObject::new(&p, object_id, owner_id, &s);
        // Assert
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.object_id(), 1_u64);
        assert_eq!(tx.owner_id(), owner_id);
        // Act
        let tx2 = TxTransferObject::from_raw(tx.raw().clone()).unwrap();
        // Assert
        assert_eq!(tx2.pub_key(), &p);
        assert_eq!(tx2.object_id(), 1_u64);
        assert_eq!(tx2.owner_id(), owner_id);
    }

    #[test]
    fn test_tx_remove_object() {
        // Arrange
        let (p, s) = gen_keypair();
        let object_id = 1_u64;
        // Act
        let tx = TxRemoveObject::new(&p, object_id, &s);
        // Assert
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.object_id(), 1_u64);
        // Act
        let tx2 = TxRemoveObject::from_raw(tx.raw().clone()).unwrap();
        // Assert
        assert_eq!(tx2.pub_key(), &p);
        assert_eq!(tx2.object_id(), 1_u64);
    }

}
