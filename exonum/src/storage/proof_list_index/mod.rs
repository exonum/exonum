use std::cell::Cell;
use std::marker::PhantomData;

use crypto::{Hash, hash, HASH_SIZE};

use super::{BaseIndex, BaseIndexIter, Snapshot, Fork, StorageValue};

use self::key::ProofListKey;

pub use self::proof::ListProof;

#[cfg(test)]
mod tests;
mod key;
mod proof;

// TODO: implement pop and truncate methods for Merkle tree

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
#[derive(Debug)]
pub struct ProofListIndex<T, V> {
    base: BaseIndex<T>,
    length: Cell<Option<u64>>,
    _v: PhantomData<V>,
}

#[derive(Debug)]
pub struct ProofListIndexIter<'a, V> {
    base_iter: BaseIndexIter<'a, ProofListKey, V>,
}

impl<T, V> ProofListIndex<T, V> {
    pub fn new(prefix: Vec<u8>, base: T) -> Self {
        ProofListIndex {
            base: BaseIndex::new(prefix, base),
            length: Cell::new(None),
            _v: PhantomData,
        }
    }
}

pub fn pair_hash(h1: &Hash, h2: &Hash) -> Hash {
    let mut v = [0; HASH_SIZE * 2];
    v[..HASH_SIZE].copy_from_slice(h1.as_ref());
    v[HASH_SIZE..].copy_from_slice(h2.as_ref());
    hash(&v)
}

impl<T, V> ProofListIndex<T, V>
    where T: AsRef<Snapshot>,
          V: StorageValue
{
    fn has_branch(&self, key: ProofListKey) -> bool {
        debug_assert!(key.height() > 0);

        key.first_left_leaf_index() < self.len()
    }

    fn get_branch(&self, key: ProofListKey) -> Option<Hash> {
        if self.has_branch(key) {
            self.base.get(&key)
        } else {
            None
        }
    }

    fn get_branch_unchecked(&self, key: ProofListKey) -> Hash {
        debug_assert!(self.has_branch(key));

        self.base.get(&key).unwrap()
    }

    fn root_key(&self) -> ProofListKey {
        ProofListKey::new(self.height(), 0)
    }

    fn construct_proof(&self, key: ProofListKey, from: u64, to: u64) -> ListProof<V> {
        if key.height() == 1 {
            return ListProof::Leaf(self.get(key.index()).unwrap());
        }
        let middle = key.first_right_leaf_index();
        if to <= middle {
            ListProof::Left(Box::new(self.construct_proof(key.left(), from, to)),
                            self.get_branch(key.right()))
        } else if middle <= from {
            ListProof::Right(self.get_branch_unchecked(key.left()),
                             Box::new(self.construct_proof(key.right(), from, to)))
        } else {
            ListProof::Full(Box::new(self.construct_proof(key.left(), from, middle)),
                            Box::new(self.construct_proof(key.right(), middle, to)))
        }
    }

    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&ProofListKey::leaf(index))
    }

    pub fn last(&self) -> Option<V> {
        match self.len() {
            0 => None,
            l => self.get(l - 1),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> u64 {
        if let Some(len) = self.length.get() {
            return len;
        }
        let len = self.base.get(&()).unwrap_or(0);
        self.length.set(Some(len));
        len
    }

    pub fn height(&self) -> u8 {
        self.len().next_power_of_two().trailing_zeros() as u8 + 1
    }

    pub fn root_hash(&self) -> Hash {
        self.get_branch(self.root_key()).unwrap_or_default()
    }

    pub fn get_proof(&self, index: u64) -> ListProof<V> {
        self.get_range_proof(index, index + 1)
    }

    pub fn get_range_proof(&self, from: u64, to: u64) -> ListProof<V> {
        if to > self.len() {
            panic!("illegal range boundaries: \
                    the len is {:?}, but the range end is {:?}",
                   self.len(),
                   to)
        }
        if to <= from {
            panic!("illegal range boundaries: \
                    the range start is {:?}, but the range end is {:?}",
                   from,
                   to)
        }

        self.construct_proof(self.root_key(), from, to)
    }

    pub fn iter(&self) -> ProofListIndexIter<V> {
        ProofListIndexIter { base_iter: self.base.iter(&0u8) }
    }

    pub fn iter_from(&self, from: u64) -> ProofListIndexIter<V> {
        ProofListIndexIter { base_iter: self.base.iter_from(&0u8, &ProofListKey::leaf(from)) }
    }
}

impl<'a, V> ProofListIndex<&'a mut Fork, V>
    where V: StorageValue
{
    fn set_len(&mut self, len: u64) {
        self.base.put(&(), len);
        self.length.set(Some(len));
    }

    fn set_branch(&mut self, key: ProofListKey, hash: Hash) {
        debug_assert!(key.height() > 0);

        self.base.put(&key, hash)
    }

    pub fn push(&mut self, value: V) {
        let len = self.len();
        self.set_len(len + 1);
        let mut key = ProofListKey::new(1, len);
        self.base.put(&key, value.hash());
        self.base.put(&ProofListKey::leaf(len), value);
        while key.height() < self.height() {
            let hash = if key.is_left() {
                hash(self.get_branch_unchecked(key).as_ref())
            } else {
                pair_hash(&self.get_branch_unchecked(key.as_left()),
                          &self.get_branch_unchecked(key))
            };
            key = key.parent();
            self.set_branch(key, hash);
        }
    }

    pub fn extend<I>(&mut self, iter: I)
        where I: IntoIterator<Item = V>
    {
        for value in iter {
            self.push(value)
        }
    }

    pub fn set(&mut self, index: u64, value: V) {
        if index >= self.len() {
            panic!("index out of bounds: \
                    the len is {} but the index is {}",
                   self.len(),
                   index);
        }
        let mut key = ProofListKey::new(1, index);
        self.base.put(&key, value.hash());
        self.base.put(&ProofListKey::leaf(index), value);
        while key.height() < self.height() {
            let (left, right) = (key.as_left(), key.as_right());
            let hash = if self.has_branch(right) {
                pair_hash(&self.get_branch_unchecked(left),
                          &self.get_branch_unchecked(right))
            } else {
                hash(self.get_branch_unchecked(left).as_ref())
            };
            key = key.parent();
            self.set_branch(key, hash);
        }
    }

    pub fn clear(&mut self) {
        self.length.set(Some(0));
        self.base.clear()
    }
}

impl<'a, T, V> ::std::iter::IntoIterator for &'a ProofListIndex<T, V>
    where T: AsRef<Snapshot>,
          V: StorageValue
{
    type Item = V;
    type IntoIter = ProofListIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for ProofListIndexIter<'a, V>
    where V: StorageValue
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(_, v)| v)
    }
}
