use num::{Integer, range, ToPrimitive, pow};

use std::marker::PhantomData;
use std::cell::Cell;

use crypto::Hash;
use super::{BaseTable, View, List, Error, VoidKey, StorageValue};
use self::proofnode::Proofnode;


mod proofnode;

impl<'a, V: StorageValue> MerkleTable<'a, V> {

    pub fn get_proof(&self, index: u64) -> Proofnode<V> {
        self.get_range_proof(index, index + 1)
    }

    pub fn get_range_proof(&self, from: u64, to: u64) -> Proofnode<V> {
        let to = ::cmp::max(to, self.len());
        let from = ::cmp::min(from, to);

        fn construct(&self, height: u64, index: u64, from: u64, to: u64) -> Proofnode<V> {
            if height == 1 {
                return Proofnode::Leaf(self.get(index).unwrap())
            }
            let (left, right) = (index << 1, index << 1 + 1);
            let middle = (1 << (height - 2)) * right;

            if middle > to {
                Proofnode::Left(Box::new(self.construct(height - 1, left, from, to)),
                                match self.has_branch(height - 1, right) {
                                    true => Some(self.get_branch(height - 1, right)),
                                    false => None
                                })
            } else if middle <= from {
                Proofnode::Right(self.get_branch(height - 1, left).unwrap(),
                                 Box::new(self.construct(height - 1, right, from, to)
            } else {
                Proofnode::Full(Box::new(self.construct(height - 1, left, from, middle)),
                                Box::new(self.construct(height - 1, right, middle, to)))
            }

        }
        construct(self.height(), 0, from, to)
    }

}
