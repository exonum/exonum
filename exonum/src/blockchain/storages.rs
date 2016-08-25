use std::slice::SliceConcatExt;
use std::borrow::{Borrow, BorrowMut};
// use std::iter::Iterator;

use ::crypto::Hash;
use ::messages::{Precommit, Propose, Message};
use ::storage::{StorageValue, Database, Error, ListTable, MapTable, Map, List};

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
    fn proposes(&mut self) -> MapTable<D, Hash, Propose> {
        MapTable::new(vec![01], self.borrow_mut())
    }

    fn heights(&mut self) -> ListTable<MapTable<D, [u8], Vec<u8>>, u64, Hash> {
        ListTable::new(MapTable::new(vec![02], self.borrow_mut()))
    }

    fn last_hash(&mut self) -> Result<Option<Hash>, Error> {
        self.heights().last()
    }

    fn last_propose(&mut self) -> Result<Option<Propose>, Error> {
        Ok(match self.last_hash()? {
            Some(hash) => Some(self.proposes().get(&hash)?.unwrap()),
            None => None,
        })

    }

    fn precommits(&mut self, hash: &Hash) -> ListTable<MapTable<D, [u8], Vec<u8>>, u32, Precommit> {
        ListTable::new(MapTable::new([&[03], hash.as_ref()].concat(), self.borrow_mut()))
    }
}
