// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate rand;

use std::collections::{HashSet, BTreeMap};
use rand::{thread_rng, Rng, sample};
use crypto::{hash, Hash, HashStream};
use storage::db::Database;
use encoding::serialize::json::reexport::to_string;
use encoding::serialize::reexport::{Serialize, Serializer};

use super::{DBKey, ProofMapIndex, ProofMapKey};
use super::proof::MapProof;
use super::key::{ChildKind, KEY_SIZE, LEAF_KEY_PREFIX};
use super::node::BranchNode;

const IDX_NAME: &'static str = "idx_name";

// Makes large data set with unique keys
fn generate_random_data(len: usize) -> Vec<([u8; KEY_SIZE], Vec<u8>)> {
    let mut rng = thread_rng();
    let mut exists_keys = HashSet::new();
    let mut base = [0; KEY_SIZE];
    rng.fill_bytes(&mut base);
    let base = base;

    let kv_generator = |_| {
        let mut v = vec![0; 8];

        // Generate only unique keys
        let mut k = base.clone();
        let byte: usize = rng.gen_range(0, 31);
        k[byte] = rng.gen::<u8>();

        rng.fill_bytes(&mut v);
        while exists_keys.contains(&k) {
            rng.fill_bytes(&mut k);
        }
        exists_keys.insert(k.clone());
        (k, v)
    };

    (0..len).map(kv_generator).collect::<Vec<_>>()
}

// Makes large data set with unique keys
fn generate_fully_random_data_keys(len: usize) -> Vec<([u8; KEY_SIZE], Vec<u8>)> {
    let mut rng = thread_rng();

    let mut exists_keys = HashSet::new();

    let kv_generator = |_| {
        let mut v = vec![0; 8];
        let mut new_key = [0; KEY_SIZE];
        rng.fill_bytes(&mut new_key);

        while exists_keys.contains(&new_key) {
            rng.fill_bytes(&mut new_key);
        }
        exists_keys.insert(new_key.clone());
        rng.fill_bytes(&mut v);
        (new_key, v)
    };

    (0..len).map(kv_generator).collect::<Vec<_>>()
}

fn gen_tempdir_name() -> String {
    thread_rng().gen_ascii_chars().take(10).collect()
}

fn insert_trivial(db1: Box<Database>, db2: Box<Database>) {
    let mut storage1 = db1.fork();
    let mut storage2 = db2.fork();

    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);
    index1.put(&[255; 32], vec![1]);
    index1.put(&[254; 32], vec![2]);

    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    index2.put(&[254; 32], vec![2]);
    index2.put(&[255; 32], vec![1]);

    assert_eq!(index1.get(&[255; 32]), Some(vec![1]));
    assert_eq!(index1.get(&[254; 32]), Some(vec![2]));
    assert_eq!(index2.get(&[255; 32]), Some(vec![1]));
    assert_eq!(index2.get(&[254; 32]), Some(vec![2]));

    assert_ne!(index1.root_hash(), Hash::zero());
    assert_eq!(index1.root_hash(), index2.root_hash());
}

fn insert_same_key(db: Box<Database>) {
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);
    assert_eq!(table.root_hash(), Hash::zero());
    let root_prefix = &[&[LEAF_KEY_PREFIX], vec![255; 32].as_slice(), &[0u8]].concat();
    let hash = HashStream::new()
        .update(root_prefix)
        .update(hash(&[2]).as_ref())
        .hash();

    table.put(&[255; 32], vec![1]);
    table.put(&[255; 32], vec![2]);
    assert_eq!(table.get(&[255; 32]), Some(vec![2]));
    assert_eq!(table.root_hash(), hash);
}

fn insert_simple(db1: Box<Database>, db2: Box<Database>) {
    let mut storage1 = db1.fork();
    let mut storage2 = db2.fork();

    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);
    index1.put(&[255; 32], vec![3]);
    index1.put(&[254; 32], vec![2]);
    index1.put(&[250; 32], vec![1]);
    index1.put(&[254; 32], vec![5]);

    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    index2.put(&[250; 32], vec![1]);
    index2.put(&[254; 32], vec![2]);
    index2.put(&[255; 32], vec![3]);
    index2.put(&[254; 32], vec![5]);

    assert!(index1.root_hash() != Hash::zero());
    assert_eq!(index1.root_hash(), index2.root_hash());
}

fn insert_reverse(db1: Box<Database>, db2: Box<Database>) {
    let mut storage1 = db1.fork();
    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);
    index1.put(&[42; 32], vec![1]);
    index1.put(&[64; 32], vec![2]);
    index1.put(&[240; 32], vec![3]);
    index1.put(&[245; 32], vec![4]);
    index1.put(&[250; 32], vec![5]);
    index1.put(&[255; 32], vec![6]);

    let mut storage2 = db2.fork();
    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    index2.put(&[255; 32], vec![6]);
    index2.put(&[250; 32], vec![5]);
    index2.put(&[245; 32], vec![4]);
    index2.put(&[240; 32], vec![3]);
    index2.put(&[64; 32], vec![2]);
    index2.put(&[42; 32], vec![1]);


    assert!(index2.root_hash() != Hash::zero());
    assert_eq!(index2.root_hash(), index1.root_hash());
}

fn remove_trivial(db1: Box<Database>, db2: Box<Database>) {
    let mut storage1 = db1.fork();
    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);
    index1.put(&[255; 32], vec![6]);
    index1.remove(&[255; 32]);

    let mut storage2 = db2.fork();
    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    index2.put(&[255; 32], vec![6]);
    index2.remove(&[255; 32]);

    assert_eq!(index1.root_hash(), Hash::zero());
    assert_eq!(index2.root_hash(), Hash::zero());
}

fn remove_simple(db1: Box<Database>, db2: Box<Database>) {
    let mut storage1 = db1.fork();
    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);
    index1.put(&[255; 32], vec![1]);
    index1.put(&[250; 32], vec![2]);
    index1.put(&[245; 32], vec![3]);

    index1.remove(&[255; 32]);
    index1.remove(&[245; 32]);

    let mut storage2 = db2.fork();
    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    index2.put(&[250; 32], vec![2]);
    index2.put(&[255; 32], vec![1]);
    index2.put(&[245; 32], vec![3]);

    index2.remove(&[255; 32]);
    index2.remove(&[245; 32]);

    assert_eq!(index2.get(&[250; 32]), Some(vec![2]));
    assert_eq!(index1.get(&[250; 32]), Some(vec![2]));
    assert!(index1.get(&[255; 32]).is_none());
    assert!(index2.get(&[255; 32]).is_none());
    assert!(index1.get(&[245; 32]).is_none());
    assert!(index2.get(&[245; 32]).is_none());

    assert_eq!(index1.root_hash(), index2.root_hash());
}

fn remove_reverse(db1: Box<Database>, db2: Box<Database>) {
    let mut storage1 = db1.fork();
    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);
    index1.put(&[42; 32], vec![1]);
    index1.put(&[64; 32], vec![2]);
    index1.put(&[240; 32], vec![3]);
    index1.put(&[245; 32], vec![4]);
    index1.put(&[250; 32], vec![5]);
    index1.put(&[255; 32], vec![6]);

    index1.remove(&[255; 32]);
    index1.remove(&[250; 32]);
    index1.remove(&[245; 32]);
    index1.remove(&[240; 32]);
    index1.remove(&[64; 32]);
    index1.remove(&[42; 32]);

    let mut storage2 = db2.fork();
    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    index2.put(&[255; 32], vec![6]);
    index2.put(&[250; 32], vec![5]);
    index2.put(&[245; 32], vec![4]);
    index2.put(&[240; 32], vec![3]);
    index2.put(&[64; 32], vec![2]);
    index2.put(&[42; 32], vec![1]);

    index2.remove(&[42; 32]);
    index2.remove(&[64; 32]);
    index2.remove(&[240; 32]);
    index2.remove(&[245; 32]);
    index2.remove(&[250; 32]);
    index2.remove(&[255; 32]);

    assert_eq!(index2.root_hash(), index1.root_hash());
}

fn fuzz_insert(db1: Box<Database>, db2: Box<Database>) {
    let mut data = generate_random_data(100);
    let mut rng = rand::thread_rng();
    let mut storage1 = db1.fork();
    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);

    for item in &data {
        index1.put(&item.0, item.1.clone());
    }

    let mut storage2 = db2.fork();
    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    rng.shuffle(&mut data);
    for item in &data {
        index2.put(&item.0, item.1.clone());
    }

    for item in &data {
        let v1 = index1.get(&item.0);
        let v2 = index2.get(&item.0);
        assert_eq!(v1.as_ref(), Some(&item.1));
        assert_eq!(v2.as_ref(), Some(&item.1));
    }

    assert!(index2.root_hash() != Hash::zero());
    assert_eq!(index2.root_hash(), index1.root_hash());

    // Test same keys
    rng.shuffle(&mut data);
    for item in &data {
        index1.put(&item.0, vec![1]);
    }
    rng.shuffle(&mut data);
    for item in &data {
        index2.put(&item.0, vec![1]);
    }

    for item in &data {
        let v1 = index1.get(&item.0);
        let v2 = index2.get(&item.0);
        assert_eq!(v1.as_ref(), Some(&vec![1]));
        assert_eq!(v2.as_ref(), Some(&vec![1]));
    }
    assert_eq!(index2.root_hash(), index1.root_hash());
}

fn check_map_proof<K, V>(
    proof: MapProof<K, V>,
    key: Option<K>,
    table: &ProofMapIndex<&mut Fork, K, V>,
) where
    K: ProofMapKey + PartialEq + ::std::fmt::Debug,
    V: StorageValue + PartialEq + ::std::fmt::Debug,
{
    let entries = match key {
        Some(key) => {
            let value = table.get(&key).unwrap();
            vec![(key, value)]
        }
        None => vec![],
    };
    assert_eq!(proof.try_into().unwrap(), (entries, table.root_hash()));
}

fn check_map_proofs<K, V>(
    proof: MapProof<K, V>,
    keys: Vec<K>,
    table: &ProofMapIndex<&mut Fork, K, V>,
) where
    K: ProofMapKey + PartialEq + Ord + ::std::fmt::Debug,
    V: StorageValue + PartialEq + ::std::fmt::Debug,
{
    let entries = {
        let mut entries = BTreeMap::new();

        for key in keys {
            if table.contains(&key) {
                let value = table.get(&key).unwrap();
                entries.insert(key, value);
            }
        }

        entries
    };

    assert_eq!(proof.try_into().unwrap(), (entries, table.root_hash()));
}

const MAX_CHECKED_ELEMENTS: usize = 1_024;

fn check_proofs_for_data<K, V>(db: &Box<Database>, data: Vec<(K, V)>, nonexisting_keys: Vec<K>)
where
    K: ProofMapKey + Copy + PartialEq + ::std::fmt::Debug + Serialize,
    V: StorageValue + Clone + PartialEq + ::std::fmt::Debug + Serialize,
{
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);
    for &(ref key, ref value) in &data {
        table.put(key, value.clone());
    }

    let batch_size = data.len();
    let indexes = if batch_size < MAX_CHECKED_ELEMENTS {
        (0..batch_size).collect()
    } else {
        let mut rng = rand::thread_rng();
        sample(&mut rng, 0..batch_size, MAX_CHECKED_ELEMENTS)
    };

    for i in indexes {
        let key = data[i].0;
        let proof = table.get_proof(key);
        check_map_proof(proof, Some(key), &table);
    }

    for key in nonexisting_keys {
        if !table.contains(&key) {
            // The check is largely redundant, but better be here anyway
            let proof = table.get_proof(key);
            check_map_proof(proof, None, &table);
        }
    }
}

fn check_multiproofs_for_data<K, V>(db: &Box<Database>, data: Vec<(K, V)>, nonexisting_keys: Vec<K>)
where
    K: ProofMapKey + Copy + Ord + PartialEq + ::std::fmt::Debug + Serialize,
    V: StorageValue + Clone + PartialEq + ::std::fmt::Debug + Serialize,
{
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(vec![255], &mut storage);
    for &(ref key, ref value) in &data {
        table.put(key, value.clone());
    }

    let mut rng = rand::thread_rng();

    // Test for batches of 1, 11, ..., 101 keys
    for proof_size in (0..11).map(|x| x * 10 + 1) {
        // Check the multiproof only for existing keys
        let keys = sample(&mut rng, data.iter().map(|&(k, _)| k), proof_size);
        let proof = table.get_multiproof(keys.clone());
        check_map_proofs(proof, keys, &table);

        // Check the multiproof for the equal number of existing and non-existing keys
        let mut keys = sample(&mut rng, data.iter().map(|&(k, _)| k), proof_size);
        let non_keys = sample(&mut rng, &nonexisting_keys, proof_size);
        keys.extend(non_keys);
        let proof = table.get_multiproof(keys.clone());
        check_map_proofs(proof, keys, &table);
    }
}

fn build_proof_in_empty_tree(db: Box<Database>) {
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);

    // Just to notify the compiler of the types used; same key is added and then removed from tree.
    table.put(&[230; 32], vec![1]);
    table.remove(&[230; 32]);

    let proof = table.get_proof([244; 32]);
    assert_eq!(proof.proof(), vec![]);
    check_map_proof(proof, None, &table);
}

fn build_multiproof_in_empty_tree(db: Box<Database>) {
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(vec![255], &mut storage);

    // Just to notify the compiler of the types used; same key is added and then removed from tree.
    table.put(&[230; 32], vec![1]);
    table.remove(&[230; 32]);

    let keys = vec![[0; 32], [230; 32], [244; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(proof.proof(), vec![]);
    check_map_proofs(proof, keys, &table);
}

fn build_proof_in_single_node_tree(db: Box<Database>) {
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(vec![255], &mut storage);

    table.put(&[230; 32], vec![1]);
    let proof = table.get_proof([230; 32]);
    assert_eq!(proof.proof(), vec![]);
    check_map_proof(proof, Some([230; 32]), &table);

    let proof = table.get_proof([128; 32]);
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&[230; 32]), hash(&vec![1]))]
    );
    check_map_proof(proof, None, &table);
}

fn build_multiproof_in_single_node_tree(db: Box<Database>) {
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(vec![255], &mut storage);

    table.put(&[230; 32], vec![1]);

    let keys = vec![[230; 32], [1; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(proof.proof(), vec![]);
    check_map_proofs(proof, keys, &table);

    let keys = vec![[229; 32], [1; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&[230; 32]), hash(&vec![1]))]
    );
    check_map_proofs(proof, keys, &table);
}

fn build_proof_in_multinode_tree(db: Box<Database>) {
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);

    table.put(&[1; 32], vec![1]);
    table.put(&[4; 32], vec![2]);
    let proof = table.get_proof([1; 32]);
    assert_eq!(proof.proof(), vec![(DBKey::leaf(&[4; 32]), hash(&vec![2]))]);
    check_map_proof(proof, Some([1; 32]), &table);

    let proof = table.get_proof([4; 32]);
    assert_eq!(proof.proof(), vec![(DBKey::leaf(&[1; 32]), hash(&vec![1]))]);
    check_map_proof(proof, Some([4; 32]), &table);

    // Key left of all keys in the tree
    let proof = table.get_proof([0; 32]);
    let exp_proof =
        vec![(DBKey::leaf(&[1; 32]), hash(&vec![1])), (DBKey::leaf(&[4; 32]), hash(&vec![2]))];
    assert_eq!(proof.proof(), exp_proof);
    check_map_proof(proof, None, &table);

    // Key between the keys in the tree
    let proof = table.get_proof([2; 32]);
    assert_eq!(proof.proof(), exp_proof);
    check_map_proof(proof, None, &table);

    // Key to the right of all keys
    let proof = table.get_proof([255; 32]);
    assert_eq!(proof.proof(), exp_proof);
    check_map_proof(proof, None, &table);

    // Insert key that splits 15-bit segment off the left key in the tree.
    // The key itself is to the left of the `[1; 32]` key.
    let left_key = {
        let mut key = [0; 32];
        key[0] = 1;
        table.put(&key, vec![3]);
        key
    };

    let left_hash = {
        let mut node = BranchNode::empty();
        node.set_child(ChildKind::Left, &DBKey::leaf(&left_key), &hash(&vec![3]));
        node.set_child(ChildKind::Right, &DBKey::leaf(&[1; 32]), &hash(&vec![1]));
        node.hash()
    };

    let proof = table.get_proof([1; 32]);
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&left_key), hash(&vec![3])), (DBKey::leaf(&[4; 32]), hash(&vec![2]))]
    );
    check_map_proof(proof, Some([1; 32]), &table);

    let proof = table.get_proof([2; 32]);
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&left_key).truncate(15), left_hash), (DBKey::leaf(&[4; 32]), hash(&vec![2]))]
    );
    check_map_proof(proof, None, &table);

    let proof = table.get_proof([4; 32]);
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&left_key).truncate(15), left_hash)]
    );
    check_map_proof(proof, Some([4; 32]), &table);

    let proof = table.get_proof([128; 32]);
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&left_key).truncate(15), left_hash), (DBKey::leaf(&[4; 32]), hash(&vec![2]))]
    );
    check_map_proof(proof, None, &table);

    // Insert key that splits 12-bit segment off the [4; 32] key in the tree.
    // The key is to the right of the [4; 32] key.
    let right_key = {
        let mut key = [0; 32];
        key[0] = 4;
        key[1] = 9;
        table.put(&key, vec![4]);
        key
    };

    let right_hash = {
        let mut node = BranchNode::empty();
        node.set_child(ChildKind::Left, &DBKey::leaf(&[4; 32]), &hash(&vec![2]));
        node.set_child(ChildKind::Right, &DBKey::leaf(&right_key), &hash(&vec![4]));
        node.hash()
    };

    let proof = table.get_proof([1; 32]);
    assert_eq!(
        proof.proof(),
        vec![
            (DBKey::leaf(&left_key), hash(&vec![3])),
            (DBKey::leaf(&right_key).truncate(12), right_hash),
        ]
    );
    check_map_proof(proof, Some([1; 32]), &table);

    // Non-existing key between two children at the root node
    let proof = table.get_proof([2; 32]);
    assert_eq!(
        proof.proof(),
        vec![
            (DBKey::leaf(&left_key).truncate(15), left_hash),
            (DBKey::leaf(&right_key).truncate(12), right_hash),
        ]
    );
    check_map_proof(proof, None, &table);

    // Non-existing key between the first added node `[1; 32]` and the `left_key`.
    let nonexisting_key = {
        let mut key = [0; 32];
        key[0] = 1;
        key[1] = 1;
        key[29] = 29;
        key
    };

    let proof = table.get_proof(nonexisting_key);
    assert_eq!(
        proof.proof(),
        vec![
            (DBKey::leaf(&left_key), hash(&vec![3])),
            (DBKey::leaf(&[1; 32]), hash(&vec![1])),
            (DBKey::leaf(&right_key).truncate(12), right_hash),
        ]
    );
    check_map_proof(proof, None, &table);

    let subtree_hash = table.root_hash();
    table.put(&[129; 32], vec![5]);
    // The tree is now as follows:
    // - Bits(0000_0): -> (subtree_hash)
    //   - Bits(...001_0000_000): -> (left_hash)
    //     - left_key -> [3]
    //     - [1; 32] -> [1]
    //   - Bits(...100_0000): -> (right_hash)
    //     - [4; 32] -> [2]
    //     - right_key -> [4]
    // - [129; 32] -> [5]

    let proof = table.get_proof([129; 32]);
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&[0; 32]).truncate(5), subtree_hash)]
    );
    check_map_proof(proof, Some([129; 32]), &table);

    let proof = table.get_proof([128; 32]);
    assert_eq!(
        proof.proof(),
        vec![
            (DBKey::leaf(&[0; 32]).truncate(5), subtree_hash),
            (DBKey::leaf(&[129; 32]), hash(&vec![5])),
        ]
    );
    check_map_proof(proof, None, &table);

    let proof = table.get_proof([4; 32]);
    assert_eq!(
        proof.proof(),
        vec![
            (DBKey::leaf(&left_key).truncate(15), left_hash),
            (DBKey::leaf(&right_key), hash(&vec![4])),
            (DBKey::leaf(&[129; 32]), hash(&vec![5])),
        ]
    );
    check_map_proof(proof, Some([4; 32]), &table);
}

fn build_multiproof_simple(db: Box<Database>) {
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(vec![255], &mut storage);

    table.put(&[1; 32], vec![1]);
    table.put(&[4; 32], vec![2]);

    let keys = vec![[0; 32], [1; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(proof.proof(), vec![(DBKey::leaf(&[4; 32]), hash(&vec![2]))]);
    check_map_proofs(proof, keys, &table);

    let keys = vec![[2; 32], [0; 32], [1; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(proof.proof(), vec![(DBKey::leaf(&[4; 32]), hash(&vec![2]))]);
    check_map_proofs(proof, keys, &table);

    let keys = vec![[2; 32], [0; 32], [1; 32], [129; 32], [4; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(proof.proof(), vec![]);
    check_map_proofs(proof, keys, &table);

    let keys = vec![[2; 32], [0; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&[1; 32]), hash(&vec![1])), (DBKey::leaf(&[4; 32]), hash(&vec![2]))]
    );
    check_map_proofs(proof, keys, &table);

    let keys = vec![[2; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&[1; 32]), hash(&vec![1])), (DBKey::leaf(&[4; 32]), hash(&vec![2]))]
    );
    check_map_proofs(proof, keys, &table);

    let keys = vec![[255; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&[1; 32]), hash(&vec![1])), (DBKey::leaf(&[4; 32]), hash(&vec![2]))]
    );
    check_map_proofs(proof, keys, &table);

    // Insert key that splits 15-bit segment off the left key in the tree.
    // The key itself is to the left of the `[1; 32]` key.
    let left_key = {
        let mut key = [0; 32];
        key[0] = 1;
        table.put(&key, vec![3]);
        key
    };

    let left_hash = {
        let mut node = BranchNode::empty();
        node.set_child(ChildKind::Left, &DBKey::leaf(&left_key), &hash(&vec![3]));
        node.set_child(ChildKind::Right, &DBKey::leaf(&[1; 32]), &hash(&vec![1]));
        node.hash()
    };

    let proof = table.get_multiproof(vec![[0; 32]]);
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&[1; 32]).truncate(15), left_hash), (DBKey::leaf(&[4; 32]), hash(&vec![2]))]
    );
    check_map_proofs(proof, vec![[0; 32]], &table);

    let keys = {
        // `key > [1; 32]`, which is visible from the `left_key` / `[1; 32]` junction
        let mut key = [1; 32];
        key[1] = 16;
        vec![[0; 32], key, [2; 32]]
    };
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&[1; 32]).truncate(15), left_hash), (DBKey::leaf(&[4; 32]), hash(&vec![2]))]
    );
    check_map_proofs(proof, keys, &table);

    let keys = vec![[0; 32], [1; 32], [2; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&left_key), hash(&vec![3])), (DBKey::leaf(&[4; 32]), hash(&vec![2]))]
    );
    check_map_proofs(proof, keys, &table);

    let keys = vec![[0; 32], [1; 32], [4; 32], [2; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&left_key), hash(&vec![3]))]
    );
    check_map_proofs(proof, keys, &table);

    let keys = vec![[0; 32], [4; 32], [255; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&[1; 32]).truncate(15), left_hash)]
    );
    check_map_proofs(proof, keys, &table);

    let keys = {
        // `key` is between `left_key` and `[1; 32]`, so they both should be returned in the proof
        // to prove non-existence of `key`
        let mut key = [0; 32];
        key[0] = 1;
        key[1] = 1;
        vec![[0; 32], [4; 32], key]
    };
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof(),
        vec![(DBKey::leaf(&left_key), hash(&vec![3])), (DBKey::leaf(&[1; 32]), hash(&vec![1]))]
    );
    check_map_proofs(proof, keys, &table);
}

fn fuzz_insert_build_proofs_in_table_filled_with_hashes(db: Box<Database>) {
    for batch_size in (0..16).map(|x| 1 << x) {
        let data: Vec<(Hash, Hash)> = generate_fully_random_data_keys(batch_size)
            .into_iter()
            .map(|(key, val)| (hash(&key), hash(&val)))
            .collect();

        let nonexisting_count = ::std::cmp::min(MAX_CHECKED_ELEMENTS, batch_size);
        let nonexisting_keys: Vec<_> = generate_fully_random_data_keys(nonexisting_count / 2)
            .into_iter()
            .flat_map(|(key, val)| vec![hash(&key), hash(&val)])
            .collect();

        check_proofs_for_data(&db, data, nonexisting_keys);
    }
}

fn fuzz_insert_build_proofs(db: Box<Database>) {
    for batch_size in (1..11).map(|x| (1 << x) - 1) {
        let data = generate_fully_random_data_keys(batch_size);

        let nonexisting_count = ::std::cmp::min(MAX_CHECKED_ELEMENTS, batch_size);
        let nonexisting_keys: Vec<_> = generate_fully_random_data_keys(nonexisting_count)
            .into_iter()
            .map(|(key, _)| key)
            .collect();

        check_proofs_for_data(&db, data, nonexisting_keys);
    }
}

fn fuzz_insert_build_multiproofs(db: Box<Database>) {
    for batch_size in (9..16).map(|x| 1 << x) {
        let data = generate_fully_random_data_keys(batch_size);

        let nonexisting_count = ::std::cmp::min(MAX_CHECKED_ELEMENTS, batch_size);
        let nonexisting_keys: Vec<_> = generate_fully_random_data_keys(nonexisting_count)
            .into_iter()
            .map(|(key, _)| key)
            .collect();

        check_multiproofs_for_data(&db, data, nonexisting_keys);
    }
}

fn fuzz_delete_build_proofs(db: Box<Database>) {
    let data = generate_fully_random_data_keys(9_000);
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);
    for item in &data {
        table.put(&item.0, item.1.clone());
    }

    let (keys_to_remove, keys_to_remove_seq) = {
        let mut rng = rand::thread_rng();
        let mut keys = sample(&mut rng, data.iter().map(|item| item.0.clone()), 2_000);
        rng.shuffle(&mut keys);
        let seq_keys = keys.split_off(1_000);
        (keys, seq_keys)
    };

    for key in &keys_to_remove {
        table.remove(key);
    }
    for key in keys_to_remove {
        let proof = table.get_proof(key);
        check_map_proof(proof, None, &table);
    }

    for key in keys_to_remove_seq {
        let proof = table.get_proof(key);
        check_map_proof(proof, Some(key.clone()), &table);
        table.remove(&key);
        let proof = table.get_proof(key);
        check_map_proof(proof, None, &table);
    }
}

fn fuzz_delete(db1: Box<Database>, db2: Box<Database>) {
    let mut data = generate_random_data(100);
    let mut rng = rand::thread_rng();
    let mut storage1 = db1.fork();
    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);

    for item in &data {
        index1.put(&item.0, item.1.clone());
    }

    let mut storage2 = db2.fork();
    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    rng.shuffle(&mut data);

    for item in &data {
        index2.put(&item.0, item.1.clone());
    }

    let saved_hash = index1.root_hash();

    let mut keys_to_remove = data.iter()
        .take(50)
        .map(|item| item.0.clone())
        .collect::<Vec<_>>();

    rng.shuffle(&mut keys_to_remove);
    for key in &keys_to_remove {
        index1.remove(key);
    }
    rng.shuffle(&mut keys_to_remove);
    for key in &keys_to_remove {
        index2.remove(key);
    }

    for key in &keys_to_remove {
        assert!(index1.get(key).is_none());
        assert!(index2.get(key).is_none());
    }

    assert!(index2.root_hash() != Hash::zero());
    assert_eq!(index2.root_hash(), index1.root_hash());

    for item in &data {
        index1.put(&item.0, item.1.clone());
    }
    rng.shuffle(&mut data);
    for item in &data {
        index2.put(&item.0, item.1.clone());
    }

    for item in &data {
        let v1 = index1.get(&item.0);
        let v2 = index2.get(&item.0);
        assert_eq!(v1.as_ref(), Some(&item.1));
        assert_eq!(v2.as_ref(), Some(&item.1));
    }
    assert_eq!(index2.root_hash(), index1.root_hash());
    assert_eq!(index2.root_hash(), saved_hash);
}

fn fuzz_insert_after_delete(db: Box<Database>) {
    let mut storage = db.fork();
    let mut index = ProofMapIndex::new(IDX_NAME, &mut storage);

    let data = generate_random_data(100);

    for item in &data[0..50] {
        index.put(&item.0, item.1.clone());
    }
    let saved_hash = index.root_hash();
    for item in &data[50..] {
        index.put(&item.0, item.1.clone());
    }
    for item in &data[50..] {
        index.remove(&item.0);
    }

    for item in &data[0..50] {
        let v1 = index.get(&item.0);
        assert_eq!(v1.as_ref(), Some(&item.1));
    }
    for item in &data[50..] {
        let v1 = index.get(&item.0);
        assert_eq!(v1.as_ref(), None);
    }
    assert_eq!(index.root_hash(), saved_hash);
}

fn iter(db: Box<Database>) {
    let mut fork = db.fork();
    let mut map_index = ProofMapIndex::new(IDX_NAME, &mut fork);

    let k0 = [0; 32];
    let k1 = [1; 32];
    let k2 = [2; 32];
    let k3 = [3; 32];
    let k4 = [4; 32];

    map_index.put(&k1, 1u8);
    map_index.put(&k2, 2u8);
    map_index.put(&k3, 3u8);

    assert_eq!(
        map_index.iter().collect::<Vec<([u8; 32], u8)>>(),
        vec![(k1, 1), (k2, 2), (k3, 3)]
    );

    assert_eq!(
        map_index.iter_from(&k0).collect::<Vec<([u8; 32], u8)>>(),
        vec![(k1, 1), (k2, 2), (k3, 3)]
    );
    assert_eq!(
        map_index.iter_from(&k1).collect::<Vec<([u8; 32], u8)>>(),
        vec![(k1, 1), (k2, 2), (k3, 3)]
    );
    assert_eq!(
        map_index.iter_from(&k2).collect::<Vec<([u8; 32], u8)>>(),
        vec![(k2, 2), (k3, 3)]
    );
    assert_eq!(
        map_index.iter_from(&k4).collect::<Vec<([u8; 32], u8)>>(),
        Vec::<([u8; 32], u8)>::new()
    );

    assert_eq!(
        map_index.keys().collect::<Vec<[u8; 32]>>(),
        vec![k1, k2, k3]
    );

    assert_eq!(
        map_index.keys_from(&k0).collect::<Vec<[u8; 32]>>(),
        vec![k1, k2, k3]
    );
    assert_eq!(
        map_index.keys_from(&k1).collect::<Vec<[u8; 32]>>(),
        vec![k1, k2, k3]
    );
    assert_eq!(
        map_index.keys_from(&k2).collect::<Vec<[u8; 32]>>(),
        vec![k2, k3]
    );
    assert_eq!(
        map_index.keys_from(&k4).collect::<Vec<[u8; 32]>>(),
        Vec::<[u8; 32]>::new()
    );

    assert_eq!(map_index.values().collect::<Vec<u8>>(), vec![1, 2, 3]);

    assert_eq!(
        map_index.values_from(&k0).collect::<Vec<u8>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        map_index.values_from(&k1).collect::<Vec<u8>>(),
        vec![1, 2, 3]
    );
    assert_eq!(map_index.values_from(&k2).collect::<Vec<u8>>(), vec![2, 3]);
    assert_eq!(
        map_index.values_from(&k4).collect::<Vec<u8>>(),
        Vec::<u8>::new()
    );
}

mod memorydb_tests {
    use std::path::Path;
    use tempdir::TempDir;
    use storage::{Database, MemoryDB};

    fn create_database(_: &Path) -> Box<Database> {
        Box::new(MemoryDB::new())
    }

    #[test]
    fn test_insert_trivial() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db1 = create_database(path1);
        let db2 = create_database(path2);
        super::insert_trivial(db1, db2);
    }

    #[test]
    fn test_insert_same_key() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::insert_same_key(db);
    }

    #[test]
    fn test_insert_simple() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db1 = create_database(path1);
        let db2 = create_database(path2);
        super::insert_simple(db1, db2);
    }

    #[test]
    fn test_insert_reverse() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::insert_reverse(db1, db2);
    }

    #[test]
    fn test_remove_trivial() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::remove_trivial(db1, db2);
    }

    #[test]
    fn remove_simple() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::remove_simple(db1, db2);
    }

    #[test]
    fn remove_reverse() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::remove_reverse(db1, db2);
    }

    #[test]
    fn test_fuzz_insert() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::fuzz_insert(db1, db2);
    }

    #[test]
    fn test_build_proof_in_empty_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_proof_in_empty_tree(db);
    }

    #[test]
    fn test_build_multiproof_in_empty_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_multiproof_in_empty_tree(db);
    }

    #[test]
    fn test_build_proof_in_single_node_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_proof_in_single_node_tree(db);
    }

    #[test]
    fn test_build_multiproof_in_single_node_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_multiproof_in_single_node_tree(db);
    }

    #[test]
    fn test_build_proof_in_multinode_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_proof_in_multinode_tree(db);
    }

    #[test]
    fn test_build_multiproof_simple() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_multiproof_simple(db);
    }

    #[test]
    fn test_fuzz_insert_build_proofs_in_table_filled_with_hashes() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::fuzz_insert_build_proofs_in_table_filled_with_hashes(db);
    }

    #[test]
    fn test_fuzz_insert_build_proofs() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::fuzz_insert_build_proofs(db);
    }

    #[test]
    fn test_fuzz_insert_build_multiproofs() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::fuzz_insert_build_multiproofs(db);
    }

    #[test]
    fn test_fuzz_delete_build_proofs() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::fuzz_delete_build_proofs(db);
    }

    #[test]
    fn test_fuzz_delete() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::fuzz_delete(db1, db2);
    }

    #[test]
    fn test_fuzz_insert_after_delete() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::fuzz_insert_after_delete(db);
    }

    #[test]
    fn test_iter() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::iter(db);
    }
}

#[cfg(feature = "rocksdb")]
mod rocksdb_tests {
    use std::path::Path;
    use tempdir::TempDir;
    use storage::{Database, RocksDB, RocksDBOptions};

    fn create_database(path: &Path) -> Box<Database> {
        let mut opts = RocksDBOptions::default();
        opts.create_if_missing(true);
        Box::new(RocksDB::open(path, opts).unwrap())
    }

    #[test]
    fn test_insert_trivial() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db1 = create_database(path1);
        let db2 = create_database(path2);
        super::insert_trivial(db1, db2);
    }

    #[test]
    fn test_insert_same_key() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::insert_same_key(db);
    }

    #[test]
    fn test_insert_simple() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db1 = create_database(path1);
        let db2 = create_database(path2);
        super::insert_simple(db1, db2);
    }

    #[test]
    fn test_insert_reverse() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::insert_reverse(db1, db2);
    }

    #[test]
    fn test_remove_trivial() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::remove_trivial(db1, db2);
    }

    #[test]
    fn remove_simple() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::remove_simple(db1, db2);
    }

    #[test]
    fn remove_reverse() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::remove_reverse(db1, db2);
    }

    #[test]
    fn test_fuzz_insert() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::fuzz_insert(db1, db2);
    }

    #[test]
    fn test_build_proof_in_empty_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_proof_in_empty_tree(db);
    }

    #[test]
    fn test_build_multiproof_in_empty_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_multiproof_in_empty_tree(db);
    }

    #[test]
    fn test_build_proof_in_single_node_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_proof_in_single_node_tree(db);
    }

    #[test]
    fn test_build_multiproof_in_single_node_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_multiproof_in_single_node_tree(db);
    }

    #[test]
    fn test_build_proof_in_multinode_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_proof_in_multinode_tree(db);
    }

    #[test]
    fn test_build_multiproof_simple() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_multiproof_simple(db);
    }

    #[test]
    fn test_fuzz_insert_build_proofs_in_table_filled_with_hashes() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::fuzz_insert_build_proofs_in_table_filled_with_hashes(db);
    }

    #[test]
    fn test_fuzz_insert_build_proofs() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::fuzz_insert_build_proofs(db);
    }

    #[test]
    fn test_fuzz_insert_build_multiproofs() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::fuzz_insert_build_multiproofs(db);
    }

    #[test]
    fn test_fuzz_delete_build_proofs() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::fuzz_delete_build_proofs(db);
    }

    #[test]
    fn test_fuzz_delete() {
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let db1 = create_database(path1);
        let dir2 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path2 = dir2.path();
        let db2 = create_database(path2);
        super::fuzz_delete(db1, db2);
    }

    #[test]
    fn test_fuzz_insert_after_delete() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::fuzz_insert_after_delete(db);
    }

    #[test]
    fn test_iter() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::iter(db);
    }
}
