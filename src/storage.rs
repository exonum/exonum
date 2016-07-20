use std::slice;
use std::slice::SliceConcatExt;
use std::convert::AsRef;
use std::marker::PhantomData;
use std::cell::Cell;
use std::num::{Zero, One};
use std::ops::{Add, Sub};
use std::sync::Arc;
use std::collections::BTreeMap;

use byteorder::{ByteOrder, BigEndian};

use ::crypto::Hash;
use ::messages::{MessageBuffer, Message, TxMessage, Precommit, Propose};

pub struct Storage<T: Database> {
    db: T
}

const TRANSACTION_PREFIX: &'static [u8] = &[00];
const PROPOSES_PREFIX: &'static [u8] = &[01];
const PRECOMMIT_PREFIX: &'static [u8] = &[02];
const HEIGHT_PREFIX: &'static [u8] = &[03];

impl<T> Storage<T> where T: Database {
    pub fn new(db: T) -> Storage<T> {
        Storage {
            db: db
        }
    }

    pub fn transactions<'a>(&'a mut self) -> PrefixMap<'a, T, Hash, TxMessage> {
        self.db.map(vec![00])
    }

    pub fn proposes<'a>(&'a mut self) -> PrefixMap<'a, T, Hash, Propose> {
        self.db.map(vec![01])
    }

    pub fn heights<'a>(&'a mut self) -> MappedList<PrefixMap<'a, T, [u8], Vec<u8>>, u64, Hash> {
        self.db.list(vec![02])
    }

    pub fn last_hash(&mut self) -> Option<Hash> {
        self.heights().last()
    }

    pub fn last_propose(&mut self) -> Option<Propose> {
        match self.last_hash() {
            Some(hash) => Some(self.proposes().get(&hash).unwrap()),
            None => None
        }

    }

    pub fn precommits<'a>(&'a mut self, hash: &'a Hash) -> MappedList<PrefixMap<'a, T, [u8], Vec<u8>>, u32, Precommit> {
        self.db.list([PRECOMMIT_PREFIX, hash.as_ref()].concat())
    }

    pub fn fork<'a>(&'a self) -> Storage<Fork<'a, T>> {
        Storage {
            db: self.db.fork()
        }
    }

    pub fn merge(&mut self, patch: Patch) {
        self.db.merge(patch)
    }
}

impl<'a, T> Storage<Fork<'a, T>> where T: Database {
    pub fn patch(self) -> Patch {
        self.db.patch()
    }
}

pub trait Database: Map<[u8], Vec<u8>>+Sized {
    fn fork<'a>(&'a self) -> Fork<'a, Self>;
    fn merge(&mut self, patch: Patch);
}

pub enum Change {
    Put(Vec<u8>),
    Delete,
}

pub struct Patch {
    changes: BTreeMap<Vec<u8>, Change>
}

pub struct Fork<'a, T: Database + 'a> {
    database: &'a T,
    changes: BTreeMap<Vec<u8>, Change>
}

impl<'a, T: Database + 'a> Fork<'a, T> {
    pub fn patch(self) -> Patch {
        Patch {
            changes: self.changes
        }
    }
}

impl<'a, T> Map<[u8], Vec<u8>> for Fork<'a, T> where T: Database + 'a {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self.changes.get(key) {
            Some(change) => match *change {
                Change::Put(ref v) => Some(v.clone()),
                Change::Delete => None,
            },
            None => self.database.get(key)
        }
    }

    fn put(&mut self, key: &[u8], value: Vec<u8>) {
        self.changes.insert(key.to_vec(), Change::Put(value));
    }

    fn delete(&mut self, key: &[u8]) {
        self.changes.insert(key.to_vec(), Change::Delete);
    }
}

impl<'a, T: Database + 'a + ?Sized> Database for Fork<'a, T> {
    fn fork<'b>(&'b self) -> Fork<'b, Self> {
        Fork {
            database: self,
            changes: BTreeMap::new()
        }
    }

    fn merge(&mut self, patch: Patch) {
        self.changes.extend(patch.changes.into_iter());
    }
}

pub struct MemoryDatabase {
    map: BTreeMap<Vec<u8>, Vec<u8>>
}

impl MemoryDatabase {
    pub fn new() -> MemoryDatabase {
        MemoryDatabase {
            map: BTreeMap::new()
        }
    }
}

impl Map<[u8], Vec<u8>> for MemoryDatabase {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.map.get(key).map(Clone::clone)
    }

    fn put(&mut self, key: &[u8], value: Vec<u8>) {
        self.map.insert(key.to_vec(), value);
    }

    fn delete(&mut self, key: &[u8]) {
        self.map.remove(key);
    }
}

impl Database for MemoryDatabase {
    fn fork<'a>(&'a self) -> Fork<'a, Self> {
        Fork {
            database: self,
            changes: BTreeMap::new()
        }
    }

    fn merge(&mut self, patch: Patch) {
        for (key, change) in patch.changes.iter() {
            match *change {
                Change::Put(ref v) => {
                    self.map.insert(key.clone(), v.clone());
                },
                Change::Delete => {
                    self.map.remove(key);
                }
            }
        }
    }
}

pub trait StorageValue {
    fn serialize(self) -> Vec<u8>;
    fn deserialize(v: Vec<u8>) -> Self;
}

impl StorageValue for u32 {
    // TODO: return Cow<[u8]>
    fn serialize(self) -> Vec<u8> {
        let mut v = Vec::new();
        BigEndian::write_u32(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_u32(&v)
    }
}

impl StorageValue for u64 {
    fn serialize(self) -> Vec<u8> {
        let mut v = Vec::new();
        BigEndian::write_u64(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_u64(&v)
    }
}

impl StorageValue for Hash {
    fn serialize(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Hash::from_slice(&v).unwrap()
    }
}

impl<T> StorageValue for T where T: Message {
    fn serialize(self) -> Vec<u8> {
        self.raw().as_ref().as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Message::from_raw(Arc::new(MessageBuffer::from_vec(v))).unwrap()
    }
}

impl StorageValue for TxMessage {
    fn serialize(self) -> Vec<u8> {
        self.raw().as_ref().as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        TxMessage::from_raw(Arc::new(MessageBuffer::from_vec(v))).unwrap()
    }

}



impl StorageValue for Vec<u8> {
    fn serialize(self) -> Vec<u8> {
        self
    }

    fn deserialize(v: Vec<u8>) -> Self {
        v
    }
}

pub trait Map<K: ?Sized, V> {
    fn get(&self, key: &K) -> Option<V>;
    fn put(&mut self, key: &K, value: V);
    fn delete(&mut self, key: &K);
}

pub struct PrefixMap<'a, T: Map<[u8], Vec<u8>> + 'a, K: ?Sized, V> {
    prefix: Vec<u8>,
    storage: &'a mut T,
    _k: PhantomData<K>,
    _v: PhantomData<V>
}

impl<'a, T, K: ?Sized, V> Map<K, V> for PrefixMap<'a, T, K, V>
        where T: Map<[u8], Vec<u8>>,
              K: AsRef<[u8]>,
              V: StorageValue {
    fn get(&self, key: &K) -> Option<V> {
        self.storage.get(&[&self.prefix, key.as_ref()].concat()).map(StorageValue::deserialize)
    }

    fn put(&mut self, key: &K, value: V) {
        self.storage.put(&[&self.prefix, key.as_ref()].concat(), value.serialize())
    }

    fn delete(&mut self, key: &K) {
        self.storage.delete(&[&self.prefix, key.as_ref()].concat())
    }
}

trait MapExt : Map<[u8], Vec<u8>> + Sized {
    fn list<'a, K, V>(&'a mut self, prefix: Vec<u8>) -> MappedList<PrefixMap<'a, Self, [u8], Vec<u8>>, K, V>
        where K: Zero+One+Add<Output=K>+Copy+StorageValue,
              V: StorageValue;

    fn map<'a, K: ?Sized, V>(&'a mut self, prefix: Vec<u8>) -> PrefixMap<'a, Self, K, V>;
}

impl<T> MapExt for T where T: Map<[u8], Vec<u8>> + Sized {
    fn list<'a, K, V>(&'a mut self, prefix: Vec<u8>) -> MappedList<PrefixMap<'a, Self, [u8], Vec<u8>>, K, V>
        where K: Copy+StorageValue,
              V: StorageValue {
        MappedList {
            map: self.map(prefix),
            count: Cell::new(None),
            _v: PhantomData
        }
    }

    fn map<'a, K: ?Sized, V>(&'a mut self, prefix: Vec<u8>) -> PrefixMap<'a, Self, K, V> {
        PrefixMap {
            prefix: prefix,
            storage: self,
            _k: PhantomData,
            _v: PhantomData
        }
    }
}

pub struct MappedList<T: Map<[u8], Vec<u8>>, K, V> {
    map: T,
    count: Cell<Option<K>>,
    _v: PhantomData<V>
}

impl<'a, T, K, V> MappedList<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Zero+One+Add<Output=K>+Sub<Output=K>+PartialEq+Copy+StorageValue,
          ::std::ops::Range<K>: ::std::iter::Iterator<Item=K>,
          V: StorageValue {
    pub fn append(&mut self, value: V) {
        let len = self.len();
        self.map.put(&len.serialize(), value.serialize());
        self.map.put(&[], (len+One::one()).serialize());
        self.count.set(Some(len+One::one()));
    }

    pub fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=V> {
        let mut len = self.len();
        for value in iter {
            self.map.put(&len.serialize(), value.serialize());
            len = len + One::one();
        }
        self.map.put(&[], (len+One::one()).serialize());
        self.count.set(Some(len+One::one()));
    }

    pub fn get(&self, index: K) -> Option<V> {
        self.map.get(&index.serialize()).map(StorageValue::deserialize)
    }

    pub fn last(&self) -> Option<V> {
        if self.is_empty() {
            None
        } else {
            self.get(self.len() - One::one())
        }
    }

    // TODO: implement iterator for List
    pub fn iter(&self) -> Option<Vec<V>> {
        if self.is_empty() {
            None
        } else {
            Some((Zero::zero()..self.len()).map(|i| self.get(i).unwrap()).collect())
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == Zero::zero()
    }

    pub fn len(&self) -> K {
        if let Some(count) = self.count.get() {
            return count;
        }

        let c = self.map.get(&[]).map(K::deserialize).unwrap_or(Zero::zero());
        self.count.set(Some(c));
        c
    }
}
