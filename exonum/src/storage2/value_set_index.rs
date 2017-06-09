use std::marker::PhantomData;

use crypto::Hash;

use super::{BaseIndex, BaseIndexIter, Snapshot, Fork, StorageValue};

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
    pub fn contains(&self, item: &V) -> bool {
        self.contains_by_hash(&item.hash())
    }

    pub fn contains_by_hash(&self, hash: &Hash) -> bool {
        self.base.contains(hash)
    }

    pub fn iter(&self) -> ValueSetIndexIter<V> {
        ValueSetIndexIter { base_iter: self.base.iter(&()) }
    }

    pub fn iter_from(&self, from: &Hash) -> ValueSetIndexIter<V> {
        ValueSetIndexIter { base_iter: self.base.iter_from(&(), from) }
    }

    pub fn hashes(&self) -> ValueSetIndexHashes {
        ValueSetIndexHashes { base_iter: self.base.iter(&()) }
    }

    pub fn hashes_from(&self, from: &Hash) -> ValueSetIndexHashes {
        ValueSetIndexHashes { base_iter: self.base.iter_from(&(), from) }
    }
}

impl<'a, V> ValueSetIndex<&'a mut Fork, V> where V: StorageValue {
    pub fn insert(&mut self, item: V) {
        self.base.put(&item.hash(), item)
    }

    pub fn remove(&mut self, item: &V) {
        self.remove_by_hash(&item.hash())
    }

    pub fn remove_by_hash(&mut self, hash: &Hash) {
        self.base.remove(hash)
    }

    pub fn clear(&mut self) {
        self.base.clear()
    }
}

impl<'a, T, V> ::std::iter::IntoIterator for &'a ValueSetIndex<T, V> where T: AsRef<Snapshot>,
                                                                           V: StorageValue {
    type Item = (Hash, V);
    type IntoIter = ValueSetIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
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
