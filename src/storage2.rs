use std::slice;
use std::slice::SliceConcatExt;
use std::convert::AsRef;
use std::marker::PhantomData;
use std::cell::Cell;
use std::num::{Zero, One};
use std::ops::{Add, Sub};
use std::sync::Arc;

use byteorder::{ByteOrder, BigEndian};

use ::crypto::Hash;
use ::messages::{MessageBuffer, Message, TxMessage, Precommit, Propose};

pub struct Storage<T: Map<[u8], Vec<u8>>> {
    db: T
}

const TRANSACTION_PREFIX: &'static [u8] = &[00];
const PROPOSES_PREFIX: &'static [u8] = &[01];
const PRECOMMIT_PREFIX: &'static [u8] = &[02];
const HEIGHT_PREFIX: &'static [u8] = &[03];

impl<T> Storage<T> where T: Map<[u8], Vec<u8>> {

//     fn block_hash(&self) -> Hash;

//     fn height(&self) -> Height;

//     fn prev_hash(&self) -> Hash {
//         self.get_block(self.height().height()).unwrap()
//     }

//     fn prev_time(&self) -> Timespec {
//         // TODO: Possibly inefficient
//         self.get_propose(&self.prev_hash()).unwrap().time()
//     }

    fn transactions<'a>(&'a mut self) -> PrefixMap<'a, T, Hash, TxMessage> {
        self.db.map(vec![00])
    }

    fn proposes<'a>(&'a mut self) -> PrefixMap<'a, T, Hash, Propose> {
        self.db.map(vec![01])
    }

    fn heights<'a>(&'a mut self) -> MappedList<PrefixMap<'a, T, [u8], Vec<u8>>, u64, Hash> {
        self.db.list(vec![02])
    }

    fn last_propose(&mut self) -> Option<Propose> {
        match self.heights().last() {
            Some(hash) => self.proposes().get(&hash),
            None => None
        }
    }

    fn precommits<'a>(&'a mut self, hash: &'a Hash) -> MappedList<PrefixMap<'a, T, [u8], Vec<u8>>, u32, Precommit> {
        self.db.list([PRECOMMIT_PREFIX, hash.as_ref()].concat())
    }

//     fn merge(&mut self, patch: &Patch);
}

pub trait StorageValue {
    fn serialize(self) -> Vec<u8>;
    fn deserialize(v: Vec<u8>) -> Self;
}

// impl<T> StorageValue for T where T: From<Vec<u8>> + Into<Vec<u8>> {
//     fn serialize(self) -> Vec<u8> {
//         self.into()
//     }

//     fn deserialize(v: Vec<u8>) -> Self {
//         Self::from(v)
//     }
// }

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

    fn prefix<'a>(&'a mut self, prefix: Vec<u8>) -> PrefixMap<'a, Self, [u8], Vec<u8>>;
}

impl<T> MapExt for T where T: Map<[u8], Vec<u8>> + Sized {
    fn list<'a, K, V>(&'a mut self, prefix: Vec<u8>) -> MappedList<PrefixMap<'a, Self, [u8], Vec<u8>>, K, V>
        where K: Copy+StorageValue,
              V: StorageValue {
        MappedList {
            map: self.prefix(prefix),
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

    fn prefix<'a>(&'a mut self, prefix: Vec<u8>) -> PrefixMap<'a, Self, [u8], Vec<u8>> {
        PrefixMap {
            prefix: prefix,
            storage: self,
            _k: PhantomData,
            _v: PhantomData
        }
    }
}

// impl<T> T where T: Map<[u8], Vec<u8>> {
//     fn list<'a, K, V>(&'a self) -> MappedList<T, K, V> {
//     }
// }

// pub struct Value<'a, T: Map<[u8], Vec<u8>> + 'a, V> {
//     storage: &'a mut T,
//     _v: PhantomData<V>,
// }

// impl<'a, T, V> Value<'a, T, V> {
//     fn get(&self)
// }

pub struct MappedList<T: Map<[u8], Vec<u8>>, K, V> {
    map: T,
    count: Cell<Option<K>>,
    _v: PhantomData<V>
}

impl<'a, T, K, V> MappedList<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Zero+One+Add<Output=K>+Sub<Output=K>+PartialEq+Copy+StorageValue,
          V: StorageValue {
    fn append(&mut self, value: V) {
        let len = self.len();
        self.map.put(&len.serialize(), value.serialize());
        self.map.put(&[], (len+One::one()).serialize());
        self.count.set(Some(len+One::one()));
    }

    fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=V> {
        let mut len = self.len();
        for value in iter {
            self.map.put(&len.serialize(), value.serialize());
            len = len + One::one();
        }
        self.map.put(&[], (len+One::one()).serialize());
        self.count.set(Some(len+One::one()));
    }

    fn get(&self, index: K) -> Option<V> {
        self.map.get(&index.serialize()).map(StorageValue::deserialize)
    }

    fn last(&self) -> Option<V> {
        if self.len() == Zero::zero() {
            None
        } else {
            self.get(self.len() - One::one())
        }
    }

//     fn iter() {

//     }

    fn len(&self) -> K {
        if let Some(count) = self.count.get() {
            return count;
        }

        let c = self.map.get(&[]).map(K::deserialize).unwrap_or(Zero::zero());
        self.count.set(Some(c));
        c
    }
}


