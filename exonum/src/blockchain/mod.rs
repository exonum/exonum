mod block;
mod storages;

use std::collections::HashMap;
use std::borrow::Borrow;
use std::ops::Deref;

use ::crypto::{Hash, hash};
use ::messages::{Propose, Precommit, Message};
use ::storage::{StorageValue, Patch, Database, Fork, Error, Map, List};

pub use self::block::Block;
pub use self::storages::View;

pub trait Blockchain: Sized + Clone + Send + 'static
    where Self: Deref<Target = <Self as Blockchain>::Database>
{
    type View: View<<<Self as Blockchain>::Database as Database>::Fork, Transaction=Self::Transaction>;
    type Database: Database;
    type Transaction: Message + StorageValue;

    fn last_hash(&self) -> Result<Option<Hash>, Error> {
        self.view().heights().last()
    }

    fn last_block(&self) -> Result<Option<Block>, Error> {
        Ok(match self.last_hash()? {
            Some(hash) => Some(self.view().blocks().get(&hash)?.unwrap()),
            None => None,
        })

    }

    fn verify_tx(tx: &Self::Transaction) -> bool;
    fn state_hash(fork: &Self::View) -> Result<Hash, Error>;
    fn execute(fork: &Self::View, tx: &Self::Transaction) -> Result<(), Error>;

    fn create_patch(&self,
                    propose: &Propose,
                    txs: &HashMap<Hash, Self::Transaction>)
                    -> Result<(Hash, Patch), Error> {
        // Get last hash
        let last_hash = self.last_hash()?.unwrap_or(hash(&[]));
        // Create fork
        let mut fork = self.view();
        // Save & execute transactions
        for hash in propose.transactions() {
            let tx = txs.get(hash).unwrap().clone();
            Self::execute(&mut fork, &tx)?;
            fork.transactions()
                .put(hash, tx)
                .unwrap();
            fork.block_txs(propose.height())
                .append(*hash)
                .unwrap();
        }
        // Get tx hash
        let tx_hash = fork.block_txs(propose.height()).root_hash()?.unwrap_or(hash(&[]));
        // Get state hash
        let state_hash = Self::state_hash(&mut fork)?;
        // Create block
        let block = Block::new(propose.height(),
                               propose.time(),
                               &last_hash,
                               &tx_hash,
                               &state_hash);
        // Eval block hash
        let block_hash = block.hash();
        // Update height
        // TODO: check that height == propose.height
        fork.heights().append(block_hash).is_ok();
        // Save block
        fork.blocks().put(&block_hash, block).is_ok();
        // Save propose (FIXME: remove)
        fork.proposes().put(&Message::hash(propose), propose.clone()).is_ok();

        Ok((block_hash, fork.changes()))
    }

    fn commit<'a, I: Iterator<Item = &'a Precommit>>(&self,
                                                     block_hash: Hash,
                                                     patch: Patch,
                                                     precommits: I)
                                                     -> Result<(), Error> {
        let patch = {
            let view = self.view();
            view.merge(patch);

            for precommit in precommits {
                view.precommits(&block_hash).append(precommit.clone())?;
            }

            view.changes()
        };

        self.merge(patch)
    }

    fn view(&self) -> Self::View {
        Self::View::from_fork(self.borrow().fork())
    }

    fn merge(&self, patch: Patch) -> Result<(), Error> {
        self.deref().merge(patch)
    }
}
