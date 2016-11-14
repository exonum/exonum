use num::{Integer, range, ToPrimitive, pow};

use std::marker::PhantomData;
use std::cell::Cell;

use super::{Map, List, Error, StorageValue, InclusionProof};

use ::crypto::{hash, Hash};
use std::collections::HashMap;

type Range<K> = Option<(K, K)>;
/// Merkle tree over list.
/// Данные в таблице хранятся в строчках,
/// высота определяется количеством записаных значений H = len / 2 +1
/// H  | Элементы
/// 0  | Записанные данные
/// 1  | Хэши от исходных данных
/// 2..| Дерево хешей, где каждая новая высота считает Hash(Hash(h - 1, i), Hash(h - 1, i + 1))
#[derive(Copy, Clone)]
enum NeighbourPosition {
    Left,
    Right,
}

pub struct MerkleRangeProof<K> {
    tree_height: K,
    range_start: K,
    // end - exclusive
    range_end: K,
    merkle_elements_table: HashMap<K, (Option<Hash>, NeighbourPosition)>,
}

impl<K> MerkleRangeProof<K>
    where K: Integer + Copy + Clone + ToPrimitive + StorageValue + ::std::hash::Hash
{
    fn new(max_height: K, range_start: K, range_end: K) -> MerkleRangeProof<K> {
        MerkleRangeProof {
            tree_height: max_height,
            range_start: range_start,
            range_end: range_end,
            merkle_elements_table: HashMap::with_capacity(max_height.to_usize().unwrap()),
        }
    }

    fn construct_proof_subtree<T, V>(&mut self,
                                     node_height: K,
                                     node_index: K,
                                     range_start: K,
                                     range_end: K,
                                     source: &MerkleTable<T, K, V>)
                                     -> Result<(), Error>
        where T: Map<[u8], Vec<u8>>,
              V: StorageValue
    {
        if node_height > K::zero() {
            if node_height < self.tree_height {
                let (hash_option, position) = source.get_neighbour_hash(node_height, node_index)?;
                let ordinal =
                    Self::convert_to_ordinal_number(self.tree_height, node_height, node_index);
                self.merkle_elements_table.insert(ordinal, (hash_option, position));
            }
            if node_height > K::one() {
                let subtree_hight = node_height - K::one();
                let left_child_index = node_index * (K::one() + K::one());
                let righ_child_index = node_index * (K::one() + K::one()) + K::one();
                let right_subtree_first_index =
                    Self::index_of_first_element_in_subtree(subtree_hight, righ_child_index);
                let (left_range, right_range) =
                    Self::split_range(range_start, right_subtree_first_index, range_end);
                match (left_range, right_range) {
                    (Some((l_s, l_e)), Some((r_s, r_e))) => {
                        self.construct_proof_subtree(subtree_hight,
                                                     left_child_index,
                                                     l_s,
                                                     l_e,
                                                     source)?;
                        self.construct_proof_subtree(subtree_hight,
                                                     righ_child_index,
                                                     r_s,
                                                     r_e,
                                                     source)?;
                    } 
                    (Some((l_s, l_e)), None) => {
                        self.construct_proof_subtree(subtree_hight,
                                                     left_child_index,
                                                     l_s,
                                                     l_e,
                                                     source)?;
                    }
                    (None, Some((r_s, r_e))) => {
                        self.construct_proof_subtree(subtree_hight,
                                                     righ_child_index,
                                                     r_s,
                                                     r_e,
                                                     source)?;
                    }
                    (None, None) => {
                        unreachable!();
                    }

                };
            }
            Ok(())
        } else {
            unreachable!();
        }
    }

    fn split_range(start: K, middle: K, end: K) -> (Range<K>, Range<K>) {
        debug_assert!(start < end);
        if middle >= end {
            (Some((start, end)), None)
        } else if middle <= start {
            (None, Some((start, end)))
        } else {
            // start < middle < end
            (Some((start, middle)), Some((middle, end)))
        }
    }

    fn index_of_first_element_in_subtree(subtree_root_height: K, subtree_root_index: K) -> K {
        pow(K::one() + K::one(),
            (subtree_root_height - K::one()).to_usize().unwrap()) * subtree_root_index
    }

    fn convert_to_ordinal_number(max_height: K, height: K, index_in_row: K) -> K {
        debug_assert!(max_height >= height);
        let pow = pow((K::one() + K::one()),
                      (max_height - height).to_usize().unwrap());
        debug_assert!(index_in_row < pow);

        pow + index_in_row
    }

    fn combine_hashes(&self, hash_in_position: Hash, element_height: K, element_index: K) -> Hash {
        let ordinal_of_merkle_el =
            Self::convert_to_ordinal_number(self.tree_height, element_height, element_index);
        let (merkle_element, position) = *(self.merkle_elements_table
            .get(&ordinal_of_merkle_el)
            .expect(&format!("No merkle path element in table for height: {:?}, index: {:?} ",
                             element_height.to_usize().unwrap(),
                             element_index.to_usize().unwrap())));
        if let Some(elem_hash) = merkle_element {
            match position {
                NeighbourPosition::Left => {
                    hash(&[elem_hash.as_ref(), hash_in_position.as_ref()].concat())
                } 
                NeighbourPosition::Right => {
                    hash(&[hash_in_position.as_ref(), elem_hash.as_ref()].concat())
                } 
            }
        } else {
            hash(hash_in_position.as_ref())
        }
    }

    // fn convert_to_height_and_index(max_height: K, ordinal_number: K) -> (K, K) {
    //     debug_assert!(ordinal_number >= K::one());
    //     let depth_diff = Self::upper_power_of_two(ordinal_number) - K::one();
    //     let power_of_two = pow((K::one() + K::one()), depth_diff.to_usize().unwrap());
    //     let index_in_row = ordinal_number.mod_floor(&power_of_two);
    //     (max_height - depth_diff, index_in_row)
    // }

    // fn upper_power_of_two(v: K) -> K {
    //     let mut p = K::one();
    //     let mut i = K::zero();
    //     while p <= v {
    //         p = p * (K::one() + K::one());
    //         i = i + K::one();
    //     }
    //     i
    // }
}

impl<V: StorageValue, K> InclusionProof<V> for MerkleRangeProof<K>
    where K: Integer + Copy + Clone + ToPrimitive + StorageValue + ::std::hash::Hash
{
    fn verify(&self, values: Vec<&V>, root_hash: Hash) -> bool {
        let mut len_range = (self.range_end - self.range_start).to_usize().unwrap();
        debug_assert!(values.len() == len_range);

        let mut value_iter = values.into_iter();
        let mut previous_row: Vec<(K, Hash)> = Vec::with_capacity(len_range);
        for index in range(self.range_start, self.range_end) {
            previous_row.push((index, (value_iter.next().unwrap()).hash()));
        }

        let mut current_height = K::one();
        if current_height == self.tree_height {
            debug_assert!(previous_row.len() == 1);
            let (_, only_hash) = previous_row[0];
            return only_hash == root_hash;
        }

        let mut combined_hashes_row: Vec<(K, Hash)>;

        while current_height < self.tree_height {
            let mut last_upper_index: K = self.range_end; // we need to initialize with something, that doesn't coincide with possible values of upper_index below
            len_range = len_range / 2 + len_range % 2;
            combined_hashes_row = Vec::with_capacity(len_range);

            for (index, hash_in_table) in previous_row {
                let upper_index = index / (K::one() + K::one());
                let upper_hash_candidate =
                    self.combine_hashes(hash_in_table, current_height, index);
                if last_upper_index == upper_index {
                    let (_, prev_combined_hash) = combined_hashes_row[combined_hashes_row.len() -
                                                                      1];
                    if prev_combined_hash != upper_hash_candidate {
                        return false;
                    }
                } else {
                    combined_hashes_row.push((upper_index, upper_hash_candidate));
                }
                last_upper_index = upper_index;
            }

            previous_row = combined_hashes_row;
            current_height = current_height + K::one();
        }

        debug_assert!(previous_row.len() == 1);
        let (_, only_hash) = previous_row[0];
        only_hash == root_hash
    }
}

pub struct MerkleTable<T: Map<[u8], Vec<u8>>, K, V> {
    map: T,
    count: Cell<Option<K>>,
    _v: PhantomData<V>,
}

impl<'a, T, K, V> MerkleTable<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Integer + Copy + Clone + ToPrimitive + StorageValue + ::std::hash::Hash,
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

    fn get_neighbour_hash(&self,
                          self_height: K,
                          self_index: K)
                          -> Result<(Option<Hash>, NeighbourPosition), Error> {
        let pos: NeighbourPosition;
        let neighbour_index: K;

        if self_index.is_even() {
            neighbour_index = self_index + K::one();
            pos = NeighbourPosition::Right;
        } else {
            neighbour_index = self_index - K::one();
            pos = NeighbourPosition::Left;
        };
        let neighbour_hash = self.get_hash(self_height, neighbour_index)?;
        Ok((neighbour_hash, pos))
    }

    pub fn construct_path_for_range(&self,
                                    range_start: K,
                                    range_end: K)
                                    -> Result<MerkleRangeProof<K>, Error> {
        if (range_start < K::zero() || range_end > self.len()?) || range_end <= range_start {
            return Err(Error::new(format!("Illegal range boundaries for MerkleTable. len: \
                                           {:?}, range start: {:?}, range_end: {:?}",
                                          self.len()?.to_usize().unwrap(),
                                          range_start.to_usize().unwrap(),
                                          range_end.to_usize().unwrap())));
        }
        let max_height = self.height()?;
        let mut res = MerkleRangeProof::new(max_height, range_start, range_end);
        res.construct_proof_subtree(max_height, K::zero(), range_start, range_end, self)?;
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
        self.set_hash(K::one(), index, bytes)?;
        let mut current_height = K::one();
        while index != K::zero() {
            // Left leaf, Right leaf is empty
            let new_hash = if index.is_even() {
                let h1 = self.get_hash(current_height, index)?.unwrap();
                hash(h1.as_ref())
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
                hash(h1.as_ref())
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
          K: Integer + Copy + Clone + ToPrimitive + StorageValue + ::std::hash::Hash,
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
    use ::storage::{MemoryDB, List, MapTable, MerkleTable, InclusionProof, MerkleRangeProof};
    use std::collections::HashMap;

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
        let h3up = hash(h3.as_ref());
        let h123 = hash(&[h12.as_ref(), h3up.as_ref()].concat());

        let h34 = hash(&[h3.as_ref(), h4.as_ref()].concat());
        let h1234 = hash(&[h12.as_ref(), h34.as_ref()].concat());

        let h5up = hash(h5.as_ref());
        let h5upup = hash(h5up.as_ref());
        let h12345 = hash(&[h1234.as_ref(), h5upup.as_ref()].concat());

        let h56 = hash(&[h5.as_ref(), h6.as_ref()].concat());
        let h56up = hash(h56.as_ref());
        let h123456 = hash(&[h1234.as_ref(), h56up.as_ref()].concat());

        let h78 = hash(&[h7.as_ref(), h8.as_ref()].concat());
        let h7up = hash(h7.as_ref());
        let h567 = hash(&[h56.as_ref(), h7up.as_ref()].concat());
        let h1234567 = hash(&[h1234.as_ref(), h567.as_ref()].concat());

        let h5678 = hash(&[h56.as_ref(), h78.as_ref()].concat());
        let h12345678 = hash(&[h1234.as_ref(), h5678.as_ref()].concat());

        let mut path_result = table.construct_path_for_range(0, 1);
        assert!(path_result.is_err());


        table.append(vec![1]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h1);

        let mut range_proof = table.construct_path_for_range(0, 1).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 0);
        assert!(range_proof.verify(vec![&vec![1]], h1));
        assert!(!range_proof.verify(vec![&vec![254]], h1));


        table.append(vec![2]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h12);
        range_proof = table.construct_path_for_range(1, 2).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 1);
        assert!(range_proof.verify(vec![&vec![2]], h12));
        assert!(!range_proof.verify(vec![&vec![1]], h12));
        range_proof = table.construct_path_for_range(0, 2).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 2);
        assert!(range_proof.verify(vec![&vec![1], &vec![2]], h12));
        assert!(!range_proof.verify(vec![&vec![2], &vec![1]], h12));

        table.append(vec![3]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h123);
        range_proof = table.construct_path_for_range(2, 3).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 2);
        assert!(range_proof.verify(vec![&vec![3]], h123));
        let (hash_option, _) = *range_proof.merkle_elements_table.get(&6).unwrap();
        assert!(hash_option.is_none());
        let (hash_option, _) = *range_proof.merkle_elements_table.get(&3).unwrap();
        assert!(hash_option.unwrap() == h12);
        range_proof = table.construct_path_for_range(0, 3).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 5);
        assert!(range_proof.verify(vec![&vec![1], &vec![2], &vec![3]], h123));
        assert!(!range_proof.verify(vec![&vec![2], &vec![2], &vec![3]], h123));


        table.append(vec![4]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h1234);
        range_proof = table.construct_path_for_range(3, 4).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 2);
        assert!(range_proof.verify(vec![&vec![4]], h1234));
        let (hash_option, _) = *range_proof.merkle_elements_table.get(&7).unwrap();
        assert!(hash_option.unwrap() == h3);
        let (hash_option, _) = *range_proof.merkle_elements_table.get(&3).unwrap();
        assert!(hash_option.unwrap() == h12);
        range_proof = table.construct_path_for_range(0, 4).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 6);
        assert!(range_proof.verify(vec![&vec![1], &vec![2], &vec![3], &vec![4]], h1234));
        assert!(!range_proof.verify(vec![&vec![1], &vec![2], &vec![3], &vec![3]], h1234));



        table.append(vec![5]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h12345);
        range_proof = table.construct_path_for_range(4, 5).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 3);
        assert!(range_proof.verify(vec![&vec![5]], h12345));
        let (hash_option, _) = *range_proof.merkle_elements_table.get(&12).unwrap();
        assert!(hash_option.is_none());
        let (hash_option, _) = *range_proof.merkle_elements_table.get(&6).unwrap();
        assert!(hash_option.is_none());
        let (hash_option, _) = *range_proof.merkle_elements_table.get(&3).unwrap();
        assert!(hash_option.unwrap() == h1234);
        range_proof = table.construct_path_for_range(0, 5).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 10);
        assert!(range_proof.verify(vec![&vec![1], &vec![2], &vec![3], &vec![4], &vec![5]],
                                   h12345));
        assert!(!range_proof.verify(vec![&vec![1], &vec![2], &vec![3], &vec![4], &vec![11]],
                                    h12345));

        table.append(vec![6]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h123456);
        range_proof = table.construct_path_for_range(5, 6).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 3);
        assert!(range_proof.verify(vec![&vec![6]], h123456));
        range_proof = table.construct_path_for_range(0, 6).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 11);
        assert!(range_proof.verify(vec![&vec![1], &vec![2], &vec![3], &vec![4], &vec![5], &vec![6]],
                    h123456));
        assert!(!range_proof.verify(vec![&vec![1], &vec![2], &vec![3], &vec![4], &vec![11], &vec![6]], h123456));

        table.append(vec![7]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h1234567);
        range_proof = table.construct_path_for_range(6, 7).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 3);
        assert!(range_proof.verify(vec![&vec![7]], h1234567));
        range_proof = table.construct_path_for_range(0, 7).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 13);
        assert!(range_proof.verify(vec![&vec![1], &vec![2], &vec![3], &vec![4], &vec![5], &vec![6], &vec![7]], h1234567));
        assert!(!range_proof.verify(vec![&vec![1], &vec![2], &vec![3], &vec![4], &vec![5], &vec![6], &vec![23]], h1234567));

        table.append(vec![8]).unwrap();
        assert_eq!(table.root_hash().unwrap(), h12345678);
        range_proof = table.construct_path_for_range(7, 8).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 3);
        assert!(range_proof.verify(vec![&vec![8]], h12345678));
        range_proof = table.construct_path_for_range(0, 7).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 13);
        assert!(range_proof.verify(vec![&vec![1], &vec![2], &vec![3], &vec![4], &vec![5], &vec![6], &vec![7]], h12345678));
        range_proof = table.construct_path_for_range(0, 8).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 14);
        assert!(range_proof.verify(vec![&vec![1], &vec![2], &vec![3], &vec![4], &vec![5], &vec![6], &vec![7], &vec![8]], h12345678));
        range_proof = table.construct_path_for_range(3, 5).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 6);
        assert!(range_proof.verify(vec![&vec![4], &vec![5]], h12345678));
        range_proof = table.construct_path_for_range(2, 6).unwrap();
        assert_eq!(range_proof.merkle_elements_table.len(), 8);
        assert!(range_proof.verify(vec![&vec![3], &vec![4], &vec![5], &vec![6]], h12345678));

        path_result = table.construct_path_for_range(8, 9);
        assert!(path_result.is_err());

        path_result = table.construct_path_for_range(6, 6);
        assert!(path_result.is_err());

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

    #[test]
    fn test_indices_converting_for_merkle_range_proof() {
        let mut map_tested_values = HashMap::new();
        map_tested_values.insert(1u32, (5u32, 5u32, 0u32));
        map_tested_values.insert(3u32, (5u32, 4u32, 1u32));
        map_tested_values.insert(6u32, (4u32, 2u32, 2u32));
        map_tested_values.insert(15u32, (4u32, 1u32, 7u32));
        map_tested_values.insert(12u32, (3u32, 0u32, 4u32));
        map_tested_values.insert(8u32, (4u32, 1u32, 0u32));
        map_tested_values.insert(13u32, (4u32, 1u32, 5u32));
        map_tested_values.insert(25u32, (5u32, 1u32, 9u32));
        map_tested_values.insert(16u32, (7u32, 3u32, 0u32));
        map_tested_values.insert(31u32, (4u32, 0u32, 15u32));
        map_tested_values.insert(129u32, (8u32, 1u32, 1u32));



        for key in map_tested_values.keys() {
            let expected_ordinal = *key;
            let (max_height, height, index) = *map_tested_values.get(key).unwrap();
            assert_eq!(expected_ordinal,
                       MerkleRangeProof::convert_to_ordinal_number(max_height, height, index));
            // assert_eq!((height, index),
            // MerkleRangeProof::convert_to_height_and_index(max_height, expected_ordinal));
        }

        assert_eq!(MerkleRangeProof::index_of_first_element_in_subtree(4u32, 1u32),
                   8);
        assert_eq!(MerkleRangeProof::index_of_first_element_in_subtree(5u32, 1u32),
                   16);
        assert_eq!(MerkleRangeProof::index_of_first_element_in_subtree(3u32, 3u32),
                   12);
        assert_eq!(MerkleRangeProof::index_of_first_element_in_subtree(2u32, 3u32),
                   6);
        assert_eq!(MerkleRangeProof::index_of_first_element_in_subtree(1u32, 7u32),
                   7);
    }

    #[test]
    fn test_split_range() {
        assert_eq!((Some((0u32, 17u32)), Some((17u32, 31u32))),
                   MerkleRangeProof::split_range(0u32, 17u32, 31u32));
        assert_eq!((Some((0u32, 31u32)), None),
                   MerkleRangeProof::split_range(0u32, 31u32, 31u32));
        assert_eq!((None, Some((5u32, 31u32))),
                   MerkleRangeProof::split_range(5u32, 0u32, 31u32));
    }
}
