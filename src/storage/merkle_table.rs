use num::{Integer, range, ToPrimitive};

use std::marker::PhantomData;
use std::cell::Cell;

use super::{Map, List, Error, StorageValue};

use ::crypto::{hash, Hash};

/// Merkle tree over list.
/// Данные в таблице хранятся в строчках,
/// высота определяется количеством записаных значений H = len / 2 +1
/// H  | Элементы
/// 0  | Записанные данные
/// 1  | Хэши от исходных данных
/// 2..| Дерево хешей, где каждая новая высота считает Hash(Hash(h - 1, i), Hash(h - 1, i + 1))
pub struct MerkleTable<T: Map<[u8], Vec<u8>>, K, V> {
    map: T,
    count: Cell<Option<K>>,
    _v: PhantomData<V>,
}

impl<'a, T, K, V> MerkleTable<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Integer + Copy + Clone + ToPrimitive + StorageValue,
          V: StorageValue
{
    pub fn new(map: T) -> Self {
        MerkleTable {
            map: map,
            count: Cell::new(None),
            _v: PhantomData,
        }
    }

    // TODO: implement iterator for List
    pub fn iter(&self) -> Result<Option<Vec<V>>, Error> {
        Ok(if self.is_empty()? {
            None
        } else {
            Some(range(K::zero(), self.len()?).map(|i| self.get(i).unwrap().unwrap()).collect())
        })
    }

    pub fn root_hash(&self) -> Result<Option<Hash>, Error> {
        self.get_hash(self.height()?, K::zero())
    }

    fn height(&self) -> Result<K, Error> {
        let len = self.len()?;
        let h = Self::upper_power_of_two(len) + K::one();
        Ok(h)
    }

    // TODO replace by library method
    fn upper_power_of_two(v: K) -> K {
        let mut p = K::one();
        let mut i = K::zero();
        while p < v {
            p = p * (K::one() + K::one());
            i = i + K::one();
        }
        i
    }

    fn set_len(&mut self, len: K) -> Result<(), Error> {
        self.count.set(Some(len));
        self.map.put(&[], len.serialize())
    }

    // TODO reduce reallocations. We can create a key by one allocation.
    fn db_key(h: K, i: K) -> Vec<u8> {
        [h.serialize(), i.serialize()].concat()
    }

    fn get_hash(&self, height: K, index: K) -> Result<Option<Hash>, Error> {
        debug_assert!(height > K::zero());

        let v = self.map.get(&Self::db_key(height, index))?;
        let hash = v.map(|x| {
            debug_assert_eq!(x.len(), 32);
            Hash::from_slice(&x).unwrap()
        });
        Ok(hash)
    }

    fn rebuild_hash(&mut self, mut index: K, bytes: &Vec<u8>) -> Result<(), Error> {
        // FIXME avoid reallocation
        self.map.put(&Self::db_key(K::one(), index), bytes.hash().as_ref().to_vec())?;
        let mut current_height = K::one();
        while index != K::zero() {
            // Left leaf, Right leaf is empty
            let new_hash = if index.is_even() {
                let h1 = self.get_hash(current_height, index)?.unwrap(); //TODO replace by error
                h1
            } else {
                // Right leaf
                let h1 = self.get_hash(current_height, index - K::one())?.unwrap();
                let h2 = self.get_hash(current_height, index)?.unwrap();
                hash(&[h1.as_ref(), h2.as_ref()].concat())
            };
            current_height = current_height + K::one();
            index = index / (K::one() + K::one());
            self.map.put(&Self::db_key(current_height, index), new_hash.serialize())?;
        }
        Ok(())
    }
}

impl<T, K: ?Sized, V> List<K, V> for MerkleTable<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Integer + Copy + Clone + ToPrimitive + StorageValue,
          V: StorageValue
{
    fn append(&mut self, value: V) -> Result<(), Error> {
        let len = self.len()?;
        let bytes = value.serialize();
        self.rebuild_hash(len, &bytes)?;

        self.map.put(&Self::db_key(K::zero(), len), bytes)?;
        self.set_len(len + K::one())?;
        Ok(())
    }

    fn extend<I>(&mut self, iter: I) -> Result<(), Error>
        where I: IntoIterator<Item = V>
    {
        for value in iter {
            self.append(value)?;
        }
        Ok(())
    }

    fn get(&self, index: K) -> Result<Option<V>, Error> {
        let value = self.map.get(&Self::db_key(K::zero(), index))?;
        Ok(value.map(StorageValue::deserialize))
    }

    fn last(&self) -> Result<Option<V>, Error> {
        let len = self.len()?;
        if len == K::zero() {
            Ok(None)
        } else {
            self.get(len - K::one())
        }
    }

    fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == K::zero())
    }

    fn len(&self) -> Result<K, Error> {
        if let Some(count) = self.count.get() {
            return Ok(count);
        }

        let v = self.map.get(&[])?;
        let c = v.map(K::deserialize).unwrap_or(K::zero());
        self.count.set(Some(c));
        Ok(c)
    }
}



#[cfg(test)]
mod tests {
    extern crate rand;

    use ::crypto::hash;
    use ::storage::{MemoryDB, MapExt, List};


    #[test]
    fn list_methods() {
        let mut storage = MemoryDB::new();
        let mut table = storage.merkle_list(vec![255]);

        assert!(table.is_empty().unwrap());
        assert_eq!(table.len().unwrap(), 0);
        table.append(vec![1]).unwrap();
        assert!(!table.is_empty().unwrap());
        assert_eq!(table.len().unwrap(), 1);

        table.append(vec![2]).unwrap();
        assert_eq!(table.len().unwrap(), 2);

        table.append(vec![3]).unwrap();
        assert_eq!(table.len().unwrap(), 3);

        assert_eq!(table.get(0u32).unwrap(), Some(vec![1]));
        assert_eq!(table.get(1).unwrap(), Some(vec![2]));
        assert_eq!(table.get(2).unwrap(), Some(vec![3]));
    }

    #[test]
    fn height() {
        let mut storage = MemoryDB::new();
        let mut table = storage.merkle_list(vec![255]);

        table.append(vec![1]).unwrap();
        assert_eq!(table.height().unwrap(), 1);

        table.append(vec![2]).unwrap();
        assert_eq!(table.height().unwrap(), 2);

        table.append(vec![3]).unwrap();
        assert_eq!(table.height().unwrap(), 3);

        table.append(vec![4]).unwrap();
        assert_eq!(table.height().unwrap(), 3);

        assert_eq!(table.len().unwrap(), 4);
        assert_eq!(table.get(0u32).unwrap(), Some(vec![1]));
        assert_eq!(table.get(1).unwrap(), Some(vec![2]));
        assert_eq!(table.get(2).unwrap(), Some(vec![3]));
        assert_eq!(table.get(3).unwrap(), Some(vec![4]));
    }

    #[test]
    fn hashes() {
        let mut storage = MemoryDB::new();
        let mut table = storage.merkle_list(vec![255]);
        assert_eq!(table.root_hash().unwrap(), None);

        let h1 = hash(&vec![1]);
        let h2 = hash(&vec![2]);
        let h3 = hash(&vec![3]);
        let h4 = hash(&vec![4]);
        let h5 = hash(&vec![5]);
        let h6 = hash(&vec![6]);
        let h7 = hash(&vec![7]);
        let h8 = hash(&vec![8]);
        let h12 = hash(&[h1.as_ref(), h2.as_ref()].concat());
        let h123 = hash(&[h12.as_ref(), h3.as_ref()].concat());
        let h34 = hash(&[h3.as_ref(), h4.as_ref()].concat());
        let h1234 = hash(&[h12.as_ref(), h34.as_ref()].concat());
        let h12345 = hash(&[h1234.as_ref(), h5.as_ref()].concat());
        let h56 = hash(&[h5.as_ref(), h6.as_ref()].concat());
        let h123456 = hash(&[h1234.as_ref(), h56.as_ref()].concat());
        let h78 = hash(&[h7.as_ref(), h8.as_ref()].concat());
        let h567 = hash(&[h56.as_ref(), h7.as_ref()].concat());
        let h1234567 = hash(&[h1234.as_ref(), h567.as_ref()].concat());
        let h5678 = hash(&[h56.as_ref(), h78.as_ref()].concat());
        let h12345678 = hash(&[h1234.as_ref(), h5678.as_ref()].concat());

        table.append(vec![1]).unwrap();
        assert_eq!(table.root_hash().unwrap(), Some(h1));

        table.append(vec![2]).unwrap();
        assert_eq!(table.root_hash().unwrap(), Some(h12));

        table.append(vec![3]).unwrap();
        assert_eq!(table.root_hash().unwrap(), Some(h123));

        table.append(vec![4]).unwrap();
        assert_eq!(table.root_hash().unwrap(), Some(h1234));

        table.append(vec![5]).unwrap();
        assert_eq!(table.root_hash().unwrap(), Some(h12345));

        table.append(vec![6]).unwrap();
        assert_eq!(table.root_hash().unwrap(), Some(h123456));

        table.append(vec![7]).unwrap();
        assert_eq!(table.root_hash().unwrap(), Some(h1234567));

        table.append(vec![8]).unwrap();
        assert_eq!(table.root_hash().unwrap(), Some(h12345678));

        assert_eq!(table.get(0u32).unwrap(), Some(vec![1]));
    }

    #[test]
    fn hash_in_values() {
        let mut storage = MemoryDB::new();
        let mut table = storage.merkle_list(vec![255]);

        let h = hash(&[1, 2, 3 ,4]);
        table.append(h).unwrap();
        assert_eq!(table.get(0u32).unwrap(), Some(h));
    }
}
