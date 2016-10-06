use std::ops::Deref;

use time::Timespec;
use byteorder::{ByteOrder, LittleEndian};

use exonum::messages::Field;
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Map, Fork, MerklePatriciaTable, MapTable, MerkleTable, ListTable,
                      Result as StorageResult};
use exonum::blockchain::View;

use super::{DigitalRightsTx, Uuid, Fingerprint, ContentShare};

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
        const SIZE = 48;

        title:                  &str            [00 => 08]
        price_per_listen:       u64             [08 => 16]
        min_plays:              u64             [16 => 24]
        additional_conditions:  &str            [24 => 32]
        owners:                 &[u32]          [32 => 40]
        distributors:           &[u16]          [40 => 48]
    }
}

storage_value! {
    Ownership {
        const SIZE = 80;

        fingerprint:            &Fingerprint    [00 => 32]
        plays:                  u64             [32 => 40]
        amount:                 u64             [40 => 48]
        reports_hash:           &Hash           [48 => 80]
    }
}

storage_value! {
    Contract {
        const SIZE = 80;

        fingerprint:            &Fingerprint    [00 => 32]
        plays:                  u64             [32 => 40]
        amount:                 u64             [40 => 48]
        reports_hash:           &Hash           [48 => 80]
    }
}

storage_value! {
    Report {
        const SIZE = 66;

        distributor_id:         u16             [00 => 02]
        fingerprint:            &Fingerprint    [02 => 34]
        time:                   Timespec        [34 => 42]
        plays:                  u64             [42 => 50]
        amount:                 u64             [50 => 58]
        comment:                &str            [58 => 66]
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
        Field::write(&distributors, &mut self.raw, 40, 48);
    }

    pub fn shares(&self) -> Vec<ContentShare> {
        self.owners()
            .iter()
            .cloned()
            .map(|x| -> ContentShare { x.into() })
            .collect()
    }
}

impl Ownership {
    pub fn add_plays(&mut self, plays: u64) {
        let new_plays = self.plays() + plays;
        Field::write(&new_plays, &mut self.raw, 32, 40);
    }
    pub fn add_amount(&mut self, amount: u64) {
        let new_amount = self.amount() + amount;
        Field::write(&new_amount, &mut self.raw, 40, 48);
    }

    pub fn set_reports_hash(&mut self, hash: &Hash) {
        hash.write(&mut self.raw, 48, 80)
    }
}

impl Contract {
    pub fn add_plays(&mut self, plays: u64) {
        let new_plays = self.plays() + plays;
        Field::write(&new_plays, &mut self.raw, 32, 40);
    }
    pub fn add_amount(&mut self, amount: u64) {
        let new_amount = self.amount() + amount;
        Field::write(&new_amount, &mut self.raw, 40, 48);
    }

    pub fn set_reports_hash(&mut self, hash: &Hash) {
        hash.write(&mut self.raw, 48, 80)
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
    pub fn contents(&self)
                    -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, Fingerprint, Content> {
        MerklePatriciaTable::new(MapTable::new(vec![32], &self))
    }
    pub fn reports(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, Uuid, Report> {
        MerklePatriciaTable::new(MapTable::new(vec![33], &self))
    }

    pub fn owner_contents(&self,
                          owner_id: u16)
                          -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Ownership> {
        let mut prefix = vec![34; 3];
        LittleEndian::write_u16(&mut prefix[1..], owner_id);
        MerkleTable::new(MapTable::new(prefix, &self))
    }
    pub fn distributor_contracts(&self,
                                 distributor_id: u16)
                                 -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Contract> {
        let mut prefix = vec![35; 3];
        LittleEndian::write_u16(&mut prefix[1..], distributor_id);
        MerkleTable::new(MapTable::new(prefix, &self))
    }
    pub fn owner_reports(&self,
                         id: u16,
                         fingerprint: &Fingerprint)
                         -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Uuid> {
        let mut prefix = vec![36; 35];
        LittleEndian::write_u16(&mut prefix[1..], id);
        prefix[3..].copy_from_slice(fingerprint.as_ref());

        MerkleTable::new(MapTable::new(prefix, &self))
    }
    pub fn distributor_reports(&self,
                               id: u16,
                               fingerprint: &Fingerprint)
                               -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u64, Uuid> {
        let mut prefix = vec![37; 35];
        LittleEndian::write_u16(&mut prefix[1..], id);
        prefix[3..].copy_from_slice(fingerprint.as_ref());

        MerkleTable::new(MapTable::new(prefix, &self))
    }

    pub fn participants(&self) -> MapTable<F, PublicKey, u16> {
        MapTable::new(vec![40], &self)
    }
    pub fn fingerprints(&self) -> ListTable<MapTable<F, [u8], Vec<u8>>, u64, Fingerprint> {
        ListTable::new(MapTable::new(vec![41], &self))
    }

    pub fn list_content(&self) -> StorageResult<Vec<(Fingerprint, Content)>> {
        let mut v = Vec::new();
        for fingerprint in self.fingerprints().values()? {
            if let Some(content) = self.contents().get(&fingerprint)? {
                v.push((fingerprint, content));
            }
        }
        Ok(v)
    }
    // TODO видимо нужны еще техниеческие таблицы, чтобы не было O(n)
    pub fn find_contract(&self,
                         id: u16,
                         fingerprint: &Fingerprint)
                         -> StorageResult<Option<(u16, Contract)>> {
        let contracts = self.distributor_contracts(id);
        let values = contracts.values()?;
        let r = values.into_iter()
            .enumerate()
            .find(|&(_, ref c)| c.fingerprint() == fingerprint)
            .map(|(x, y)| (x as u16, y));
        Ok(r)
    }
    pub fn find_ownership(&self,
                          id: u16,
                          fingerprint: &Fingerprint)
                          -> StorageResult<Option<(u16, Ownership)>> {
        let contents = self.owner_contents(id);
        let values = contents.values()?;
        let r = values.into_iter()
            .enumerate()
            .find(|&(_, ref c)| c.fingerprint() == fingerprint)
            .map(|(x, y)| (x as u16, y));
        Ok(r)
    }
}


#[cfg(test)]
mod tests {
    use time::get_time;

    use exonum::crypto::{gen_keypair, hash};
    use super::{Owner, Distributor, Content, Contract, Ownership, Report};
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
        let mut ownership = Ownership::new(&f, p, a, &r);

        assert_eq!(ownership.fingerprint(), &f);
        assert_eq!(ownership.plays(), p);
        assert_eq!(ownership.amount(), a);
        assert_eq!(ownership.reports_hash(), &r);

        let r = hash(&[12, 213, 3]);
        ownership.add_amount(a);
        ownership.add_plays(p);
        ownership.set_reports_hash(&r);

        assert_eq!(ownership.amount(), a * 2);
        assert_eq!(ownership.plays(), p * 2);
        assert_eq!(ownership.reports_hash(), &r);
    }

    #[test]
    fn test_contract() {
        let f = hash(&[]);
        let p = 10;
        let a = 1000;
        let r = hash(&[]);
        let mut contract = Contract::new(&f, p, a, &r);

        assert_eq!(contract.fingerprint(), &f);
        assert_eq!(contract.plays(), p);
        assert_eq!(contract.amount(), a);
        assert_eq!(contract.reports_hash(), &r);

        let r = hash(&[12, 213, 3]);
        contract.add_amount(a);
        contract.add_plays(p);
        contract.set_reports_hash(&r);

        assert_eq!(contract.amount(), a * 2);
        assert_eq!(contract.plays(), p * 2);
        assert_eq!(contract.reports_hash(), &r);
    }

    #[test]
    fn test_report() {
        let i = 0;
        let f = hash(&[]);
        let d = get_time();
        let p = 1000;
        let a = 10000;
        let c = "Comment";
        let report = Report::new(i, &f, d, p, a, c);

        assert_eq!(report.distributor_id(), i);
        assert_eq!(report.fingerprint(), &f);
        assert_eq!(report.time(), d);
        assert_eq!(report.plays(), p);
        assert_eq!(report.amount(), a);
        assert_eq!(report.comment(), c);
    }
}
