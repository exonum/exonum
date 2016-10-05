#![feature(type_ascription)]
#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]
#![feature(question_mark)]

#[macro_use(message, storage_value)]
extern crate exonum;
extern crate time;
extern crate byteorder;
extern crate blockchain_explorer;
extern crate serde;

mod txs;
mod view;
pub mod api;

use std::ops::Deref;

use exonum::messages::Message;
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Map, Database, Fork, Error, MerklePatriciaTable, MapTable, List};
use exonum::blockchain::{Blockchain};

pub use txs::{DigitalRightsTx, TxCreateOwner, TxCreateDistributor, TxAddContent, ContentShare, TxAddContract};
pub use view::{DigitalRightsView, Owner, Distributor, Content, Ownership, Contract};

const OWNERS_MAX_COUNT: u64 = 5000;

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

        Ok(hash(b.as_ref()))
    }

    fn execute(view: &Self::View, tx: &Self::Transaction) -> Result<(), Error> {
        match *tx {
            DigitalRightsTx::CreateOwner(ref tx) => {
                let owners = view.owners();
                if owners.len()? < OWNERS_MAX_COUNT {
                    let owner = Owner::new(tx.pub_key(), tx.name(), &hash(&[]));
                    owners.append(owner)?;
                }
            }
            DigitalRightsTx::CreateDistributor(ref tx) => {
                let distributor = Distributor::new(tx.pub_key(), tx.name(), &hash(&[]));
                view.distributors().append(distributor)?;
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
                                           tx.owners());
                view.contents().put(tx.fingerprint(), content)?;

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
                let r = view.distributors().get(tx.distributor_id() as u64)?;
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

                if view.contents().get(tx.fingerprint())?.is_none() {
                    return Ok(());
                }

                // TODO проверка, что мы не регистрировали такой же контракт

                let contract = Contract::new(tx.fingerprint(), 0, 0, &hash(&[]));
                let contracts = view.distributor_contracts(tx.distributor_id());
                contracts.append(contract)?;

                let hash = &contracts.root_hash()?;
                distrubutor.set_contracts_hash(hash);
                view.distributors().set(tx.distributor_id() as u64, distrubutor)?;
            }
            _ => {
                unimplemented!();
            }
        }
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use exonum::crypto::{gen_keypair, hash};
    use exonum::storage::{Map, List, Database, MemoryDB, Result as StorageResult};
    use exonum::blockchain::Blockchain;

    use super::{DigitalRightsView, DigitalRightsTx, DigitalRightsBlockchain, TxCreateOwner,
                TxCreateDistributor, TxAddContent, ContentShare, TxAddContract};

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
    }

    #[test]
    fn test_add_contract() {
        let b = DigitalRightsBlockchain { db: MemoryDB::new() };
        let v = b.view();

        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();

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
            let co1 = TxCreateOwner::new(&p1, "o1", &s1);
            let co2 = TxCreateOwner::new(&p2, "o2", &s2);
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
        }

        {
            let ac = TxAddContract::new(&p2, 1, f1, &s2);
            execute_tx::<MemoryDB>(&v, DigitalRightsTx::AddContract(ac.clone())).unwrap();
            let contracts = v.distributor_contracts(1);
            let c = contracts.get(0).unwrap().unwrap();
            assert_eq!(c.fingerprint(), f1);

            let d1 = v.distributors().get(0).unwrap().unwrap();
            assert_eq!(d1.contracts_hash(), &contracts.root_hash().unwrap());
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
}
