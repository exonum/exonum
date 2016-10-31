#![feature(type_ascription)]
#![feature(conservative_impl_trait)]
#![feature(proc_macro)]

#[macro_use(message, storage_value)]
extern crate exonum;
extern crate time;
extern crate byteorder;
extern crate blockchain_explorer;
extern crate serde;
#[macro_use]
extern crate serde_derive;

mod txs;
mod view;
pub mod api;

use std::ops::Deref;

use exonum::messages::Message;
use exonum::crypto::{Hash, hash};
use exonum::storage::{Map, Database, Error, List};
use exonum::blockchain::Blockchain;

pub use txs::{DigitalRightsTx, TxCreateOwner, TxCreateDistributor, TxAddContent, ContentShare,
              TxAddContract, TxReport};
pub use view::{DigitalRightsView, Owner, Distributor, Content, Ownership, Contract, Report};

pub const OWNERS_MAX_COUNT: u16 = 5000;

pub type Uuid = Hash;
pub type Fingerprint = Hash;

#[derive(Clone)]
pub struct DigitalRightsBlockchain<D: Database> {
    pub db: D,
}

impl<D: Database> Deref for DigitalRightsBlockchain<D> {
    type Target = D;

    fn deref(&self) -> &D {
        &self.db
    }
}

pub enum Role {
    Distributor(u16),
    Owner(u16),
}

impl Role {
    pub fn id(&self) -> u16 {
        match *self {
            Role::Distributor(id) => id,
            Role::Owner(id) => id,
        }
    }
}

impl<D> Blockchain for DigitalRightsBlockchain<D>
    where D: Database
{
    type Database = D;
    type Transaction = DigitalRightsTx;
    type View = DigitalRightsView<D::Fork>;

    fn verify_tx(tx: &Self::Transaction) -> bool {
        tx.verify(tx.pub_key())
    }

    fn state_hash(view: &Self::View) -> Result<Hash, Error> {
        let mut b = Vec::new();
        b.extend_from_slice(view.distributors().root_hash()?.as_ref());
        b.extend_from_slice(view.owners().root_hash()?.as_ref());
        b.extend_from_slice(view.contents().root_hash()?.as_ref());

        for id in 0..view.distributors().len()? as u16 {
            let contracts = view.distributor_contracts(id);
            b.extend_from_slice(contracts.root_hash()?.as_ref());
            for contract in contracts.values()? {
                let reports = view.distributor_reports(id, contract.fingerprint());
                let hash = reports.root_hash()?;
                b.extend_from_slice(hash.as_ref());
            }
        }
        for id in 0..view.owners().len()? as u16 {
            let ownerships = view.owner_contents(id);
            b.extend_from_slice(ownerships.root_hash()?.as_ref());
            for ownership in ownerships.values()? {
                let reports = view.owner_reports(id, ownership.fingerprint());
                let hash = reports.root_hash()?;
                b.extend_from_slice(hash.as_ref());
            }
        }

        Ok(hash(b.as_ref()))
    }

    fn execute(view: &Self::View, tx: &Self::Transaction) -> Result<(), Error> {
        match *tx {
            DigitalRightsTx::CreateOwner(ref tx) => {
                if view.find_participant(tx.pub_key())?.is_some() {
                    return Ok(());
                }

                let owners = view.owners();
                let owner_id = owners.len()? as u16;
                if owner_id < OWNERS_MAX_COUNT {
                    let owner = Owner::new(tx.pub_key(), tx.name(), &hash(&[]));
                    owners.append(owner)?;
                    view.add_participant(tx.pub_key(), Role::Owner(owner_id))?;
                }
            }
            DigitalRightsTx::CreateDistributor(ref tx) => {
                if view.find_participant(tx.pub_key())?.is_some() {
                    return Ok(());
                }

                let distributors = view.distributors();
                let distributor_id = distributors.len()?;

                let distributor = Distributor::new(tx.pub_key(), tx.name(), &hash(&[]));
                distributors.append(distributor)?;
                view.add_participant(tx.pub_key(), Role::Distributor(distributor_id as u16))?
            }
            DigitalRightsTx::AddContent(ref tx) => {
                // preconditions
                if view.contents().get(tx.fingerprint())?.is_some() {
                    return Ok(());
                }
                let (sum, shares) = {
                    let mut sum = 0;
                    let shares = tx.owners()
                        .iter()
                        .cloned()
                        .map(|x| x.into())
                        .collect::<Vec<ContentShare>>();

                    for content in &shares {
                        sum += content.share;
                        if view.owners().get(content.owner_id as u64)?.is_none() {
                            return Ok(());
                        }
                    }
                    (sum, shares)
                };
                if sum != 100 {
                    return Ok(());
                }

                // execution
                let content = Content::new(tx.title(),
                                           tx.price_per_listen(),
                                           tx.min_plays(),
                                           tx.additional_conditions(),
                                           tx.owners(),
                                           &[]);
                view.contents().put(tx.fingerprint(), content)?;
                view.fingerprints().append(*tx.fingerprint())?;

                for content_share in &shares {
                    let ownership = Ownership::new(tx.fingerprint(), 0, 0, &hash(&[]));

                    let owner_contents = view.owner_contents(content_share.owner_id);
                    owner_contents.append(ownership)?;

                    // update ownership hash
                    let hash = owner_contents.root_hash()?;
                    let mut owner = view.owners().get(content_share.owner_id as u64)?.unwrap();
                    owner.set_ownership_hash(&hash);
                    view.owners().set(content_share.owner_id as u64, owner)?;
                }
            }
            DigitalRightsTx::AddContract(ref tx) => {
                let id = tx.distributor_id();
                let fingerprint = tx.fingerprint();

                let r = view.distributors().get(id as u64)?;
                let mut distrubutor = {
                    if let Some(d) = r {
                        if d.pub_key() != tx.pub_key() {
                            return Ok(());
                        }
                        d
                    } else {
                        return Ok(());
                    }
                };

                let mut content = {
                    if let Some(content) = view.contents().get(fingerprint)? {
                        content
                    } else {
                        return Ok(());
                    }
                };

                // Проверка, нет ли у нас контракта на этот контент
                // TODO сделать, чтобы реализация работала не за O(n)
                if content.distributors().contains(&id) {
                    return Ok(());
                }

                let mut distrubutors = content.distributors().to_vec();
                distrubutors.push(id);
                content.set_distributors(distrubutors.as_ref());
                view.contents().put(fingerprint, content)?;

                let contract = Contract::new(fingerprint, 0, 0, &hash(&[]));
                let contracts = view.distributor_contracts(id);
                contracts.append(contract)?;

                let hash = &contracts.root_hash()?;
                distrubutor.set_contracts_hash(hash);
                view.distributors().set(id as u64, distrubutor)?;
            }
            DigitalRightsTx::Report(ref tx) => {
                let id = tx.distributor_id();
                let fingerprint = tx.fingerprint();

                // preconditions
                if view.reports().get(tx.uuid())?.is_some() {
                    return Ok(());
                }
                let mut distrubutor = {
                    if let Some(d) = view.distributors().get(id as u64)? {
                        if d.pub_key() != tx.pub_key() {
                            return Ok(());
                        }
                        d
                    } else {
                        return Ok(());
                    }
                };
                let content = {
                    if let Some(content) = view.contents().get(fingerprint)? {
                        content
                    } else {
                        return Ok(());
                    }
                };
                for share in content.shares() {
                    if view.owners().get(share.owner_id as u64)?.is_none() {
                        return Ok(());
                    }
                }

                let (contract_id, mut contract) = {
                    let r = view.find_contract(id, &fingerprint)?;
                    if let Some((contract_id, contract)) = r {
                        (contract_id as u64, contract)
                    } else {
                        return Ok(());
                    }
                };

                let amount = content.price_per_listen() * tx.plays();
                let report = Report::new(id,
                                         &fingerprint,
                                         tx.time(),
                                         tx.plays(),
                                         amount,
                                         tx.comment());
                view.reports().put(tx.uuid(), report)?;

                // update hashes and other fields
                let distrubutor_reports = view.distributor_reports(id, fingerprint);
                distrubutor_reports.append(*tx.uuid())?;

                // update contract
                contract.add_amount(amount);
                contract.add_plays(tx.plays());
                contract.set_reports_hash(&distrubutor_reports.root_hash()?);
                view.distributor_contracts(id).set(contract_id, contract)?;

                // update distributor
                distrubutor.set_contracts_hash(&view.distributor_contracts(id).root_hash()?);
                view.distributors().set(id as u64, distrubutor)?;

                // update owners
                for share in content.shares() {
                    let id = share.owner_id;
                    let mut owner = view.owners().get(id as u64)?.unwrap();
                    let (ownership_id, mut ownership) = {
                        let r = view.find_ownership(id, &fingerprint)?;
                        if let Some((ownership_id, ownership)) = r {
                            (ownership_id as u64, ownership)
                        } else {
                            return Ok(());
                        }
                    };

                    // Update reports hash
                    let owner_reports = view.owner_reports(id, fingerprint);
                    owner_reports.append(*tx.uuid())?;

                    // Update ownership
                    ownership.add_amount(amount * share.share as u64 / 100);
                    ownership.add_plays(tx.plays());
                    ownership.set_reports_hash(&owner_reports.root_hash()?);
                    view.owner_contents(id).set(ownership_id, ownership)?;

                    owner.set_ownership_hash(&view.owner_contents(id).root_hash()?);
                    view.owners().set(id as u64, owner)?;
                }
            }
        }
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use time;

    use exonum::crypto::{gen_keypair, hash};
    use exonum::storage::{Map, List, Database, MemoryDB, Result as StorageResult};
    use exonum::blockchain::Blockchain;

    use super::{DigitalRightsView, DigitalRightsTx, DigitalRightsBlockchain, TxCreateOwner,
                TxCreateDistributor, TxAddContent, ContentShare, TxAddContract, TxReport};

    fn execute_tx<D: Database>(v: &DigitalRightsView<D::Fork>,
                               tx: DigitalRightsTx)
                               -> StorageResult<()> {
        DigitalRightsBlockchain::<D>::execute(v, &tx)
    }

    #[test]
    fn test_add_content() {
        let b = DigitalRightsBlockchain { db: MemoryDB::new() };
        let v = b.view();

        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();

        let co1 = TxCreateOwner::new(&p1, "o1", &s1);
        let co2 = TxCreateOwner::new(&p2, "o2", &s2);
        execute_tx::<MemoryDB>(&v, DigitalRightsTx::CreateOwner(co1.clone())).unwrap();
        execute_tx::<MemoryDB>(&v, DigitalRightsTx::CreateOwner(co2.clone())).unwrap();
        let o1 = v.owners().get(0).unwrap().unwrap();
        let o2 = v.owners().get(1).unwrap().unwrap();
        assert_eq!(o1.pub_key(), co1.pub_key());
        assert_eq!(o1.name(), co1.name());
        assert_eq!(o2.pub_key(), co2.pub_key());
        assert_eq!(o2.name(), co2.name());

        {
            let d1 = [ContentShare::new(0, 30).into(), ContentShare::new(1, 70).into()];
            let f1 = &hash(&[1, 2, 3, 4]);
            let ac1 = TxAddContent::new(&p1, f1, "Manowar", 1, 10, d1.as_ref(), "None", &s1);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContent(ac1.clone())).unwrap();
            let c1 = v.contents().get(&f1).unwrap().unwrap();
            assert_eq!(c1.title(), ac1.title());
            let o1 = v.owners().get(0).unwrap().unwrap();
            let o2 = v.owners().get(1).unwrap().unwrap();
            assert_eq!(o1.ownership_hash(),
                       &v.owner_contents(0).root_hash().unwrap());
            assert_eq!(o2.ownership_hash(),
                       &v.owner_contents(1).root_hash().unwrap());
            assert!(v.fingerprints().values().unwrap().contains(f1));
        }

        {
            let f2 = &hash(&[1]);
            let ac2 = TxAddContent::new(&p1, f2, "Nanowar", 1, 10, &[], "None", &s1);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContent(ac2.clone())).unwrap();
            assert_eq!(v.contents().get(&f2).unwrap(), None);
        }

        {
            let f3 = &hash(&[2]);
            let d3 = [ContentShare::new(3, 30).into(), ContentShare::new(1, 70).into()];
            let ac3 = TxAddContent::new(&p1, f3, "Korn", 1, 10, d3.as_ref(), "Some", &s1);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContent(ac3.clone())).unwrap();
            assert_eq!(v.contents().get(&f3).unwrap(), None);
        }

        {
            let f4 = &hash(&[3]);
            let d4 = [ContentShare::new(1, 40).into(), ContentShare::new(1, 70).into()];
            let ac4 = TxAddContent::new(&p1, f4, "Slipknot", 1, 10, d4.as_ref(), "Some", &s1);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContent(ac4.clone())).unwrap();
            assert_eq!(v.contents().get(&f4).unwrap(), None);
        }

        {
            let f5 = &hash(&[4]);
            let d5 = [ContentShare::new(0, 100).into()];
            let ac5 = TxAddContent::new(&p1, f5, "Epica", 1, 10, d5.as_ref(), "Some", &s1);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContent(ac5.clone())).unwrap();

            let c = v.contents().get(&f5).unwrap().unwrap();
            let shares = c.shares();
            let id = shares[0].owner_id;
            let owner = v.owners().get(id as u64).unwrap().unwrap();
            assert_eq!(owner.pub_key(), &p1);

            let (_, ownership) = v.find_ownership(id, f5).unwrap().unwrap();
            assert_eq!(ownership.fingerprint(), f5);
            assert_eq!(ownership.plays(), 0);
            assert_eq!(ownership.amount(), 0);

            assert_eq!(v.owner_reports(id, f5).len().unwrap(), 0);
        }
    }

    #[test]
    fn test_add_contract() {
        let b = DigitalRightsBlockchain { db: MemoryDB::new() };
        let v = b.view();

        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();
        let (p3, s3) = gen_keypair();
        let (p4, s4) = gen_keypair();

        let cd1 = TxCreateDistributor::new(&p1, "d1", &s1);
        let cd2 = TxCreateDistributor::new(&p2, "d2", &s2);
        execute_tx::<MemoryDB>(&v, DigitalRightsTx::CreateDistributor(cd1.clone())).unwrap();
        execute_tx::<MemoryDB>(&v, DigitalRightsTx::CreateDistributor(cd2.clone())).unwrap();
        let d1 = v.distributors().get(0).unwrap().unwrap();
        let d2 = v.distributors().get(1).unwrap().unwrap();
        assert_eq!(d1.pub_key(), cd1.pub_key());
        assert_eq!(d1.name(), cd1.name());
        assert_eq!(d2.pub_key(), cd2.pub_key());
        assert_eq!(d2.name(), cd2.name());

        let f1 = &hash(&[1, 2, 3, 4]);
        {
            let co1 = TxCreateOwner::new(&p3, "o1", &s3);
            let co2 = TxCreateOwner::new(&p4, "o2", &s4);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::CreateOwner(co1.clone())).unwrap();
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::CreateOwner(co2.clone())).unwrap();

            let d1 = [ContentShare::new(0, 30).into(), ContentShare::new(1, 70).into()];
            let ac1 = TxAddContent::new(&p1, f1, "Manowar", 1, 10, d1.as_ref(), "None", &s1);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContent(ac1.clone())).unwrap();
        }

        {
            let ac = TxAddContract::new(&p1, 0, f1, &s1);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContract(ac.clone())).unwrap();
            let contracts = v.distributor_contracts(0);
            let c = contracts.get(0).unwrap().unwrap();
            assert_eq!(c.fingerprint(), f1);

            let d1 = v.distributors().get(0).unwrap().unwrap();
            assert_eq!(d1.contracts_hash(), &contracts.root_hash().unwrap());

            let content = v.contents().get(f1).unwrap().unwrap();
            assert_eq!(content.distributors(), &[0]);
        }

        {
            let ac = TxAddContract::new(&p2, 1, f1, &s2);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContract(ac.clone())).unwrap();
            let contracts = v.distributor_contracts(1);
            let c = contracts.get(0).unwrap().unwrap();
            assert_eq!(c.fingerprint(), f1);

            let d1 = v.distributors().get(0).unwrap().unwrap();
            assert_eq!(d1.contracts_hash(), &contracts.root_hash().unwrap());

            let content = v.contents().get(f1).unwrap().unwrap();
            assert_eq!(content.distributors(), &[0, 1]);
        }

        {
            let ac = TxAddContract::new(&p1, 1, f1, &s1);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContract(ac.clone())).unwrap();
            let contracts = v.distributor_contracts(0);
            assert_eq!(contracts.get(1).unwrap(), None);
        }

        {
            let f2 = &hash(&[3, 2, 3, 4]);
            let ac = TxAddContract::new(&p2, 1, f2, &s2);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContract(ac.clone())).unwrap();
            let contracts = v.distributor_contracts(1);
            assert_eq!(contracts.get(1).unwrap(), None);
        }
    }

    #[test]
    fn test_report() {
        let b = DigitalRightsBlockchain { db: MemoryDB::new() };
        let v = b.view();

        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();
        let (p3, s3) = gen_keypair();
        let (p4, s4) = gen_keypair();
        let price = 10;

        let cd1 = TxCreateDistributor::new(&p1, "d1", &s1);
        let cd2 = TxCreateDistributor::new(&p2, "d2", &s2);
        execute_tx::<MemoryDB>(&v, DigitalRightsTx::CreateDistributor(cd1.clone())).unwrap();
        execute_tx::<MemoryDB>(&v, DigitalRightsTx::CreateDistributor(cd2.clone())).unwrap();

        let f1 = &hash(&[1, 2, 3, 4]);
        {
            let co1 = TxCreateOwner::new(&p3, "o1", &s3);
            let co2 = TxCreateOwner::new(&p4, "o2", &s4);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::CreateOwner(co1.clone())).unwrap();
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::CreateOwner(co2.clone())).unwrap();

            let d1 = [ContentShare::new(0, 30).into(), ContentShare::new(1, 70).into()];
            let ac1 = TxAddContent::new(&p1, f1, "Manowar", price, 10, d1.as_ref(), "None", &s1);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContent(ac1.clone())).unwrap();
        }

        {
            let tx1 = DigitalRightsTx::AddContract(TxAddContract::new(&p1, 0, f1, &s1));
            let tx2 = DigitalRightsTx::AddContract(TxAddContract::new(&p2, 1, f1, &s2));
            execute_tx::<MemoryDB>(&v, tx1).unwrap();
            execute_tx::<MemoryDB>(&v, tx2).unwrap();
        }

        {
            let uuid = hash(&[90]);
            let did = 0;
            let time = time::get_time();
            let plays = 10000;
            let comment = "My First report";
            let report_tx = TxReport::new(&p1, &uuid, did, &f1, time, plays, comment, &s1);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::Report(report_tx.clone())).unwrap();
            let report = v.reports().get(&uuid).unwrap().unwrap();
            let total_amount = plays * price;
            assert_eq!(report.amount(), total_amount);

            let oc1 = v.owner_contents(0).get(0).unwrap().unwrap();
            let oc2 = v.owner_contents(1).get(0).unwrap().unwrap();

            assert_eq!(oc1.amount(), total_amount * 30 / 100);
            assert_eq!(oc2.amount(), total_amount * 70 / 100);
            assert_eq!(oc1.reports_hash(),
                       &v.owner_reports(0, f1).root_hash().unwrap());
            assert_eq!(oc2.reports_hash(),
                       &v.owner_reports(1, f1).root_hash().unwrap());

            let dc1 = v.distributor_contracts(0).get(0).unwrap().unwrap();
            assert_eq!(dc1.amount(), total_amount);

            assert_eq!(v.owner_reports(0, f1).get(0).unwrap().unwrap(), uuid);
            assert_eq!(v.owner_reports(1, f1).get(0).unwrap().unwrap(), uuid);
            assert_eq!(v.distributor_reports(0, f1).get(0).unwrap().unwrap(), uuid);
        }
    }
}
