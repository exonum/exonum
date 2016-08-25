mod block;
mod storages;

use std::collections::HashMap;
use std::borrow::{Borrow, BorrowMut};

use ::crypto::{Hash, hash};
use ::messages::{Propose, Precommit, Message};
use ::storage::{StorageValue, Patch, Database, Fork, Error, Map, List};

pub use self::block::Block;
pub use self::storages::{TxStorage, BlockStorage};

pub trait Blockchain: Sized
    where Self: Borrow<<Self as Blockchain>::Database>,
          Self: BorrowMut<<Self as Blockchain>::Database>
{
    type Database: Database;
    type Transaction: Message + StorageValue;

    fn last_hash(&mut self) -> Result<Option<Hash>, Error> {
        self.heights().last()
    }

    fn last_block(&mut self) -> Result<Option<Block>, Error> {
        Ok(match self.last_hash()? {
            Some(hash) => Some(self.blocks().get(&hash)?.unwrap()),
            None => None,
        })

    }

    fn verify_tx(tx: &Self::Transaction) -> bool;
    fn state_hash(fork: &mut Fork<Self::Database>) -> Hash;
    fn execute(fork: &mut Fork<Self::Database>, tx: &Self::Transaction) -> Result<(), Error>;

    fn create_patch(&mut self,
                    propose: &Propose,
                    txs: &HashMap<Hash, Self::Transaction>) -> Result<(Hash, Patch), Error> {
        // Get last hash
        let last_hash = self.last_hash()?.unwrap_or(hash(&[]));
        // Create fork
        let mut fork = self.fork();
        // FIXME: apply txs here
        // Get state hash
        let state_hash = Self::state_hash(&mut fork);
        // Create block
        let block = Block::new(propose.height(), propose.time(), &last_hash, &state_hash);
        // Eval block hash
        let block_hash = block.hash();
        // Update height
        fork.heights().append(block_hash).is_ok();
        // Save block
        fork.blocks().put(&block_hash, block).is_ok();
        // Save propose (FIXME: remove)
        fork.proposes().put(&Message::hash(propose), propose.clone()).is_ok();
        // Save transactions
        for hash in propose.transactions() {
            let tx = txs.get(hash).unwrap().clone();
            Self::execute(&mut fork, &tx)?;
            fork.transactions()
                .put(hash, tx)
                .unwrap();
            fork.block_txs(&block_hash)
                .append(*hash)
                .unwrap();
        }

        Ok((block_hash, fork.into()))
    }

    fn commit<'a, I: Iterator<Item=&'a Precommit>>(&mut self,
              block_hash: Hash,
              patch: Patch,
              precommits: I) -> Result<(), Error> {
        let patch = {
          let mut fork = Fork::new(self.borrow_mut(), patch);

          for precommit in precommits {
              fork.precommits(&block_hash).append(precommit.clone())?;
          }

          fork.into()
        };

        self.merge(patch)
    }

    fn fork(&self) -> Fork<Self::Database> {
        self.borrow().fork()
    }

    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        self.borrow_mut().merge(patch)
    }
}
