use std::convert::Into;

use time::Timespec;
use byteorder::{ByteOrder, LittleEndian};

use exonum::messages::{RawMessage, Message, Error as MessageError};
use exonum::crypto::{PublicKey, Hash};

use super::{Fingerprint, Uuid};

pub const TX_CREATE_OWNER_ID: u16 = 128;
pub const TX_CREATE_DITRIBUTOR_ID: u16 = 129;
pub const TX_ADD_CONTENT: u16 = 130;
pub const TX_ADD_CONTRACT: u16 = 131;
pub const TX_REPORT: u16 = 132;

message! {
    TxCreateOwner {
        const ID = TX_CREATE_OWNER_ID;
        const SIZE = 40;

        pub_key:                &PublicKey      [00 => 32]
        name:                   &str            [32 => 40]
    }
}

message! {
    TxCreateDistributor {
        const ID = TX_CREATE_DITRIBUTOR_ID;
        const SIZE = 40;

        pub_key:                &PublicKey      [00 => 32]
        name:                   &str            [32 => 40]
    }
}

message! {
    TxAddContent {
        const ID = TX_ADD_CONTENT;
        const SIZE = 96;

        pub_key:                &PublicKey      [00 => 32]
        fingerprint:            &Fingerprint    [32 => 64]
        title:                  &str            [64 => 72]
        price_per_listen:       u32             [72 => 76]
        min_plays:              u32             [76 => 80]
        owners:                 &[u32]          [80 => 88]
        additional_conditions:  &str            [88 => 96]
    }
}

message! {
    TxAddContract {
        const ID = TX_ADD_CONTRACT;
        const SIZE = 66;

        pub_key:                &PublicKey      [00 => 32]
        distributor_id:         u16             [32 => 34]
        fingerprint:            &Fingerprint    [34 => 66]
    }
}

message! {
    TxReport {
        const ID = TX_REPORT;
        const SIZE = 106;

        pub_key:                &PublicKey      [00 => 32]
        uuid:                   &Uuid           [32 => 64]
        distributor_id:         u16             [64 => 66]
        fingerprint:            &Hash           [66 => 98]
        time:                   Timespec        [98 => 106]
    }
}

impl TxAddContent {
    pub fn owner_shares(&self) -> Vec<ContentShare> {
        self.owners()
            .iter()
            .cloned()
            .map(|x| -> ContentShare { x.into() })
            .collect()
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum DigitalRightsTx {
    CreateOwner(TxCreateOwner),
    CreateDistributor(TxCreateDistributor),
    AddContent(TxAddContent),
    AddContract(TxAddContract),
    Report(TxReport),
}

impl Message for DigitalRightsTx {
    fn raw(&self) -> &RawMessage {
        match *self {
            DigitalRightsTx::CreateDistributor(ref msg) => msg.raw(),
            DigitalRightsTx::CreateOwner(ref msg) => msg.raw(),
            DigitalRightsTx::AddContent(ref msg) => msg.raw(),
            DigitalRightsTx::AddContract(ref msg) => msg.raw(),
            DigitalRightsTx::Report(ref msg) => msg.raw(),            
        }
    }

    fn from_raw(raw: RawMessage) -> Result<Self, MessageError> {
        Ok(match raw.message_type() {
            TX_CREATE_DITRIBUTOR_ID => {
                DigitalRightsTx::CreateDistributor(TxCreateDistributor::from_raw(raw)?)
            }
            TX_CREATE_OWNER_ID => DigitalRightsTx::CreateOwner(TxCreateOwner::from_raw(raw)?),
            TX_ADD_CONTENT => DigitalRightsTx::AddContent(TxAddContent::from_raw(raw)?),
            TX_ADD_CONTRACT => DigitalRightsTx::AddContract(TxAddContract::from_raw(raw)?),
            TX_REPORT => DigitalRightsTx::Report(TxReport::from_raw(raw)?),
            _ => panic!("Undefined message type"),
        })
    }

    fn hash(&self) -> Hash {
        match *self {
            DigitalRightsTx::CreateDistributor(ref msg) => msg.hash(),
            DigitalRightsTx::CreateOwner(ref msg) => msg.hash(),
            DigitalRightsTx::AddContent(ref msg) => msg.hash(),
            DigitalRightsTx::AddContract(ref msg) => msg.hash(),
            DigitalRightsTx::Report(ref msg) => msg.hash(),
        }
    }

    fn verify(&self, pub_key: &PublicKey) -> bool {
        match *self {
            DigitalRightsTx::CreateDistributor(ref msg) => msg.verify(pub_key),
            DigitalRightsTx::CreateOwner(ref msg) => msg.verify(pub_key),
            DigitalRightsTx::AddContent(ref msg) => msg.verify(pub_key),
            DigitalRightsTx::AddContract(ref msg) => msg.verify(pub_key),
            DigitalRightsTx::Report(ref msg) => msg.verify(pub_key),
        }
    }
}

impl DigitalRightsTx {
    pub fn pub_key(&self) -> &PublicKey {
        match *self {
            DigitalRightsTx::CreateDistributor(ref msg) => msg.pub_key(),
            DigitalRightsTx::CreateOwner(ref msg) => msg.pub_key(),
            DigitalRightsTx::AddContent(ref msg) => msg.pub_key(),
            DigitalRightsTx::AddContract(ref msg) => msg.pub_key(),
            DigitalRightsTx::Report(ref msg) => msg.pub_key(),
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct ContentShare {
    pub owner_id: u16,
    pub share: u16,
}

impl ContentShare {
    pub fn new(owner_id: u16, share: u16) -> ContentShare {
        debug_assert!(share <= 100);
        ContentShare {
            owner_id: owner_id,
            share: share,
        }
    }
}

impl Into<u32> for ContentShare {
    fn into(self) -> u32 {
        let mut v = vec![0; 4];
        LittleEndian::write_u16(&mut v[0..2], self.owner_id);
        LittleEndian::write_u16(&mut v[2..4], self.share);
        let u = LittleEndian::read_u32(&v[0..4]);
        u
    }
}

impl Into<ContentShare> for u32 {
    fn into(self) -> ContentShare {
        let mut v = vec![0; 4];
        LittleEndian::write_u32(&mut v[0..4], self);
        ContentShare {
            owner_id: LittleEndian::read_u16(&v[0..2]),
            share: LittleEndian::read_u16(&v[2..4]),
        }
    }
}

#[cfg(test)]
mod tests {
    use time;

    use exonum::crypto::{gen_keypair, hash};

    use super::{TxCreateOwner, TxCreateDistributor, TxAddContent, TxAddContract, TxReport,
                ContentShare};

    #[test]
    fn test_content_share() {
        let c1 = ContentShare::new(1, 50);
        let u: u32 = c1.into();
        let c2: ContentShare = u.into();

        assert_eq!(c2.owner_id, 1);
        assert_eq!(c2.share, 50);
    }

    #[test]
    fn test_tx_create_owner() {
        let (p, s) = gen_keypair();
        let tx = TxCreateOwner::new(&p, "Vasya", &s);
        assert_eq!(tx.name(), "Vasya");
        assert_eq!(tx.pub_key(), &p);
    }

    #[test]
    fn test_tx_create_distributor() {
        let (p, s) = gen_keypair();
        let tx = TxCreateDistributor::new(&p, "Vasya", &s);
        assert_eq!(tx.name(), "Vasya");
        assert_eq!(tx.pub_key(), &p);
    }

    #[test]
    fn test_tx_add_content() {
        let (p, s) = gen_keypair();
        let fingerprint = hash(&[]);
        let title = "Unknown artist - track 1";
        let price_per_listen = 1;
        let min_plays = 100;
        let additional_conditions = "Give me your money";
        let owners = [ContentShare::new(0, 15).into(), ContentShare::new(1, 85).into()];

        let tx = TxAddContent::new(&p,
                                   &fingerprint,
                                   title,
                                   price_per_listen,
                                   min_plays,
                                   &owners,
                                   additional_conditions,
                                   &s);

        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.fingerprint(), &fingerprint);
        assert_eq!(tx.title(), title);
        assert_eq!(tx.price_per_listen(), price_per_listen);
        assert_eq!(tx.min_plays(), min_plays);
        assert_eq!(tx.additional_conditions(), additional_conditions);
        assert_eq!(tx.owners(), &owners);
    }

    #[test]
    fn test_tx_add_contract() {
        let (p, s) = gen_keypair();
        let fingerprint = hash(&[]);
        let distributor = 1000;

        let tx = TxAddContract::new(&p, distributor, &fingerprint, &s);
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.fingerprint(), &fingerprint);
        assert_eq!(tx.distributor_id(), distributor);
    }

    #[test]
    fn test_tx_report() {
        let (p, s) = gen_keypair();
        let fingerprint = hash(&[]);
        let distributor = 1000;
        let uuid = hash(&[]);
        let ts = time::get_time();

        let tx = TxReport::new(&p, &uuid, distributor, &fingerprint, ts, &s);
        assert_eq!(tx.pub_key(), &p);
        assert_eq!(tx.uuid(), &uuid);
        assert_eq!(tx.fingerprint(), &fingerprint);
        assert_eq!(tx.distributor_id(), distributor);
        assert_eq!(tx.time(), ts);
    }
}
