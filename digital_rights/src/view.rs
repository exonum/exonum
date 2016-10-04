use std::ops::Deref;

use byteorder::{ByteOrder, LittleEndian};

use exonum::messages::Field;
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Map, Database, Fork, Error, MerklePatriciaTable, MapTable, MerkleTable};
use exonum::blockchain::{Blockchain, View};

use super::DigitalRightsTx;

storage_value! {
    Owner {
        const SIZE = 80;

        pub_key:            &PublicKey  [00 => 32]
        name:               &str        [32 => 40]
        ownership_hash:     &Hash       [40 => 72]
    }
}

storage_value! {
    Distributor {
        const SIZE = 80;

        pub_key:            &PublicKey  [00 => 32]
        name:               &str        [32 => 40]
        contracts_hash:     &Hash       [40 => 72]
    }
}

storage_value! {
    Content {
        const SIZE = 32;

        title:                  &str            [00 => 08]
        price_per_listen:       u32             [08 => 12]
        min_plays:              u32             [12 => 16]
        additional_conditions:  &str            [16 => 24]
        //owners:                 &[u32]          [24 => 32]
    }
}

storage_value! {
    Ownership {
        const SIZE = 32;

        fingerprint:            &Hash           [0 => 32]
    }
}

storage_value! {
    Contract {
        const SIZE = 32;

        fingerprint:            &Hash           [0 => 32]
    }
}

impl Owner {
    pub fn set_ownership_hash(&mut self, hash: &Hash) {
        hash.write(&mut self.raw, 40, 72)
    }
}

impl Distributor {
    pub fn set_contracts_hash(&mut self, hash: &Hash) {
        hash.write(&mut self.raw, 40, 72)
    }
}

pub struct DigitalRightsView<F: Fork> {
    pub fork: F,
}

impl<F> View<F> for DigitalRightsView<F>
    where F: Fork
{
    type Transaction = DigitalRightsTx;

    fn from_fork(fork: F) -> Self {
        DigitalRightsView { fork: fork }
    }
}

impl<F> Deref for DigitalRightsView<F>
    where F: Fork
{
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.fork
    }
}

impl<F> DigitalRightsView<F>
    where F: Fork
{
    pub fn owners(&self) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Owner> {
        MerkleTable::new(MapTable::new(vec![30], &self))
    }
    pub fn distributors(&self) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Distributor> {
        MerkleTable::new(MapTable::new(vec![31], &self))
    }
    pub fn contents(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, Hash, Content> {
        MerklePatriciaTable::new(MapTable::new(vec![32], &self))
    }
    pub fn owner_contents(&self,
                          owner_id: u16)
                          -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Ownership> {
        let mut prefix = vec![33; 3];
        LittleEndian::write_u16(&mut prefix[1..], owner_id);
        MerkleTable::new(MapTable::new(prefix, &self))
    }
}

// TODO test dto macro!
