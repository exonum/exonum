use std::marker::PhantomData;
use std::cell::Cell;

use crypto::Hash;
use super::{Map, List, Error, StorageValue};
use self::proofnode::Proofnode;

pub mod proofnode;

const HEIGHT_SHIFT : u64 = 58;
// TODO: add checks for overflow
// const MAX_LENGTH : u64 = 288230376151711743; // 2 ** 58 - 1

type Range = Option<(u64, u64)>;

fn split_range(start: u64, middle: u64, end: u64) -> (Range, Range) {
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

fn index_of_first_element_in_subtree(subtree_root_height: u64, subtree_root_index: u64) -> u64 {
    (1 << (subtree_root_height - 1)) * subtree_root_index
}

mod hash_rules {
    use ::crypto::{hash, Hash};
    use ::storage::fields::StorageValue;
    // pub const LEAF_DOMAIN: u8 = 00;
    // pub const BRANCH_DOMAIN: u8 = 01;
    // pub const SINGLE_BRANCH_DOMAIN: u8 = 02;

    pub fn hash_leaf<V: StorageValue>(value: &V) -> Hash {
        value.hash()
    }

    pub fn hash_branch(left: Hash, right: Hash) -> Hash {
        hash(&[left.as_ref(), right.as_ref()].concat())
    }

    pub fn hash_single_branch(left: Hash) -> Hash {
        hash(left.as_ref())
    }
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
pub struct MerkleTable<T: Map<[u8], Vec<u8>>, V> {
    map: T,
    count: Cell<Option<u64>>,
    _v: PhantomData<V>,
}

impl<'a, T, V> MerkleTable<T, V> where T: Map<[u8], Vec<u8>>, V: StorageValue
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
            (0..self.len()?).map(|i| self.get(i).unwrap().unwrap()).collect()
        })
    }

    pub fn root_hash(&self) -> Result<Hash, Error> {
        self.get_hash(self.height()?, 0)
            .map(|h| h.unwrap_or_else(Hash::zero))
    }

    fn construct_proof_subtree(&self,
                               node_height: u64,
                               node_index: u64,
                               range_start: u64,
                               range_end: u64)
                               -> Result<Proofnode<V>, Error> {

        let res: Proofnode<V>;
        if node_height == 1 {
            res = Proofnode::Leaf(self.get(node_index)?.unwrap());
        } else if node_height > 1 {
            let subtree_hight = node_height - 1;
            let left_child_index = node_index * (1 + 1);
            let righ_child_index = node_index * (1 + 1) + 1;
            let right_subtree_first_index = index_of_first_element_in_subtree(subtree_hight,
                                                                              righ_child_index);
            let (left_range, right_range) =
                split_range(range_start, right_subtree_first_index, range_end);

            res = match (left_range, right_range) {
                (Some((l_s, l_e)), Some((r_s, r_e))) => {
                    let left_proof =
                        self.construct_proof_subtree(subtree_hight, left_child_index, l_s, l_e)?;
                    let right_proof =
                        self.construct_proof_subtree(subtree_hight, righ_child_index, r_s, r_e)?;
                    Proofnode::Full(Box::new(left_proof), Box::new(right_proof))
                }
                (Some((l_s, l_e)), None) => {
                    let left_proof =
                        self.construct_proof_subtree(subtree_hight, left_child_index, l_s, l_e)?;
                    let right_hash = self.get_hash(subtree_hight, righ_child_index)?;
                    Proofnode::Left(Box::new(left_proof), right_hash)
                }
                (None, Some((r_s, r_e))) => {
                    let left_hash = self.get_hash(subtree_hight, left_child_index)?.unwrap();
                    let right_proof =
                        self.construct_proof_subtree(subtree_hight, righ_child_index, r_s, r_e)?;
                    Proofnode::Right(left_hash, Box::new(right_proof))
                }
                (None, None) => {
                    unreachable!();
                }
            };
        } else {
            unreachable!();
        }
        Ok(res)
    }

    pub fn construct_path_for_range(&self,
                                    range_start: u64,
                                    range_end: u64)
                                    -> Result<Proofnode<V>, Error> {
        if (range_end > self.len()?) || range_end <= range_start {
            return Err(Error::new(format!("Illegal range boundaries for MerkleTable. len: \
                                           {:?}, range start: {:?}, range_end: {:?}",
                                          self.len()?, range_start, range_end)));
        }

        self.construct_proof_subtree(self.height()?, 0, range_start, range_end)
    }

    fn height(&self) -> Result<u64, Error> {
        let len = self.len()?;
        let h = Self::upper_power_of_two(len) + 1;
        Ok(h)
    }

    // TODO replace by library method
    fn upper_power_of_two(v: u64) -> u64 {
        let mut p = 1;
        let mut i = 0;
        while p < v {
            p *= 2;
            i += 1;
        }
        i
    }

    fn set_len(&self, len: u64) -> Result<(), Error> {
        self.count.set(Some(len));
        self.map.put(&[], len.serialize())
    }

    fn db_key(h: u64, i: u64) -> Vec<u8> {
        StorageValue::serialize((h << HEIGHT_SHIFT) + i)
    }

    fn get_hash(&self, height: u64, index: u64) -> Result<Option<Hash>, Error> {
        debug_assert!(height > 0);

        let v = self.map.get(&Self::db_key(height, index))?;
        let hash = v.map(|x| {
            debug_assert_eq!(x.len(), 32);
            Hash::from_slice(&x).unwrap()
        });
        Ok(hash)
    }

    fn set_hash(&self, height: u64, index: u64, bytes: Hash) -> Result<(), Error> {
        // FIXME avoid reallocation
        let vec = bytes.as_ref().to_vec();
        let key = Self::db_key(height, index);
        self.map.put(&key, vec)
    }

    fn append_hash(&self, mut index: u64, bytes: Hash) -> Result<(), Error> {
        self.set_hash(1, index, bytes)?;
        let mut current_height = 1;
        while index != 0 {
            // Left leaf, Right leaf is empty
            let new_hash = if index & 1 == 0 {
                let h1 = self.get_hash(current_height, index)?.unwrap();
                hash_rules::hash_single_branch(h1)
                // TODO replace by error

            } else {
                // Right leaf
                let h1 = self.get_hash(current_height, index - 1)?.unwrap();
                let h2 = self.get_hash(current_height, index)?.unwrap();
                hash_rules::hash_branch(h1, h2)
            };
            current_height += 1;
            index /= 2;
            self.set_hash(current_height, index, new_hash)?;
        }
        Ok(())
    }

    fn update_hash_subtree(&self, mut index: u64, bytes: Hash) -> Result<(), Error> {
        self.set_hash(1, index, bytes)?;

        let height = self.height()?;
        let mut current_height = 1;
        while current_height != height {
            let i = if index & 1 == 1 {
                index - 1
            } else {
                index
            };

            let h1 = self.get_hash(current_height, i)?.unwrap();
            let h2 = self.get_hash(current_height, i + 1)?;
            let new_hash = if let Some(h2) = h2 {
                hash_rules::hash_branch(h1, h2)
            } else {
                hash_rules::hash_single_branch(h1)
            };

            current_height += 1;
            index /= 2;
            self.set_hash(current_height, index, new_hash)?;
        }
        Ok(())
    }
}

impl<T, V> List<V> for MerkleTable<T, V> where T: Map<[u8], Vec<u8>>, V: StorageValue
{
    fn append(&self, value: V) -> Result<(), Error> {
        let len = self.len()?;
        self.append_hash(len, hash_rules::hash_leaf(&value))?;

        self.map.put(&Self::db_key(0, len), value.serialize())?;
        self.set_len(len + 1)?;
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

    fn get(&self, index: u64) -> Result<Option<V>, Error> {
        let value = self.map.get(&Self::db_key(0, index))?;
        Ok(value.map(StorageValue::deserialize))
    }

    fn set(&self, index: u64, value: V) -> Result<(), Error> {
        if index >= self.len()? {
            return Err(Error::new("Wrong index!"));
        }

        self.update_hash_subtree(index, hash_rules::hash_leaf(&value))?;
        self.map.put(&Self::db_key(0, index), value.serialize())
    }


    fn last(&self) -> Result<Option<V>, Error> {
        let len = self.len()?;
        if len == 0 {
            Ok(None)
        } else {
            self.get(len - 1)
        }
    }

    fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == 0)
    }

    fn len(&self) -> Result<u64, Error> {
        if let Some(count) = self.count.get() {
            return Ok(count);
        }

        let v = self.map.get(&[])?;
        let c = v.map(StorageValue::deserialize).unwrap_or(0);
        self.count.set(Some(c));
        Ok(c)
    }
}



#[cfg(test)]
mod tests {
    extern crate rand;

    use rand::{thread_rng, Rng};
    use env_logger;
    use serde::Serialize;
    use serde_json;

    use std::collections::HashSet;

    use ::crypto::{Hash, hash};
    use ::storage::{MemoryDB, List, MapTable, MerkleTable};
    use super::{split_range, index_of_first_element_in_subtree};
    use super::proofnode::{proof_indices_values, Proofnode};
    const KEY_SIZE: usize = 10;

    #[derive(Serialize)]
    struct ProofInfo<V: Serialize> {
        root_hash: Hash,
        list_length: usize,
        proof: Proofnode<V>,
        range_st: usize,
        range_end: usize,
    }

    fn generate_fully_random_data_keys(len: usize) -> Vec<(Vec<u8>)> {
        let mut rng = thread_rng();

        let mut exists_keys = HashSet::new();

        let kv_generator = |_| {
            let mut new_val: Vec<u8> = vec![0; KEY_SIZE];
            rng.fill_bytes(&mut new_val);

            while exists_keys.contains(&new_val) {
                rng.fill_bytes(&mut new_val);
            }
            exists_keys.insert(new_val.clone());
            new_val
        };

        (0..len)
            .map(kv_generator)
            .collect::<Vec<_>>()
    }

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

        assert_eq!(table.get(0).unwrap(), Some(vec![1]));
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
        assert_eq!(table.get(0).unwrap(), Some(vec![1]));
        assert_eq!(table.get(1).unwrap(), Some(vec![2]));
        assert_eq!(table.get(2).unwrap(), Some(vec![3]));
        assert_eq!(table.get(3).unwrap(), Some(vec![4]));

        table.set(1, vec![10]).unwrap();
        assert_eq!(table.get(1).unwrap(), Some(vec![10]));
    }

    #[test]
    fn generate_proof_in_table_containing_hashes() {
        let _ = env_logger::init();

        let storage = MemoryDB::new();
        let table: MerkleTable<MapTable<MemoryDB, [u8], Vec<u8>>, Hash> =
            MerkleTable::new(MapTable::new(vec![255], &storage));
        let num_vals = 10;
        let values = generate_fully_random_data_keys(num_vals as usize);
        let hash_vals: Vec<Hash> = values.into_iter().map(|el| hash(&el)).collect::<Vec<Hash>>();
        for value in &hash_vals {
            table.append(*value).unwrap();
        }
        let table_root_hash = table.root_hash().unwrap();
        let table_len = table.len().unwrap() as usize;
        let st_r = 0;
        let end_r = 5;
        let range_proof = table.construct_path_for_range(st_r, end_r).unwrap();
        assert_eq!(range_proof.compute_proof_root(), table_root_hash);
        {
            let (inds, actual_vals): (Vec<_>, Vec<&Hash>) =
                proof_indices_values(&range_proof).into_iter().unzip();
            assert_eq!(inds, (st_r as usize..end_r as usize).collect::<Vec<_>>());
            let expect_vals = &hash_vals[st_r as usize..end_r as usize];
            let paired = expect_vals.iter().zip(actual_vals);
            for pair in paired {
                assert_eq!(*pair.0, *pair.1);
            }
        }
        let proof_info = ProofInfo {
            root_hash: table_root_hash,
            list_length: table_len,
            proof: range_proof,
            range_st: st_r as usize,
            range_end: end_r as usize,
        };
        info!("{}", serde_json::to_string(&proof_info).unwrap());
    }

    #[test]
    fn randomly_generate_proofs() {
        let _ = env_logger::init();

        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));
        let num_vals = 100;
        let values = generate_fully_random_data_keys(num_vals as usize);
        let mut rng = thread_rng();
        for value in &values {
            table.append(value.clone()).unwrap();
        }
        table.get(0).unwrap();
        let table_root_hash = table.root_hash().unwrap();
        let table_len = table.len().unwrap() as usize;

        for _ in 0..50 {
            let start_range = rng.gen_range(0, num_vals);
            let end_range = rng.gen_range(start_range + 1, num_vals + 1);
            let range_proof = table.construct_path_for_range(start_range, end_range).unwrap();
            assert_eq!(range_proof.compute_proof_root(), table_root_hash);

            {
                let (inds, actual_vals): (Vec<_>, Vec<_>) =
                    proof_indices_values(&range_proof).into_iter().unzip();
                assert_eq!(inds,
                           (start_range as usize..end_range as usize).collect::<Vec<_>>());
                let expect_vals = &values[start_range as usize..end_range as usize];
                let paired = expect_vals.iter().zip(actual_vals);
                for pair in paired {
                    assert_eq!(*pair.0, *pair.1);
                }
            }

            let json_repre = serde_json::to_string(&range_proof).unwrap();
            let proof_info = ProofInfo {
                root_hash: table_root_hash,
                list_length: table_len,
                proof: range_proof,
                range_st: start_range as usize,
                range_end: end_range as usize,
            };
            info!("{}", serde_json::to_string(&proof_info).unwrap());

            // println!("{}", json_repre);
            let deser_proof: Proofnode<Vec<u8>> = serde_json::from_str(&json_repre).unwrap();
            assert_eq!(proof_indices_values(&deser_proof).len(),
                       (end_range - start_range) as usize);
            assert_eq!(deser_proof.compute_proof_root(), table_root_hash);
            // println!("{:?}", deser_proof);
        }
    }

    #[test]
    fn test_table_and_proof_roots() {
        let _ = env_logger::init();

        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));
        assert_eq!(table.root_hash().unwrap(), Hash::zero());

        let h1 = hash(&[1, 2]);
        let h2 = hash(&[2, 3]);
        let h3 = hash(&[3, 4]);
        let h4 = hash(&[4, 5]);
        let h5 = hash(&[5, 6]);
        let h6 = hash(&[6, 7]);
        let h7 = hash(&[7, 8]);
        let h8 = hash(&[8, 9]);

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

        let h7up = hash(h7.as_ref());
        let h567 = hash(&[h56.as_ref(), h7up.as_ref()].concat());
        let h1234567 = hash(&[h1234.as_ref(), h567.as_ref()].concat());

        let h78 = hash(&[h7.as_ref(), h8.as_ref()].concat());
        let h5678 = hash(&[h56.as_ref(), h78.as_ref()].concat());
        let h12345678 = hash(&[h1234.as_ref(), h5678.as_ref()].concat());

        let expected_hash_comb: Vec<(Vec<u8>, Hash, u64)> = vec![(vec![1, 2], h1, 0),
                                                                 (vec![2, 3], h12, 1),
                                                                 (vec![3, 4], h123, 2),
                                                                 (vec![4, 5], h1234, 3),
                                                                 (vec![5, 6], h12345, 4),
                                                                 (vec![6, 7], h123456, 5),
                                                                 (vec![7, 8], h1234567, 6),
                                                                 (vec![8, 9], h12345678, 7)];

        for (inserted, exp_root, proof_ind) in expected_hash_comb {
            table.append(inserted).unwrap();
            let table_len = table.len().unwrap() as usize;

            assert_eq!(table.root_hash().unwrap(), exp_root);
            let range_proof = table.construct_path_for_range(proof_ind, proof_ind + 1).unwrap();
            assert_eq!(range_proof.compute_proof_root(), exp_root);
            assert_eq!(proof_indices_values(&range_proof).len(), 1);

            let json_repre = serde_json::to_string(&range_proof).unwrap();
            // println!("{}", json_repre);
            let deser_proof: Proofnode<Vec<u8>> = serde_json::from_str(&json_repre).unwrap();
            assert_eq!(proof_indices_values(&deser_proof).len(), 1);
            assert_eq!(deser_proof.compute_proof_root(), exp_root);
            // println!("{:?}", deser_proof);

            let proof_info = ProofInfo {
                root_hash: exp_root,
                list_length: table_len,
                proof: range_proof,
                range_st: proof_ind as usize,
                range_end: (proof_ind + 1) as usize,
            };
            info!("{}", serde_json::to_string(&proof_info).unwrap());

            let range_proof = table.construct_path_for_range(0, proof_ind + 1).unwrap();
            assert_eq!(range_proof.compute_proof_root(), exp_root);
            assert_eq!(proof_indices_values(&range_proof).len(),
                       (proof_ind + 1) as usize);

            let json_repre = serde_json::to_string(&range_proof).unwrap();
            // println!("{}", json_repre);
            let deser_proof: Proofnode<Vec<u8>> = serde_json::from_str(&json_repre).unwrap();
            assert_eq!(proof_indices_values(&deser_proof).len(),
                       (proof_ind + 1) as usize);
            assert_eq!(deser_proof.compute_proof_root(), exp_root);
            let proof_info = ProofInfo {
                root_hash: exp_root,
                list_length: table_len,
                proof: range_proof,
                range_st: 0,
                range_end: (proof_ind + 1) as usize,
            };
            info!("{}", serde_json::to_string(&proof_info).unwrap());
            // println!("{:?}", deser_proof);
            let range_proof = table.construct_path_for_range(0, 1).unwrap();
            assert_eq!(range_proof.compute_proof_root(), exp_root);
            assert_eq!(proof_indices_values(&range_proof).len(), 1);

            let json_repre = serde_json::to_string(&range_proof).unwrap();
            // println!("{}", json_repre);
            let deser_proof: Proofnode<Vec<u8>> = serde_json::from_str(&json_repre).unwrap();
            assert_eq!(proof_indices_values(&deser_proof).len(), 1);
            assert_eq!(deser_proof.compute_proof_root(), exp_root);
            // println!("{:?}", deser_proof);

            let proof_info = ProofInfo {
                root_hash: exp_root,
                list_length: table_len,
                proof: range_proof,
                range_st: 0,
                range_end: 1,
            };
            info!("{}", serde_json::to_string(&proof_info).unwrap());
        }

        let range_proof = table.construct_path_for_range(0, 8).unwrap();
        let (inds, val_refs): (Vec<_>, Vec<_>) =
            proof_indices_values(&range_proof).into_iter().unzip();
        assert_eq!(inds, (0usize..8).collect::<Vec<_>>());
        let expect_vals = vec![vec![1, 2], vec![2, 3], vec![3, 4], vec![4, 5], vec![5, 6],
                               vec![6, 7], vec![7, 8], vec![8, 9]];
        let paired = expect_vals.into_iter().zip(val_refs);
        for pair in paired {
            assert_eq!(pair.0, *pair.1);
        }

        let mut range_proof = table.construct_path_for_range(3, 5).unwrap();
        assert_eq!(proof_indices_values(&range_proof).len(), 2);
        assert_eq!(range_proof.compute_proof_root(), h12345678);
        range_proof = table.construct_path_for_range(2, 6).unwrap();
        assert_eq!(proof_indices_values(&range_proof).len(), 4);
        assert_eq!(range_proof.compute_proof_root(), h12345678);
        assert_eq!(table.get(0).unwrap(), Some(vec![1, 2]));
    }

    #[test]
    #[should_panic]
    fn test_proof_illegal_lower_bound() {
        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));
        table.construct_path_for_range(0, 1).unwrap();
        table.append(vec![1]).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_proof_illegal_bound_empty() {
        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));
        for i in 0u8..8 {
            table.append(vec![i]).unwrap();
        }
        table.construct_path_for_range(8, 9).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_proof_illegal_range() {
        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));
        for i in 0u8..4 {
            table.append(vec![i]).unwrap();
        }
        table.construct_path_for_range(2, 2).unwrap();
    }

    #[test]
    fn test_proof_structure() {
        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));
        assert_eq!(table.root_hash().unwrap(), Hash::zero());

        let h1 = hash(&vec![0, 1, 2]);
        let h2 = hash(&vec![1, 2, 3]);
        let h3 = hash(&vec![2, 3, 4]);
        let h4 = hash(&vec![3, 4, 5]);
        let h5 = hash(&vec![4, 5, 6]);
        let h12 = hash(&[h1.as_ref(), h2.as_ref()].concat());
        let h34 = hash(&[h3.as_ref(), h4.as_ref()].concat());
        let h1234 = hash(&[h12.as_ref(), h34.as_ref()].concat());
        let h5up = hash(h5.as_ref());
        let h5upup = hash(h5up.as_ref());
        let h12345 = hash(&[h1234.as_ref(), h5upup.as_ref()].concat());

        for i in 0u8..5 {
            table.append(vec![i, i + 1, i + 2]).unwrap();
        }

        assert_eq!(table.root_hash().unwrap(), h12345);
        let range_proof = table.construct_path_for_range(4, 5).unwrap();
        assert_eq!(range_proof.compute_proof_root(), h12345);

        assert_eq!(vec![4, 5, 6], *(proof_indices_values(&range_proof)[0].1));
        if let Proofnode::Right(left_hash1, right_proof1) = range_proof {
            assert_eq!(left_hash1, h1234);
            let unboxed_proof = *right_proof1;
            if let Proofnode::Left(left_proof2, right_hash2) = unboxed_proof {
                assert!(right_hash2.is_none());
                let unboxed_proof = *left_proof2;
                if let Proofnode::Left(_, right_hash3) = unboxed_proof {
                    assert!(right_hash3.is_none());
                } else {
                    assert!(false);
                }
            } else {
                assert!(false);
            }

        } else {
            assert!(false);
        }
        table.append(vec![5, 6, 7]).unwrap();
        // let range_proof = table.construct_path_for_range(3, 5).unwrap();
        // println!("{:?}", range_proof);
        // let json_repre = serde_json::to_string(&range_proof).unwrap();
        // println!("{}", json_repre );
    }

    #[test]
    fn test_hash_in_values() {
        let storage = MemoryDB::new();
        let table = MerkleTable::new(MapTable::new(vec![255], &storage));

        let h = hash(&[1, 2, 3, 4]);
        table.append(h).unwrap();
        assert_eq!(table.get(0).unwrap(), Some(h));
    }

    #[test]
    fn test_hash_set_value_simple() {
        let h1 = hash(&[1]);
        let h2 = hash(&[2]);

        let s = MemoryDB::new();
        let t = MerkleTable::new(MapTable::new(vec![255], &s));
        assert_eq!(t.get(0).unwrap(), None);
        t.append(vec![1]).unwrap();
        assert_eq!(t.root_hash().unwrap(), h1);

        t.set(0, vec![2]).unwrap();
        assert_eq!(t.root_hash().unwrap(), h2);
    }

    #[test]
    fn test_swap_values() {

        let s1 = MemoryDB::new();
        let t1: MerkleTable<MapTable<MemoryDB, [u8], Vec<u8>>, Vec<u8>> =
            MerkleTable::new(MapTable::new(vec![255], &s1));
        let values1 = vec![
            vec![1],
            vec![2],
            vec![33],
            vec![44],
        ];
        t1.extend(values1).unwrap();
        t1.swap(2, 3).unwrap();

        let s2 = MemoryDB::new();
        let t2: MerkleTable<MapTable<MemoryDB, [u8], Vec<u8>>, Vec<u8>> =
            MerkleTable::new(MapTable::new(vec![255], &s2));
        let values2 = vec![
            vec![1],
            vec![2],
            vec![44],
            vec![33],
        ];
        t2.extend(values2).unwrap();
        assert_eq!(t1.root_hash().unwrap(), t2.root_hash().unwrap());
    }

    #[test]
    fn test_hash_set_value() {
        let s1 = MemoryDB::new();
        let t1 = MerkleTable::new(MapTable::new(vec![255], &s1));
        assert_eq!(t1.get(0).unwrap(), None);
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
        assert_eq!(t2.get(0).unwrap(), None);
        t2.append(vec![4]).unwrap();
        t2.append(vec![7]).unwrap();
        t2.append(vec![5]).unwrap();
        t2.append(vec![1]).unwrap();

        assert_eq!(t1.root_hash().unwrap(), t2.root_hash().unwrap());
    }

    #[test]
    fn test_indices_converting_for_merkle_range_proof() {
        assert_eq!(index_of_first_element_in_subtree(4, 1), 8);
        assert_eq!(index_of_first_element_in_subtree(5, 1), 16);
        assert_eq!(index_of_first_element_in_subtree(3, 3), 12);
        assert_eq!(index_of_first_element_in_subtree(2, 3), 6);
        assert_eq!(index_of_first_element_in_subtree(1, 7), 7);
    }

    #[test]
    fn test_split_range() {
        assert_eq!((Some((0, 17)), Some((17, 31))),
                   split_range(0, 17, 31));
        assert_eq!((Some((0, 31)), None), split_range(0, 31, 31));
        assert_eq!((None, Some((5, 31))), split_range(5, 0, 31));
    }
}
