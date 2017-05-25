// #[cfg(test)]
// mod tests;

use std::cell::Cell;
use std::marker::PhantomData;

use crypto::{Hash, hash};

use super::{pair_hash, BaseIndex, BaseIndexIter, Snapshot, Fork, StorageValue};

const HEIGHT_SHIFT : u64 = 58;
// TODO: add checks for overflow
const MAX_LENGTH : u64 = 288230376151711743; // 2 ** 58 - 1

fn key(height: u64, index: u64) -> u64 {
    debug_assert!(height <= 58 && index <= MAX_LENGTH);

    (height << HEIGHT_SHIFT) + index
}


/// Merkle tree over list. Data in table is stored in rows.
/// Height is determined by amount of values: `H = log2(values_length) + 1`
///
/// | Height | Stored data                                                                  |
/// |-------:|------------------------------------------------------------------------------|
/// |0 | Values, stored in the structure by index. A datum is stored at `(0, index)`        |
/// |1 | Hash of value datum, stored at level 0. `(1, index) = Hash((0, index))`            |
/// |>1| Merkle tree node, where at position `(h, i) = Hash((h - 1, 2i) + (h - 1, 2i + 1))` |
///
/// `+` operation is concatenation of byte arrays.
pub struct ProofListIndex<T, V> {
    base: BaseIndex<T>,
    length: Cell<Option<u64>>,
    _v: PhantomData<V>,
}

pub struct ProofListIndexIter<'a, V> {
    base_iter: BaseIndexIter<'a, u64, V>
}

impl<T, V> ProofListIndex<T, V> {
    pub fn new(prefix: Vec<u8>, base: T) -> Self {
        ProofListIndex {
            base: BaseIndex::new(prefix, base),
            length: Cell::new(None),
            _v: PhantomData
        }
    }
}

impl<T, V> ProofListIndex<T, V> where T: AsRef<Snapshot>,
                                      V: StorageValue {
    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&key(0, index))
    }

    pub fn last(&self) -> Option<V> {
        match self.len() {
            0 => None,
            l => self.get(l - 1)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> u64 {
        if let Some(len) = self.length.get() {
            return len
        }
        let len = self.base.get(&()).unwrap_or(0);
        self.length.set(Some(len));
        len
    }

    pub fn height(&self) -> u64 {
        self.len().next_power_of_two() + 1
    }

    pub fn root_hash(&self) -> Hash {
        self.base.get(&key(self.height(), 0)).unwrap_or_default()
    }
}

impl<'a, V> ProofListIndex<&'a mut Fork, V> where V: StorageValue {
    fn set_len(&mut self, len: u64) {
        self.base.put(&(), len);
        self.length.set(Some(len));
    }

    fn has_branch(&self, height: u64, index: u64) -> bool {
        debug_assert!(height > 0 && height <= self.height() && index <= (1 << height));

        index >> height != 0
    }

    fn get_branch(&self, height: u64, index: u64) -> Hash {
        debug_assert!(self.has_branch(height, index));

        self.base.get(&key(height, index)).unwrap()
    }

    fn set_branch(&mut self, height: u64, index: u64, hash: Hash) {
        debug_assert!(self.has_branch(height, index));

        self.base.put(&key(height, index), hash)
    }

    pub fn push(&mut self, value: V) {
        let len = self.len();
        self.base.put(&key(1, len), value.hash());
        self.base.put(&key(0, len), value);
        let mut height = 1;
        let mut index = len;
        while index > 0 {
            let hash = if index & 1 == 0 {
                hash(self.get_branch(height, index).as_ref())
            } else {
                pair_hash(&self.get_branch(height, index - 1),
                          &self.get_branch(height, index))
            };
            height += 1; index >>= 1;
            self.set_branch(height, index, hash);
        }
        self.set_len(len + 1)
    }

    pub fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=V> {
        for value in iter {
            self.push(value)
        }
    }

    pub fn set(&mut self, index: u64, value: V) {
        if index >= self.len() {
            panic!("index out of bounds: the len is {} but the index is {}", self.len(), index);
        }
        self.base.put(&key(1, index), value.hash());
        self.base.put(&key(0, index), value);
        let mut height = 1;
        let mut index = index | 1;
        while index > 0 {
            let (left, right) = (index | 1 - 1, index | 1);
            let hash = if self.has_branch(height, right) {
                pair_hash(&self.get_branch(height, left),
                          &self.get_branch(height, right))
            } else {
                hash(self.get_branch(height, left).as_ref())
            };
            height += 1; index >>= 1;
            self.set_branch(height, index, hash);
        }
    }

    pub fn clear(&mut self) {
        self.length.set(Some(0));
        self.base.clear()
    }
}
