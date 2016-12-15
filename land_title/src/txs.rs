use exonum::crypto::{Hash, PublicKey};
use exonum::messages::{RawMessage, Message, Error as MessageError};
use geo::{Point, LineString, Polygon};
use time;

pub const TX_REGISTER: u16 = 128;
pub const TX_CREATE_OWNER_ID: u16 = 129;
pub const TX_CREATE_OBJECT_ID: u16 = 130;
pub const TX_MODIFY_OBJECT_ID: u16 = 131;
pub const TX_TRANSFER_OBJECT_ID: u16 = 132;
pub const TX_REMOVE_OBJECT_ID: u16 = 133;
pub const TX_RESTORE_OBJECT_ID: u16 = 134;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeoPoint {
    pub x: f64,
    pub y: f64,
}

impl GeoPoint {
    pub fn new(x: f64, y: f64) -> GeoPoint {
        GeoPoint { x: x, y: y }
    }
    pub fn from_vec(v: Vec<f64>) -> Vec<GeoPoint> {
        assert!(v.len() % 2 == 0);
        let x_coords = v.iter().enumerate().filter(|&(i, _)| i % 2 == 0).map(|(_, v)| *v);
        let y_coords = v.iter().enumerate().filter(|&(i, _)| i % 2 != 0).map(|(_, v)| *v);
        x_coords.zip(y_coords).map(|(x, y)| GeoPoint::new(x, y)).collect::<Vec<GeoPoint>>()
    }
    pub fn to_vec(points: &Vec<GeoPoint>) -> Vec<f64> {
        let mut result = vec![];
        for point in points {
            result.push(point.x);
            result.push(point.y);
        }
        result
    }
    pub fn to_polygon(geopoints: Vec<GeoPoint>) -> Polygon<f64> {
        let mut points = geopoints.clone();
        let start_point = points[0].clone();
        points.push(start_point);
        let v = Vec::new();
        Polygon::new(LineString(points.iter()
                         .map(|item| Point::new(item.x, item.y))
                         .collect::<Vec<Point<f64>>>()),
                     v)
    }
}


impl PartialEq for GeoPoint {
    fn eq(&self, other: &GeoPoint) -> bool {
        self.x == other.x && self.y == other.y
    }
}

message! {
    TxRegister {
        const ID = TX_REGISTER;
        const SIZE = 64;
        pub_key:               &PublicKey      [00 => 32]
        name:                  &str            [32 => 64]
    }
}

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
        const SIZE = 88;
        pub_key:               &PublicKey      [00 => 32]
        object_id:             u64             [32 => 40]
        title:                 &str            [40 => 72]
        points:                &[f64]          [72 => 80]
        created_at:            u64             [80 => 88]
    }
}

message! {
    TxTransferObject {
        const ID = TX_TRANSFER_OBJECT_ID;
        const SIZE = 56;
        pub_key:               &PublicKey      [00 => 32]
        object_id:             u64             [32 => 40]
        owner_id:              u64             [40 => 48]
        created_at:            u64             [48 => 56]
    }
}

message! {
    TxRemoveObject {
        const ID = TX_REMOVE_OBJECT_ID;
        const SIZE = 48;
        pub_key:               &PublicKey      [00 => 32]
        object_id:             u64             [32 => 40]
        created_at:            u64             [40 => 48]
    }
}

message! {
    TxRestoreObject {
        const ID = TX_RESTORE_OBJECT_ID;
        const SIZE = 48;
        pub_key:               &PublicKey      [00 => 32]
        object_id:             u64             [32 => 40]
        created_at:            u64             [40 => 48]
    }
}

pub fn timestamp() -> u64 {
    let timespec = time::get_time();
    timespec.sec as u64
}

#[derive(PartialEq, Debug, Clone)]
pub enum ObjectTx {
    Register(TxRegister),
    CreateOwner(TxCreateOwner),
    CreateObject(TxCreateObject),
    ModifyObject(TxModifyObject),
    TransferObject(TxTransferObject),
    RemoveObject(TxRemoveObject),
    RestoreObject(TxRestoreObject),
}

impl ObjectTx {
    pub fn pub_key(&self) -> &PublicKey {
        match *self {
            ObjectTx::Register(ref msg) => msg.pub_key(),
            ObjectTx::CreateOwner(ref msg) => msg.pub_key(),
            ObjectTx::CreateObject(ref msg) => msg.pub_key(),
            ObjectTx::ModifyObject(ref msg) => msg.pub_key(),
            ObjectTx::TransferObject(ref msg) => msg.pub_key(),
            ObjectTx::RemoveObject(ref msg) => msg.pub_key(),
            ObjectTx::RestoreObject(ref msg) => msg.pub_key(),
        }
    }
}

impl Message for ObjectTx {
    fn raw(&self) -> &RawMessage {
        match *self {
            ObjectTx::Register(ref msg) => msg.raw(),
            ObjectTx::CreateOwner(ref msg) => msg.raw(),
            ObjectTx::CreateObject(ref msg) => msg.raw(),
            ObjectTx::ModifyObject(ref msg) => msg.raw(),
            ObjectTx::TransferObject(ref msg) => msg.raw(),
            ObjectTx::RemoveObject(ref msg) => msg.raw(),
            ObjectTx::RestoreObject(ref msg) => msg.raw(),
        }
    }
    fn from_raw(raw: RawMessage) -> Result<Self, MessageError> {
        Ok(match raw.message_type() {
            TX_REGISTER => ObjectTx::Register(TxRegister::from_raw(raw)?),
            TX_CREATE_OWNER_ID => ObjectTx::CreateOwner(TxCreateOwner::from_raw(raw)?),
            TX_CREATE_OBJECT_ID => ObjectTx::CreateObject(TxCreateObject::from_raw(raw)?),
            TX_MODIFY_OBJECT_ID => ObjectTx::ModifyObject(TxModifyObject::from_raw(raw)?),
            TX_TRANSFER_OBJECT_ID => ObjectTx::TransferObject(TxTransferObject::from_raw(raw)?),
            TX_REMOVE_OBJECT_ID => ObjectTx::RemoveObject(TxRemoveObject::from_raw(raw)?),
            TX_RESTORE_OBJECT_ID => ObjectTx::RestoreObject(TxRestoreObject::from_raw(raw)?),
            _ => panic!("Undefined message type"),
        })
    }

    fn hash(&self) -> Hash {
        match *self {
            ObjectTx::Register(ref msg) => msg.hash(),
            ObjectTx::CreateOwner(ref msg) => msg.hash(),
            ObjectTx::CreateObject(ref msg) => msg.hash(),
            ObjectTx::ModifyObject(ref msg) => msg.hash(),
            ObjectTx::TransferObject(ref msg) => msg.hash(),
            ObjectTx::RemoveObject(ref msg) => msg.hash(),
            ObjectTx::RestoreObject(ref msg) => msg.hash(),
        }
    }

    fn verify(&self, pub_key: &PublicKey) -> bool {
        match *self {
            ObjectTx::Register(ref msg) => msg.verify(pub_key),
            ObjectTx::CreateOwner(ref msg) => msg.verify(pub_key),
            ObjectTx::CreateObject(ref msg) => msg.verify(pub_key),
            ObjectTx::ModifyObject(ref msg) => msg.verify(pub_key),
            ObjectTx::TransferObject(ref msg) => msg.verify(pub_key),
            ObjectTx::RemoveObject(ref msg) => msg.verify(pub_key),
            ObjectTx::RestoreObject(ref msg) => msg.verify(pub_key),
        }
    }
}

#[cfg(test)]
mod tests {

    use exonum::crypto::gen_keypair;
    use super::{TxCreateOwner, TxCreateObject, TxModifyObject, TxTransferObject, TxRemoveObject,
                TxRegister, GeoPoint, timestamp};
    use exonum::messages::Message;

    #[test]
    fn test_register() {
        // Arrange
        let (p, s) = gen_keypair();

        // Act
        let tx = TxRegister::new(&p, "Test user", &s);

        // Assert
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.name(), "Test user");

        // Act
        let tx2 = TxRegister::from_raw(tx.raw().clone()).unwrap();
        // Assert
        assert_eq!(tx2.pub_key(), &p);
        assert_eq!(tx2.name(), "Test user");
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

        // Act
        let tx2 = TxCreateOwner::from_raw(tx.raw().clone()).unwrap();
        // Assert
        assert_eq!(tx2.firstname(), "firstname");
        assert_eq!(tx2.lastname(), "lastname");
    }

    #[test]
    fn test_point() {

        // Arrange
        let u = vec![1.0_f64, 2.0_f64, 3.0_f64, 4.0_f64, 5.0_f64, 6.0_f64];

        // Act
        let points = GeoPoint::from_vec(u);
        // Assert
        assert_eq!(points,
                   [GeoPoint::new(1.0, 2.0), GeoPoint::new(3.0, 4.0), GeoPoint::new(5.0, 6.0)]);

        // Act
        let v = GeoPoint::to_vec(&points);
        // Assert
        assert_eq!(v, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);

    }


    #[test]
    fn test_tx_create_object() {
        // Arrange
        let (p, s) = gen_keypair();
        let owner_id = 5_u64;
        let title = "test object title";
        let points = GeoPoint::to_vec(&vec![GeoPoint::new(1.0, 2.0), GeoPoint::new(3.0, 4.0)]);
        // Act
        let tx = TxCreateObject::new(&p, title, &points, owner_id, &s);
        // Assert
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.title(), "test object title");
        assert_eq!(tx.points(), &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(tx.owner_id(), owner_id);
        // Act
        let tx2 = TxCreateObject::from_raw(tx.raw().clone()).unwrap();
        // Assert
        assert_eq!(tx2.pub_key(), &p);
        assert_eq!(tx2.title(), "test object title");
        assert_eq!(tx2.points(), &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(tx2.owner_id(), owner_id);
    }

    #[test]
    fn test_tx_modify_object() {
        // Arrange
        let (p, s) = gen_keypair();
        let object_id = 1_u64;
        let title = "test object title";
        let points = GeoPoint::to_vec(&vec![GeoPoint::new(1.0, 2.0), GeoPoint::new(3.0, 4.0)]);
        let created_at = timestamp();
        // Act
        let tx = TxModifyObject::new(&p, object_id, title, &points, created_at, &s);
        // Assert
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.object_id(), 1_u64);
        assert_eq!(tx.title(), "test object title");
        assert_eq!(tx.points(), &[1.0, 2.0, 3.0, 4.0]);
        // Act
        let tx2 = TxModifyObject::from_raw(tx.raw().clone()).unwrap();
        // Assert
        assert_eq!(tx2.pub_key(), &p);
        assert_eq!(tx2.object_id(), 1_u64);
        assert_eq!(tx2.title(), "test object title");
        assert_eq!(tx2.points(), &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_tx_transfer_object() {
        // Arrange
        let (p, s) = gen_keypair();
        let object_id = 1_u64;
        let owner_id = 1_u64;
        let created_at = timestamp();
        // Act
        let tx = TxTransferObject::new(&p, object_id, owner_id, created_at, &s);
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
        let created_at = timestamp();
        // Act
        let tx = TxRemoveObject::new(&p, object_id, created_at, &s);
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
