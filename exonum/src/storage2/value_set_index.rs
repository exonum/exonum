use std::marker::PhantomData;

use crypto::Hash;

use super::{BaseIndex, BaseIndexIter, Result, Snapshot, Fork, StorageKey, StorageValue};

pub struct ValueSetIndex<T, V> {
    base: BaseIndex<T>,
    _v: PhantomData<V>,
}

pub struct ValueSetIndexIter<'a, V> {
    base_iter: BaseIndexIter<'a, Hash, V>
}

pub struct ValueSetIndexHashes<'a> {
    base_iter: BaseIndexIter<'a, Hash, ()>
}

impl<T, V> ValueSetIndex<T, V> {
    pub fn new(prefix: Vec<u8>, base: T) -> Self {
        ValueSetIndex {
            base: BaseIndex::new(prefix, base),
            _v: PhantomData,
        }
    }
}

impl<T, V> ValueSetIndex<T, V> where T: AsRef<Snapshot>,
                                     V: StorageValue {
    pub fn contains(&self, item: &V) -> Result<bool> {
        self.contains_by_hash(&item.hash())
    }

    pub fn contains_by_hash(&self, hash: &Hash) -> Result<bool> {
        self.base.contains(hash)
    }

    pub fn iter(&self) -> ValueSetIndexIter<V> {
        ValueSetIndexIter { base_iter: self.base.iter() }
    }

    pub fn iter_from(&self, from: &Hash) -> ValueSetIndexIter<V> {
        ValueSetIndexIter { base_iter: self.base.iter_from(from) }
    }

    pub fn hashes(&self) -> ValueSetIndexHashes {
        ValueSetIndexHashes { base_iter: self.base.iter() }
    }

    pub fn hashes_from(&self, from: &Hash) -> ValueSetIndexHashes {
        ValueSetIndexHashes { base_iter: self.base.iter_from(from) }
    }
}

impl<T, V> ValueSetIndex<T, V> where T: AsMut<Fork>,
                                     V: StorageValue {
    pub fn insert(&mut self, item: V) {
        self.base.put(&item.hash(), item)
    }

    pub fn delete(&mut self, item: &V) {
        self.delete_by_hash(&item.hash())
    }

    pub fn delete_by_hash(&mut self, hash: &Hash) {
        self.base.delete(hash)
    }
}

impl<'a, V> Iterator for ValueSetIndexIter<'a, V> where V: StorageValue {
    type Item = (Hash, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next()
    }
}

impl<'a> Iterator for ValueSetIndexHashes<'a> {
    type Item = Hash;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}
