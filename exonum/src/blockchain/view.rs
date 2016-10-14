use std::slice::SliceConcatExt;
use std::ops::Deref;

use ::crypto::Hash;
use ::messages::{Precommit, Propose, Message};
use ::storage::{StorageValue, Fork, ListTable, MapTable, MerkleTable};

use super::Block;

pub trait View<F: Fork>: Deref<Target = F> {
    type Transaction: Message + StorageValue;

    fn from_fork(fork: F) -> Self;

    fn transactions(&self) -> MapTable<F, Hash, Self::Transaction> {
        MapTable::new(vec![00], self)
    }

    fn blocks(&self) -> MapTable<F, Hash, Block> {
        MapTable::new(vec![01], self)
    }

    fn heights(&self) -> ListTable<MapTable<F, [u8], Vec<u8>>, u64, Hash> {
        ListTable::new(MapTable::new(vec![02], self))
    }

    fn block_txs(&self, height: u64) -> MerkleTable<MapTable<F, [u8], Vec<u8>>, u32, Hash> {
        MerkleTable::new(MapTable::new([&[03u8] as &[u8], &height.serialize()].concat(), self))
    }

    fn precommits(&self, hash: &Hash) -> ListTable<MapTable<F, [u8], Vec<u8>>, u32, Precommit> {
        ListTable::new(MapTable::new([&[03], hash.as_ref()].concat(), self))
    }
}
