use std::ops::Deref;

use byteorder::{ByteOrder, LittleEndian};

use exonum::messages::Field;
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Fork, MerklePatriciaTable, MapTable, MerkleTable};
use exonum::blockchain::View;

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
        const SIZE = 40;

        title:                  &str            [00 => 08]
        price_per_listen:       u32             [08 => 12]
        min_plays:              u32             [12 => 16]
        additional_conditions:  &str            [16 => 24]
        owners:                 &[u32]          [24 => 32]
        distributors:           &[u16]          [32 => 40]
    }
}

storage_value! {
    Ownership {
        const SIZE = 80;

        fingerprint:            &Hash           [00 => 32]
        plays:                  u64             [32 => 40]
        amount:                 u64             [40 => 48]
        reports_hash:           &Hash           [48 => 80]
    }
}

storage_value! {
    Contract {
        const SIZE = 80;

        fingerprint:            &Hash           [00 => 32]
        plays:                  u64             [32 => 40]
        amount:                 u64             [40 => 48]
        reports_hash:           &Hash           [48 => 80]
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

impl Content {
    pub fn set_distributors(&mut self, distributors: &[u16]) {
        Field::write(&distributors, &mut self.raw, 32, 40);
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
    pub fn distributor_contracts(&self,
                                 distributor_id: u16)
                                 -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Contract> {
        let mut prefix = vec![34; 3];
        LittleEndian::write_u16(&mut prefix[1..], distributor_id);
        MerkleTable::new(MapTable::new(prefix, &self))
    }
}


#[cfg(test)]
mod tests {
    use exonum::crypto::{gen_keypair, hash};
    use super::{Owner, Distributor, Content, Contract, Ownership};
    use super::super::txs::ContentShare;

    #[test]
    fn test_owner() {
        let p = gen_keypair().0;
        let s = "One";
        let h = hash(&[]);
        let owner = Owner::new(&p, s, &h);

        assert_eq!(owner.pub_key(), &p);
        assert_eq!(owner.name(), s);
        assert_eq!(owner.ownership_hash(), &h);
    }

    #[test]
    fn test_distributor() {
        let p = gen_keypair().0;
        let s = "Dist";
        let h = hash(&[]);
        let owner = Distributor::new(&p, s, &h);

        assert_eq!(owner.pub_key(), &p);
        assert_eq!(owner.name(), s);
        assert_eq!(owner.contracts_hash(), &h);
    }

    #[test]
    fn test_content() {
        let title = "Iron Maiden - Brave New World";
        let price_per_listen = 1;
        let min_plays = 100;
        let additional_conditions = "";
        let owners = [ContentShare::new(0, 15).into(), ContentShare::new(1, 85).into()];
        let distributors = [0, 1, 2, 3, 4, 5];

        let mut content = Content::new(title,
                                   price_per_listen,
                                   min_plays,
                                   additional_conditions,
                                   owners.as_ref(),
                                   distributors.as_ref());

        assert_eq!(content.title(), title);
        assert_eq!(content.price_per_listen(), price_per_listen);
        assert_eq!(content.min_plays(), min_plays);
        assert_eq!(content.additional_conditions(), additional_conditions);
        assert_eq!(content.owners(), owners.as_ref());
        assert_eq!(content.distributors(), distributors.as_ref());

        let distributors = [4, 3, 2];
        content.set_distributors(distributors.as_ref());
        assert_eq!(content.distributors(), distributors.as_ref());

        let distributors = [18, 19, 20, 1, 2, 4, 5, 6, 3, 4, 5, 6];
        content.set_distributors(distributors.as_ref());
        assert_eq!(content.distributors(), distributors.as_ref());
    }

    #[test]
    fn test_ownership() {
        let f = hash(&[]);
        let p = 10;
        let a = 1000;
        let r = hash(&[]);
        let ownership = Ownership::new(&f, p, a, &r);

        assert_eq!(ownership.fingerprint(), &f);
        assert_eq!(ownership.plays(), p);
        assert_eq!(ownership.amount(), a);
        assert_eq!(ownership.reports_hash(), &r);
    }

    #[test]
    fn test_contract() {
        let f = hash(&[]);
        let p = 10;
        let a = 1000;
        let r = hash(&[]);
        let contract = Contract::new(&f, p, a, &r);

        assert_eq!(contract.fingerprint(), &f);
        assert_eq!(contract.plays(), p);
        assert_eq!(contract.amount(), a);
        assert_eq!(contract.reports_hash(), &r);
    }
}
