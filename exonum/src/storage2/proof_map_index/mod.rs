use std::cell::Cell;
use std::marker::PhantomData;

use crypto::{Hash, hash};

use super::{pair_hash, BaseIndex, BaseIndexIter, Snapshot, Fork, StorageKey, StorageValue};

use self::key::{ProofMapKey, DBKey, ChildKind};
use self::node::{Node, BranchNode};

#[cfg(test)]
mod tests;
mod key;
mod node;
mod proof;

pub struct ProofMapIndex<T, K, V> {
    base: BaseIndex<T>,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<T, K, V> ProofMapIndex<T, K, V> {
    pub fn new(prefix: Vec<u8>, base: T) -> Self {
        ProofMapIndex {
            base: BaseIndex::new(prefix, base),
            _k: PhantomData,
            _v: PhantomData
        }
    }
}


impl<T, K, V> ProofMapIndex<T, K, V> where T: AsRef<Snapshot>,
                                           K: ProofMapKey,
                                           V: StorageValue {
    fn root_prefix(&self) -> Option<Vec<u8>> {
        unimplemented!();
    }

    fn get_node_unchecked(&self, key: DBKey) -> Node<V> {
        // TODO: unwrap?
        match key.is_leaf() {
            true => Node::Leaf(self.base.get(&key).unwrap()),
            false => Node::Branch(self.base.get(&key).unwrap())
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.base.get(&DBKey::leaf(key))
    }

    pub fn contains(&self, key: &K) -> bool {
        self.base.contains(&DBKey::leaf(key))
    }

}

impl<'a, K, V> ProofMapIndex<&'a mut Fork, K, V> where K: ProofMapKey,
                                                       V: StorageValue {
    pub fn put(&self, key: &K, value: V) {
        // self.insert(&v, value)
    }

    pub fn delete(&self, key: &K) {
        // self.remove(DBKey::leaf(&v))
    }

    pub fn clear(&mut self) {
        self.base.clear()
    }
}
