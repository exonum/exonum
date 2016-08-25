use std::slice::SliceConcatExt;
use std::borrow::{Borrow, BorrowMut};
// use std::iter::Iterator;

use ::crypto::Hash;
use ::messages::{Precommit, Propose, Message};
use ::storage::{StorageValue, Database, Fork, ListTable, MapTable};

use super::{Block, Blockchain};

pub trait TxStorage<D: Database, T: Message + StorageValue>
    where Self: Borrow<D> + BorrowMut<D>
{
    fn transactions(&mut self) -> MapTable<D, Hash, T> {
        MapTable::new(vec![00], self.borrow_mut())
    }
}

pub trait BlockStorage<D: Database>
    where Self: Borrow<D> + BorrowMut<D>
{
    fn blocks(&mut self) -> MapTable<D, Hash, Block> {
        MapTable::new(vec![01], self.borrow_mut())
    }

    // FIXME: remove this!
    fn proposes(&mut self) -> MapTable<D, Hash, Propose> {
        MapTable::new(vec![10], self.borrow_mut())
    }

    fn heights(&mut self) -> ListTable<MapTable<D, [u8], Vec<u8>>, u64, Hash> {
        ListTable::new(MapTable::new(vec![02], self.borrow_mut()))
    }

    fn block_txs(&mut self, hash: &Hash) -> ListTable<MapTable<D, [u8], Vec<u8>>, u32, Hash> {
        ListTable::new(MapTable::new([&[03], hash.as_ref()].concat(), self.borrow_mut()))
    }

    fn precommits(&mut self, hash: &Hash) -> ListTable<MapTable<D, [u8], Vec<u8>>, u32, Precommit> {
        ListTable::new(MapTable::new([&[03], hash.as_ref()].concat(), self.borrow_mut()))
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

