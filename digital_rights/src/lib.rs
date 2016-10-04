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

use exonum::messages::{RawMessage, Message, Error as MessageError};
use exonum::crypto::{PublicKey, Hash, hash};
use exonum::storage::{Map, Database, Fork, Error, MerklePatriciaTable, MapTable, List};
use exonum::blockchain::{Blockchain, View};

pub use txs::{DigitalRightsTx, TxCreateOwner, TxCreateDistributor};
pub use view::{DigitalRightsView, Owner, Distributor};

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
            _ => {
                unimplemented!();
            }
        }
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use byteorder::{ByteOrder, LittleEndian};

    use exonum::crypto::gen_keypair;
    use exonum::storage::MemoryDB;
    use exonum::blockchain::Blockchain;
    use exonum::messages::Message;

    use super::{DigitalRightsTx, DigitalRightsBlockchain, TxCreateOwner, TxCreateDistributor};

}
