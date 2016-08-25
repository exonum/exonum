mod storages;

use std::borrow::{Borrow, BorrowMut};

use ::messages::Message;
use ::storage::{StorageValue, Patch, Database, Fork, Error};

pub use self::storages::{TxStorage, BlockStorage};

pub trait Blockchain: Sized
    where Self: Borrow<<Self as Blockchain>::Database>,
          Self: BorrowMut<<Self as Blockchain>::Database>
{
    type Database: Database;
    type Transaction: Message + StorageValue;

    fn fork(&self) -> Fork<Self::Database> {
        self.borrow().fork()
    }

    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        self.borrow_mut().merge(patch)
    }
}

impl<T, Tx, Db> TxStorage<Db, Tx> for T
    where T: Blockchain<Database = Db, Transaction = Tx>,
          Db: Database,
          Tx: Message + StorageValue
{
}

impl<'a, Tx, Db> TxStorage<Fork<'a, Db>, Tx> for Fork<'a, Db>
    where Db: Database,
          Tx: Message + StorageValue
{
}

impl<T, Db, Tx> BlockStorage<Db> for T
    where T: Blockchain<Database = Db, Transaction = Tx>,
          Db: Database,
          Tx: Message + StorageValue
{
}

impl<'a, Db> BlockStorage<Fork<'a, Db>> for Fork<'a, Db> where Db: Database {}
