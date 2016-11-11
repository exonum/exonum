use num::{Integer, range, ToPrimitive};

use std::marker::PhantomData;
use std::cell::Cell;

use super::{Map, List, Error, StorageValue, InclusionProof};

use ::crypto::{hash, Hash};

/// Merkle tree over list.
/// Данные в таблице хранятся в строчках,
/// высота определяется количеством записаных значений H = len / 2 +1
/// H  | Элементы
/// 0  | Записанные данные
/// 1  | Хэши от исходных данных
/// 2..| Дерево хешей, где каждая новая высота считает Hash(Hash(h - 1, i), Hash(h - 1, i + 1))

enum NeighbourPosition {
        Left(Hash), 
        Right(Hash)
}

pub struct MerkleTreePath {
    path: Vec<NeighbourPosition>
}

impl<V:StorageValue> InclusionProof<V> for MerkleTreePath {
    fn verify(&self, value : &V, roothash: Hash) -> bool {        
        let mut result = value.hash(); 
        for elem in &self.path {
            result = match *elem {
                     NeighbourPosition::Left(neighbour_hash) => hash(&[neighbour_hash.as_ref(), result.as_ref()].concat()),                     
                     NeighbourPosition::Right(neighbour_hash) => hash(&[result.as_ref(), neighbour_hash.as_ref()].concat()),
                }; 
        }           
        result == roothash
    }
}

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
    pub fn values(&self) -> Result<Vec<V>, Error> {
        Ok(if self.is_empty()? {
            Vec::new()
        } else {
            range(K::zero(), self.len()?).map(|i| self.get(i).unwrap().unwrap()).collect()
        })
    }

    pub fn root_hash(&self) -> Result<Hash, Error> {
        self.get_hash(self.height()?, K::zero())
            .map(|h| h.unwrap_or_else(|| hash(&[])))
    }

    pub fn construct_path_for_index(&self, mut self_index: K) -> Result<MerkleTreePath, Error>{ 
        if self_index >= self.len()? || self_index < K::zero() {
            return Err(Error::new("Wrong index for MerkleTable!"));
        }
        let mut res = MerkleTreePath {
            path: Vec::new(),
        }; 
        let mut current_height = K::one();
        let height = self.height()?; 

        while current_height != height {
            let neighbour_on_left = !self_index.is_even(); 
            let neighbour_index = if !neighbour_on_left {
                self_index + K::one()
            } else {
                self_index - K::one()
            };
            let self_hash = self.get_hash(current_height, self_index)?.unwrap();
            let neighbour_hash = self.get_hash(current_height, neighbour_index)?;
            let path_element = if let Some(neighbour_hash) = neighbour_hash {
                neighbour_hash
            } else {
                self_hash
            }; 
            match neighbour_on_left {
                true => res.path.push(NeighbourPosition::Left(path_element)), 
                false => res.path.push(NeighbourPosition::Right(path_element))
            }
            current_height = current_height + K::one(); 
            self_index = self_index / (K::one() + K::one()) ; 
        } 
        Ok(res)

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

    fn set_len(&self, len: K) -> Result<(), Error> {
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

    fn set_hash(&self, height: K, index: K, bytes: Hash) -> Result<(), Error> {
        // FIXME avoid reallocation
        let vec = bytes.as_ref().to_vec();
        let key = Self::db_key(height, index);
        self.map.put(&key, vec)
    }

    fn append_hash(&self, mut index: K, bytes: Hash) -> Result<(), Error> {
        if index != self.len()? {
            return Err(Error::new("Appending hash not to the end of list in MerkleTable")); 
        }
        self.set_hash(K::one(), index, bytes)?;
        let mut current_height = K::one();
        while index != K::zero() {
            // Left leaf, Right leaf is empty
            let new_hash = if index.is_even() {
                let h1 = self.get_hash(current_height, index)?.unwrap(); 
                hash(&[h1.as_ref(), h1.as_ref()].concat())
                // TODO replace by error
                
            } else {
                // Right leaf
                let h1 = self.get_hash(current_height, index - K::one())?.unwrap();
                let h2 = self.get_hash(current_height, index)?.unwrap();
                hash(&[h1.as_ref(), h2.as_ref()].concat())
            };
            current_height = current_height + K::one();
            index = index / (K::one() + K::one());
            self.set_hash(current_height, index, new_hash)?;
        }
        Ok(())
    }

    fn update_hash_subtree(&self, mut index: K, bytes: Hash) -> Result<(), Error> {
        self.set_hash(K::one(), index, bytes)?;

        let height = self.height()?;
        let mut current_height = K::one();
        while current_height != height {
            let i = if !index.is_even() {
                index - K::one()
            } else {
                index
            };

            let h1 = self.get_hash(current_height, i)?.unwrap();
            let h2 = self.get_hash(current_height, i + K::one())?;
            let new_hash = if let Some(h2) = h2 {
                hash(&[h1.as_ref(), h2.as_ref()].concat())
            } else {
                hash(&[h1.as_ref(), h1.as_ref()].concat())                
            };

            current_height = current_height + K::one();
            index = index / (K::one() + K::one());
            self.set_hash(current_height, index, new_hash)?;
        }
        Ok(())
    }
}

impl<T, K: ?Sized, V> List<K, V> for MerkleTable<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Integer + Copy + Clone + ToPrimitive + StorageValue,
          V: StorageValue
{
    fn append(&self, value: V) -> Result<(), Error> {
        let len = self.len()?;
        self.append_hash(len, value.hash())?;

        self.map.put(&Self::db_key(K::zero(), len), value.serialize())?;
        self.set_len(len + K::one())?;
        Ok(())
    }

    fn extend<I>(&self, iter: I) -> Result<(), Error>
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

    fn set(&self, index: K, value: V) -> Result<(), Error> {
        if index >= self.len()? || index < K::zero() {
            return Err(Error::new("Wrong index!"));
        }

        self.update_hash_subtree(index, value.hash())?;
        self.map.put(&Self::db_key(K::zero(), index), value.serialize())
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
        let c = v.map_or_else(K::zero, K::deserialize);
        self.count.set(Some(c));
        Ok(c)
    }
}



#[cfg(test)]
mod tests {
    extern crate rand;

    use ::crypto::hash;
    use ::storage::{MemoryDB, List, MapTable, MerkleTable, InclusionProof};


    #[test]
    fn test_list_methods() {
        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));

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
    fn test_height() {
        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));

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

        table.set(1, vec![10]).unwrap();
        assert_eq!(table.get(1).unwrap(), Some(vec![10]));
    }

    #[test]
    fn test_hashes() {
        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));
        assert_eq!(table.root_hash().unwrap(), hash(&[]));

        let h1 = hash(&[1]);
        let h2 = hash(&[2]);
        let h3 = hash(&[3]);
        let h4 = hash(&[4]);
        let h5 = hash(&[5]);
        let h6 = hash(&[6]);
        let h7 = hash(&[7]);
        let h8 = hash(&[8]);
        let h12 = hash(&[h1.as_ref(), h2.as_ref()].concat());
        let h33 = hash(&[h3.as_ref(), h3.as_ref()].concat());
        let h123 = hash(&[h12.as_ref(), h33.as_ref()].concat());

        let h34 = hash(&[h3.as_ref(), h4.as_ref()].concat());
        let h1234 = hash(&[h12.as_ref(), h34.as_ref()].concat());

        let h55 = hash(&[h5.as_ref(), h5.as_ref()].concat());
        let h5555 = hash(&[h55.as_ref(), h55.as_ref()].concat());
        let h12345 = hash(&[h1234.as_ref(), h5555.as_ref()].concat());

        let h56 = hash(&[h5.as_ref(), h6.as_ref()].concat());
        let h5656 = hash(&[h56.as_ref(), h56.as_ref()].concat());
        let h123456 = hash(&[h1234.as_ref(), h5656.as_ref()].concat());

        let h78 = hash(&[h7.as_ref(), h8.as_ref()].concat());
        let h77 = hash(&[h7.as_ref(), h7.as_ref()].concat());
        let h567 = hash(&[h56.as_ref(), h77.as_ref()].concat());
        let h1234567 = hash(&[h1234.as_ref(), h567.as_ref()].concat());

        let h5678 = hash(&[h56.as_ref(), h78.as_ref()].concat());
        let h12345678 = hash(&[h1234.as_ref(), h5678.as_ref()].concat());

        table.append(vec![1]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h1);

        table.append(vec![2]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h12);

        table.append(vec![3]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h123);

        table.append(vec![4]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h1234);

        let path_of_third = table.construct_path_for_index(2).unwrap(); 
        assert!(path_of_third.verify(&vec![3], table.root_hash().unwrap())); 

        table.append(vec![5]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h12345);

        table.append(vec![6]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h123456);

        table.append(vec![7]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h1234567);

        table.append(vec![8]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h12345678);

        assert_eq!(table.get(0u32).unwrap(), Some(vec![1]));
    }

    #[test]
    fn test_hash_in_values() {
        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));

        let h = hash(&[1, 2, 3, 4]);
        table.append(h).unwrap();
        assert_eq!(table.get(0u32).unwrap(), Some(h));
    }

    #[test]
    fn test_hash_set_value_simple() {
        let h1 = hash(&[1]);
        let h2 = hash(&[2]);

        let s = MemoryDB::new();
        let t = MerkleTable::new(MapTable::new(vec![255], &s));
        assert_eq!(t.get(0u32).unwrap(), None);
        t.append(vec![1]).unwrap();
        assert_eq!(t.root_hash().unwrap(), h1);

        t.set(0, vec![2]).unwrap();
        assert_eq!(t.root_hash().unwrap(), h2);
    }

    #[test]
    fn test_hash_set_value() {
        let s1 = MemoryDB::new();
        let t1 = MerkleTable::new(MapTable::new(vec![255], &s1));
        assert_eq!(t1.get(0u32).unwrap(), None);
        t1.append(vec![1]).unwrap();
        t1.append(vec![2]).unwrap();
        t1.append(vec![3]).unwrap();
        t1.append(vec![4]).unwrap();

        t1.set(0, vec![4]).unwrap();
        t1.set(1, vec![7]).unwrap();
        t1.set(2, vec![5]).unwrap();
        t1.set(3, vec![1]).unwrap();

        let s2 = MemoryDB::new();
        let t2 = MerkleTable::new(MapTable::new(vec![255], &s2));
        assert_eq!(t2.get(0u32).unwrap(), None);
        t2.append(vec![4]).unwrap();
        t2.append(vec![7]).unwrap();
        t2.append(vec![5]).unwrap();
        t2.append(vec![1]).unwrap();

        assert_eq!(t1.root_hash().unwrap(), t2.root_hash().unwrap());
    }
}
