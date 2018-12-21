// Copyright 2018 The Exonum Team
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

use std::{cmp, collections::HashSet, fmt::Debug, hash::Hash as StdHash};

use byteorder::{ByteOrder, LittleEndian};
use rand::{
    self,
    seq::{IteratorRandom, SliceRandom},
    Rng, RngCore, SeedableRng,
};
use rand_xorshift::XorShiftRng;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{self, json};

use exonum_crypto::{hash, CryptoHash, Hash, HashStream};

use super::{
    key::{BitsRange, ChildKind, KEY_SIZE, LEAF_KEY_PREFIX},
    node::BranchNode,
    proof::MapProofBuilder,
    HashedKey, MapProof, MapProofError, ProofMapIndex, ProofMapKey, ProofPath,
};
use crate::{BinaryValue, Database, Fork, TemporaryDB, UniqueHash};

const IDX_NAME: &'static str = "idx_name";

// Makes large data set with unique keys
fn generate_random_data(len: usize) -> Vec<([u8; KEY_SIZE], Vec<u8>)> {
    let mut rng = rand::thread_rng();
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
fn generate_random_data_keys<R: Rng>(len: usize, rng: &mut R) -> Vec<([u8; KEY_SIZE], Vec<u8>)> {
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

#[test]
fn test_map_methods() {
    let db = TemporaryDB::default();
    let mut fork = db.fork();
    let mut index = ProofMapIndex::new(IDX_NAME, &mut fork);

    assert_eq!(index.get(&[1; 32]), None);
    assert!(!index.contains(&[1; 32]));

    index.put(&[1; 32], 1u8);

    assert_eq!(index.get(&[1; 32]), Some(1u8));
    assert!(index.contains(&[1; 32]));

    index.remove(&[1; 32]);

    assert!(!index.contains(&[1; 32]));
    assert_eq!(index.get(&[1; 32]), None);

    index.put(&[2; 32], 2u8);
    index.put(&[3; 32], 3u8);
    index.clear();

    assert!(!index.contains(&[2; 32]));
    assert!(!index.contains(&[3; 32]));
}

#[test]
fn test_insert_trivial() {
    let db1 = TemporaryDB::default();
    let db2 = TemporaryDB::default();
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

    assert_ne!(index1.merkle_root(), Hash::zero());
    assert_eq!(index1.merkle_root(), index2.merkle_root());
}

#[test]
fn test_insert_same_key() {
    let db = TemporaryDB::default();
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);
    assert_eq!(table.merkle_root(), Hash::zero());
    let root_prefix = &[&[LEAF_KEY_PREFIX], vec![255; 32].as_slice(), &[0_u8]].concat();
    let hash = HashStream::new()
        .update(root_prefix)
        .update(hash(&[2]).as_ref())
        .hash();

    table.put(&[255; 32], vec![1]);
    table.put(&[255; 32], vec![2]);
    assert_eq!(table.get(&[255; 32]), Some(vec![2]));
    assert_eq!(table.merkle_root(), hash);
}

#[test]
fn test_insert_simple() {
    let db1 = TemporaryDB::default();
    let db2 = TemporaryDB::default();
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

    assert!(index1.merkle_root() != Hash::zero());
    assert_eq!(index1.merkle_root(), index2.merkle_root());
}

#[test]
fn test_insert_reverse() {
    let db1 = TemporaryDB::default();
    let db2 = TemporaryDB::default();
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

    assert!(index2.merkle_root() != Hash::zero());
    assert_eq!(index2.merkle_root(), index1.merkle_root());
}

#[test]
fn test_remove_trivial() {
    let db1 = TemporaryDB::default();
    let db2 = TemporaryDB::default();
    let mut storage1 = db1.fork();
    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);
    index1.put(&[255; 32], vec![6]);
    index1.remove(&[255; 32]);

    let mut storage2 = db2.fork();
    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    index2.put(&[255; 32], vec![6]);
    index2.remove(&[255; 32]);

    assert_eq!(index1.merkle_root(), Hash::zero());
    assert_eq!(index2.merkle_root(), Hash::zero());
}

#[test]
fn test_remove_simple() {
    let db1 = TemporaryDB::default();
    let db2 = TemporaryDB::default();
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

    assert_eq!(index1.merkle_root(), index2.merkle_root());
}

#[test]
fn test_remove_reverse() {
    let db1 = TemporaryDB::default();
    let db2 = TemporaryDB::default();
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

    assert_eq!(index2.merkle_root(), index1.merkle_root());
}

#[test]
fn test_fuzz_insert() {
    let db1 = TemporaryDB::default();
    let db2 = TemporaryDB::default();
    let mut data = generate_random_data(100);
    let mut rng = rand::thread_rng();
    let mut storage1 = db1.fork();
    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);

    for item in &data {
        index1.put(&item.0, item.1.clone());
    }

    let mut storage2 = db2.fork();
    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    data.shuffle(&mut rng);
    for item in &data {
        index2.put(&item.0, item.1.clone());
    }

    for item in &data {
        let v1 = index1.get(&item.0);
        let v2 = index2.get(&item.0);
        assert_eq!(v1.as_ref(), Some(&item.1));
        assert_eq!(v2.as_ref(), Some(&item.1));
    }

    assert!(index2.merkle_root() != Hash::zero());
    assert_eq!(index2.merkle_root(), index1.merkle_root());

    // Test same keys
    data.shuffle(&mut rng);
    for item in &data {
        index1.put(&item.0, vec![1]);
    }
    data.shuffle(&mut rng);
    for item in &data {
        index2.put(&item.0, vec![1]);
    }

    for item in &data {
        let v1 = index1.get(&item.0);
        let v2 = index2.get(&item.0);
        assert_eq!(v1.as_ref(), Some(&vec![1]));
        assert_eq!(v2.as_ref(), Some(&vec![1]));
    }
    assert_eq!(index2.merkle_root(), index1.merkle_root());
}

fn check_map_proof<K, V>(
    proof: MapProof<K, V>,
    key: Option<K>,
    table: &ProofMapIndex<&mut Fork, K, V>,
) where
    K: ProofMapKey + PartialEq + Debug + Serialize + DeserializeOwned,
    V: BinaryValue + UniqueHash + PartialEq + Debug + Serialize + DeserializeOwned,
{
    let serialized_proof = serde_json::to_value(&proof).unwrap();
    let deserialized_proof: MapProof<K, V> = serde_json::from_value(serialized_proof).unwrap();

    let entries = match key {
        Some(key) => {
            let value = table.get(&key).unwrap();
            vec![(key, value)]
        }
        None => vec![],
    };

    let proof = proof.check().unwrap();
    assert_eq!(
        proof.entries().collect::<Vec<_>>(),
        entries
            .iter()
            .map(|&(ref k, ref v)| (k, v))
            .collect::<Vec<_>>()
    );
    assert_eq!(proof.merkle_root(), table.merkle_root());

    let deserialized_proof = deserialized_proof.check().unwrap();
    assert_eq!(
        deserialized_proof.entries().collect::<Vec<_>>(),
        proof.entries().collect::<Vec<_>>()
    );
    assert_eq!(deserialized_proof.merkle_root(), proof.merkle_root());
}

fn check_map_multiproof<K, V>(
    proof: MapProof<K, V>,
    keys: Vec<K>,
    table: &ProofMapIndex<&mut Fork, K, V>,
) where
    K: ProofMapKey + Clone + PartialEq + Debug,
    V: BinaryValue + UniqueHash + PartialEq + Debug,
{
    let (entries, missing_keys) = {
        let mut entries: Vec<(K, V)> = Vec::new();
        let mut missing_keys: Vec<K> = Vec::new();

        for key in keys {
            if table.contains(&key) {
                let value = table.get(&key).unwrap();
                entries.push((key, value));
            } else {
                missing_keys.push(key);
            }
        }

        // Sort entries and missing keys by the order imposed by the `ProofPath`
        // serialization of the keys
        entries.sort_unstable_by(|&(ref x, _), &(ref y, _)| {
            ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap()
        });
        missing_keys
            .sort_unstable_by(|x, y| ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap());

        (entries, missing_keys)
    };

    let proof = proof.check().unwrap();
    assert_eq!(proof.merkle_root(), table.merkle_root());
    assert_eq!(missing_keys.iter().collect::<Vec<&_>>(), {
        let mut actual_keys = proof.missing_keys().collect::<Vec<_>>();
        actual_keys
            .sort_unstable_by(|&x, &y| ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap());
        actual_keys
    });
    assert_eq!(
        entries
            .iter()
            .map(|&(ref k, ref v)| (k, v))
            .collect::<Vec<_>>(),
        {
            let mut actual_entries = proof.entries().collect::<Vec<_>>();
            actual_entries.sort_unstable_by(|&(x, _), &(y, _)| {
                ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap()
            });
            actual_entries
        }
    );
}

const MAX_CHECKED_ELEMENTS: usize = 1_024;

fn check_proofs_for_data<K, V>(db: &dyn Database, data: Vec<(K, V)>, nonexisting_keys: Vec<K>)
where
    K: ProofMapKey + Copy + PartialEq + Debug + Serialize + DeserializeOwned,
    V: BinaryValue + UniqueHash + Clone + PartialEq + Debug + Serialize + DeserializeOwned,
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
        let mut rng = XorShiftRng::from_seed(rand::random());
        (0..batch_size).choose_multiple(&mut rng, MAX_CHECKED_ELEMENTS)
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

fn check_multiproofs_for_data<K, V>(db: &dyn Database, data: Vec<(K, V)>, nonexisting_keys: Vec<K>)
where
    K: ProofMapKey + Copy + Ord + PartialEq + StdHash + Debug + Serialize,
    V: BinaryValue + UniqueHash + Clone + PartialEq + Debug + Serialize,
{
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);
    for &(ref key, ref value) in &data {
        table.put(key, value.clone());
    }

    let mut rng = XorShiftRng::from_seed(rand::random());

    // Test for batches of 1, 11, ..., 101 keys
    for proof_size in (0..11).map(|x| x * 10 + 1) {
        // Check the multiproof only for existing keys
        let keys = data
            .iter()
            .map(|&(k, _)| k)
            .choose_multiple(&mut rng, proof_size);
        let proof = table.get_multiproof(keys.clone());
        check_map_multiproof(proof, keys, &table);

        // Check the multiproof for the equal number of existing and non-existing keys
        let mut keys = data
            .iter()
            .map(|&(k, _)| k)
            .choose_multiple(&mut rng, proof_size);
        let non_keys = nonexisting_keys
            .iter()
            .cloned()
            .choose_multiple(&mut rng, proof_size);
        keys.extend(non_keys);
        let proof = table.get_multiproof(keys.clone());
        check_map_multiproof(proof, keys, &table);
    }
}

#[test]
fn test_invalid_map_proofs() {
    use self::MapProofError::*;

    let h = hash(&vec![1]);

    let proof: MapProof<[u8; 32], Vec<u8>> = MapProofBuilder::new()
        .add_proof_entry(ProofPath::new(&[1; 32]).prefix(240), h)
        .create();
    match proof.check().unwrap_err() {
        NonTerminalNode(..) => {}
        e => panic!("expected non-terminal node error, got {}", e),
    }

    let json = json!({
        "proof": [
            { "path": "11", "hash": Hash::default() },
            { "path": "0", "hash": Hash::default() },
        ],
        "entries": []
    });
    let proof: MapProof<[u8; 32], Vec<u8>> = serde_json::from_value(json).unwrap();
    match proof.check().unwrap_err() {
        InvalidOrdering(..) => {}
        e => panic!("expected invalid ordering error, got {}", e),
    }

    let proof: MapProof<[u8; 32], Vec<u8>> = MapProofBuilder::new()
        .add_proof_entry(ProofPath::new(&[1; 32]).prefix(3), h)
        .add_proof_entry(ProofPath::new(&[1; 32]).prefix(77), h)
        .create();
    match proof.check().unwrap_err() {
        EmbeddedPaths { .. } => {}
        e => panic!("expected embedded paths error, got {}", e),
    }

    let proof: MapProof<[u8; 32], Vec<u8>> = MapProofBuilder::new()
        .add_proof_entry(ProofPath::new(&[1; 32]).prefix(3), h)
        .add_entry([1; 32], vec![1, 2, 3])
        .create();
    match proof.check().unwrap_err() {
        EmbeddedPaths { .. } => {}
        e => panic!("expected embedded paths error, got {}", e),
    }

    let proof: MapProof<[u8; 32], Vec<u8>> = MapProofBuilder::new()
        .add_proof_entry(ProofPath::new(&[1; 32]).prefix(3), h)
        .add_entry([1; 32], vec![1, 2, 3])
        .create();
    match proof.check().unwrap_err() {
        EmbeddedPaths { .. } => {}
        e => panic!("expected embedded paths error, got {}", e),
    }

    let proof: MapProof<[u8; 32], Vec<u8>> = MapProofBuilder::new()
        .add_proof_entry(ProofPath::new(&[0; 32]).prefix(10), h)
        .add_proof_entry(ProofPath::new(&[1; 32]), h)
        .add_entry([1; 32], vec![1, 2, 3])
        .create();
    match proof.check().unwrap_err() {
        DuplicatePath(..) => {}
        e => panic!("expected duplicate path error, got {}", e),
    }

    let proof: MapProof<[u8; 32], Vec<u8>> = MapProofBuilder::new()
        .add_proof_entry(ProofPath::new(&[0; 32]).prefix(10), h)
        .add_entry([1; 32], vec![1, 2, 3])
        .add_entry([1; 32], vec![1, 2, 3])
        .create();
    match proof.check().unwrap_err() {
        DuplicatePath(..) => {}
        e => panic!("expected duplicate path error, got {}", e),
    }
}

#[test]
fn test_build_proof_in_empty_tree() {
    let db = TemporaryDB::default();
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);

    // Just to notify the compiler of the types used; same key is added and then removed from tree.
    table.put(&[230; 32], vec![1]);
    table.remove(&[230; 32]);

    let proof = table.get_proof([244; 32]);
    assert_eq!(proof.proof_unchecked(), vec![]);
    check_map_proof(proof, None, &table);
}

#[test]
fn test_build_multiproof_in_empty_tree() {
    let db = TemporaryDB::default();
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);

    // Just to notify the compiler of the types used; same key is added and then removed from tree.
    table.put(&[230; 32], vec![1]);
    table.remove(&[230; 32]);

    let keys = vec![[0; 32], [230; 32], [244; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(proof.proof_unchecked(), vec![]);
    check_map_multiproof(proof, keys, &table);
}

#[test]
fn test_build_proof_in_single_node_tree() {
    let db = TemporaryDB::default();
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);

    table.put(&[230; 32], vec![1]);
    let proof = table.get_proof([230; 32]);
    assert_eq!(proof.proof_unchecked(), vec![]);
    check_map_proof(proof, Some([230; 32]), &table);

    let proof = table.get_proof([128; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&[230; 32]), hash(&vec![1]))]
    );
    check_map_proof(proof, None, &table);
}

#[test]
fn test_build_multiproof_in_single_node_tree() {
    let db = TemporaryDB::default();
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);

    table.put(&[230; 32], vec![1]);

    let keys = vec![[230; 32], [1; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(proof.proof_unchecked(), vec![]);
    check_map_multiproof(proof, keys, &table);

    let keys = vec![[229; 32], [1; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&[230; 32]), hash(&vec![1]))]
    );
    check_map_multiproof(proof, keys, &table);
}

#[test]
fn test_build_proof_in_complex_tree() {
    let db = TemporaryDB::default();
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);

    table.put(&[128; 32], vec![1]); // 128 = 0b1000_0000 ~ ProofPath(00000001...)
    table.put(&[32; 32], vec![2]); //   32 = 0b0010_0000 ~ ProofPath(00000100...)
    let proof = table.get_proof([128; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&[32; 32]), hash(&vec![2]))]
    );
    check_map_proof(proof, Some([128; 32]), &table);

    let proof = table.get_proof([32; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&[128; 32]), hash(&vec![1]))]
    );
    check_map_proof(proof, Some([32; 32]), &table);

    // Key left of all keys in the tree
    let proof = table.get_proof([0; 32]);
    let exp_proof = vec![
        (ProofPath::new(&[128; 32]), hash(&vec![1])),
        (ProofPath::new(&[32; 32]), hash(&vec![2])),
    ];
    assert_eq!(proof.proof_unchecked(), exp_proof);
    check_map_proof(proof, None, &table);

    // Key between the keys in the tree
    let proof = table.get_proof([64; 32]); // 64 ~ ProofPath(00000010...)
    assert_eq!(proof.proof_unchecked(), exp_proof);
    check_map_proof(proof, None, &table);

    // Key to the right of all keys
    let proof = table.get_proof([255; 32]);
    assert_eq!(proof.proof_unchecked(), exp_proof);
    check_map_proof(proof, None, &table);

    // Insert key that splits 15-bit segment off the left key in the tree.
    // The key itself is to the left of the `[128; 32]` key.
    let left_key = {
        let mut key = [0; 32];
        key[0] = 128;
        table.put(&key, vec![3]);
        key
    };

    let left_hash = {
        let mut node = BranchNode::empty();
        node.set_child(ChildKind::Left, &ProofPath::new(&left_key), &hash(&vec![3]));
        node.set_child(
            ChildKind::Right,
            &ProofPath::new(&[128; 32]),
            &hash(&vec![1]),
        );
        node.hash()
    };

    let proof = table.get_proof([128; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&left_key), hash(&vec![3])),
            (ProofPath::new(&[32; 32]), hash(&vec![2])),
        ]
    );
    check_map_proof(proof, Some([128; 32]), &table);

    let proof = table.get_proof([64; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&left_key).prefix(15), left_hash),
            (ProofPath::new(&[32; 32]), hash(&vec![2])),
        ]
    );
    check_map_proof(proof, None, &table);

    let proof = table.get_proof([32; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&left_key).prefix(15), left_hash)]
    );
    check_map_proof(proof, Some([32; 32]), &table);

    let proof = table.get_proof([1; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&left_key).prefix(15), left_hash),
            (ProofPath::new(&[32; 32]), hash(&vec![2])),
        ]
    );
    check_map_proof(proof, None, &table);

    // Insert key that splits 12-bit segment off the [32; 32] key in the tree.
    // The key is to the right of the [32; 32] key.
    let right_key = {
        let mut key = [0; 32];
        key[0] = 32;
        key[1] = 0b1001_0000; // represented as `..00001001..` in `ProofPath`
        table.put(&key, vec![4]);
        key
    };

    let right_hash = {
        let mut node = BranchNode::empty();
        node.set_child(ChildKind::Left, &ProofPath::new(&[32; 32]), &hash(&vec![2]));
        node.set_child(
            ChildKind::Right,
            &ProofPath::new(&right_key),
            &hash(&vec![4]),
        );
        node.hash()
    };

    let proof = table.get_proof([128; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&left_key), hash(&vec![3])),
            (ProofPath::new(&right_key).prefix(12), right_hash),
        ]
    );
    check_map_proof(proof, Some([128; 32]), &table);

    // Non-existing key between two children at the root node
    let proof = table.get_proof([64; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&left_key).prefix(15), left_hash),
            (ProofPath::new(&right_key).prefix(12), right_hash),
        ]
    );
    check_map_proof(proof, None, &table);

    // Non-existing key between the first added node `[128; 32]` and the `left_key`.
    let nonexisting_key = {
        let mut key = [0; 32];
        key[0] = 128;
        key[1] = 128;
        key[29] = 29;
        key
    };

    let proof = table.get_proof(nonexisting_key);
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&left_key), hash(&vec![3])),
            (ProofPath::new(&[128; 32]), hash(&vec![1])),
            (ProofPath::new(&right_key).prefix(12), right_hash),
        ]
    );
    check_map_proof(proof, None, &table);

    let subtree_hash = table.merkle_root();
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
        proof.proof_unchecked(),
        vec![(ProofPath::new(&[0; 32]).prefix(5), subtree_hash)]
    );
    check_map_proof(proof, Some([129; 32]), &table);

    let proof = table.get_proof([1; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&[0; 32]).prefix(5), subtree_hash),
            (ProofPath::new(&[129; 32]), hash(&vec![5])),
        ]
    );
    check_map_proof(proof, None, &table);

    let proof = table.get_proof([32; 32]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&left_key).prefix(15), left_hash),
            (ProofPath::new(&right_key), hash(&vec![4])),
            (ProofPath::new(&[129; 32]), hash(&vec![5])),
        ]
    );
    check_map_proof(proof, Some([32; 32]), &table);
}

#[test]
fn test_build_multiproof_simple() {
    let db = TemporaryDB::default();
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);

    table.put(&[128; 32], vec![1]);
    table.put(&[32; 32], vec![2]);

    let keys = vec![[0; 32], [128; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&[32; 32]), hash(&vec![2]))]
    );
    check_map_multiproof(proof, keys, &table);

    let keys = vec![[64; 32], [0; 32], [128; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&[32; 32]), hash(&vec![2]))]
    );
    check_map_multiproof(proof, keys, &table);

    let keys = vec![[64; 32], [0; 32], [128; 32], [129; 32], [32; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(proof.proof_unchecked(), vec![]);
    check_map_multiproof(proof, keys, &table);

    let keys = vec![[64; 32], [0; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&[128; 32]), hash(&vec![1])),
            (ProofPath::new(&[32; 32]), hash(&vec![2])),
        ]
    );
    check_map_multiproof(proof, keys, &table);

    let keys = vec![[64; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&[128; 32]), hash(&vec![1])),
            (ProofPath::new(&[32; 32]), hash(&vec![2])),
        ]
    );
    check_map_multiproof(proof, keys, &table);

    let keys = vec![[64; 32], [64; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&[128; 32]), hash(&vec![1])),
            (ProofPath::new(&[32; 32]), hash(&vec![2])),
        ]
    );
    check_map_multiproof(proof, vec![[64; 32]], &table);

    let keys = vec![[128; 32], [64; 32], [128; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&[32; 32]), hash(&vec![2]))]
    );
    check_map_multiproof(proof, vec![[128; 32], [64; 32]], &table);

    let keys = vec![[255; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&[128; 32]), hash(&vec![1])),
            (ProofPath::new(&[32; 32]), hash(&vec![2])),
        ]
    );
    check_map_multiproof(proof, keys, &table);

    // Insert key that splits 15-bit segment off the left key in the tree.
    // The key itself is to the left of the `[1; 32]` key.
    let left_key = {
        let mut key = [0; 32];
        key[0] = 128;
        table.put(&key, vec![3]);
        key
    };

    let left_hash = {
        let mut node = BranchNode::empty();
        node.set_child(ChildKind::Left, &ProofPath::new(&left_key), &hash(&vec![3]));
        node.set_child(
            ChildKind::Right,
            &ProofPath::new(&[128; 32]),
            &hash(&vec![1]),
        );
        node.hash()
    };

    let proof = table.get_multiproof(vec![[0; 32]]);
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&[128; 32]).prefix(15), left_hash),
            (ProofPath::new(&[32; 32]), hash(&vec![2])),
        ]
    );
    check_map_multiproof(proof, vec![[0; 32]], &table);

    let keys = {
        // `key > [128; 32]`, which is visible from the `left_key` / `[128; 32]` junction
        let mut key = [128; 32];
        key[1] = 8;
        vec![[0; 32], key, [64; 32]]
    };
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&[128; 32]).prefix(15), left_hash),
            (ProofPath::new(&[32; 32]), hash(&vec![2])),
        ]
    );
    check_map_multiproof(proof, keys, &table);

    let keys = vec![[0; 32], [128; 32], [64; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&left_key), hash(&vec![3])),
            (ProofPath::new(&[32; 32]), hash(&vec![2])),
        ]
    );
    check_map_multiproof(proof, keys, &table);

    let keys = vec![[0; 32], [128; 32], [32; 32], [64; 32], [129; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&left_key), hash(&vec![3]))]
    );
    check_map_multiproof(proof, keys, &table);

    let keys = vec![[0; 32], [32; 32], [255; 32]];
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&[128; 32]).prefix(15), left_hash)]
    );
    check_map_multiproof(proof, keys, &table);

    let keys = {
        // `key` is between `left_key` and `[128; 32]`, so they both should be returned
        // in the proof to prove non-existence of `key`
        let mut key = [0; 32];
        key[0] = 128;
        key[1] = 128;
        vec![[0; 32], [32; 32], key]
    };
    let proof = table.get_multiproof(keys.clone());
    assert_eq!(
        proof.proof_unchecked(),
        vec![
            (ProofPath::new(&left_key), hash(&vec![3])),
            (ProofPath::new(&[128; 32]), hash(&vec![1])),
        ]
    );
    check_map_multiproof(proof, keys, &table);
}

#[test]
fn test_fuzz_insert_build_proofs_in_table_filled_with_hashes() {
    let db = TemporaryDB::default();
    let mut rng = XorShiftRng::from_seed(rand::random());
    let batch_sizes = (7..9).map(|x| 1 << x);

    for batch_size in batch_sizes {
        let data: Vec<(Hash, Hash)> = generate_random_data_keys(batch_size, &mut rng)
            .into_iter()
            .map(|(key, val)| (hash(&key), hash(&val)))
            .collect();

        let nonexisting_count = cmp::min(MAX_CHECKED_ELEMENTS, batch_size);
        let nonexisting_keys: Vec<_> = generate_random_data_keys(nonexisting_count / 2, &mut rng)
            .into_iter()
            .flat_map(|(key, val)| vec![hash(&key), hash(&val)])
            .collect();

        check_proofs_for_data(&db, data, nonexisting_keys);
    }
}

#[test]
fn test_fuzz_insert_build_proofs() {
    let db = TemporaryDB::default();
    let mut rng = XorShiftRng::from_seed(rand::random());
    let batch_sizes = (7..9).map(|x| (1 << x) - 1);

    for batch_size in batch_sizes {
        let data = generate_random_data_keys(batch_size, &mut rng);

        let nonexisting_count = cmp::min(MAX_CHECKED_ELEMENTS, batch_size);
        let nonexisting_keys: Vec<_> = generate_random_data_keys(nonexisting_count, &mut rng)
            .into_iter()
            .map(|(key, _)| key)
            .collect();

        check_proofs_for_data(&db, data, nonexisting_keys);
    }
}

#[test]
fn test_fuzz_insert_build_multiproofs() {
    let db = TemporaryDB::default();
    let mut rng = XorShiftRng::from_seed(rand::random());
    let batch_sizes = (7..9).map(|x| 1 << x);

    for batch_size in batch_sizes {
        let data = generate_random_data_keys(batch_size, &mut rng);

        let nonexisting_count = cmp::min(MAX_CHECKED_ELEMENTS, batch_size);
        let nonexisting_keys: Vec<_> = generate_random_data_keys(nonexisting_count, &mut rng)
            .into_iter()
            .map(|(key, _)| key)
            .collect();

        check_multiproofs_for_data(&db, data, nonexisting_keys);
    }
}

#[test]
fn test_fuzz_delete_build_proofs() {
    let db = TemporaryDB::default();
    const SAMPLE_SIZE: usize = 200;

    let mut rng = XorShiftRng::from_seed(rand::random());
    let data = generate_random_data_keys(SAMPLE_SIZE, &mut rng);

    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);
    for item in &data {
        table.put(&item.0, item.1.clone());
    }

    let (keys_to_remove, keys_to_remove_seq) = {
        let mut keys = data
            .iter()
            .map(|item| item.0.clone())
            .choose_multiple(&mut rng, SAMPLE_SIZE / 5);
        keys.shuffle(&mut rng);
        let seq_keys = keys.split_off(SAMPLE_SIZE / 10);
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

#[test]
fn test_fuzz_delete() {
    let db1 = TemporaryDB::default();
    let db2 = TemporaryDB::default();
    let mut data = generate_random_data(100);
    let mut rng = rand::thread_rng();
    let mut storage1 = db1.fork();
    let mut index1 = ProofMapIndex::new(IDX_NAME, &mut storage1);

    for item in &data {
        index1.put(&item.0, item.1.clone());
    }

    let mut storage2 = db2.fork();
    let mut index2 = ProofMapIndex::new(IDX_NAME, &mut storage2);
    data.shuffle(&mut rng);

    for item in &data {
        index2.put(&item.0, item.1.clone());
    }

    let saved_hash = index1.merkle_root();

    let mut keys_to_remove = data
        .iter()
        .take(50)
        .map(|item| item.0.clone())
        .collect::<Vec<_>>();

    keys_to_remove.shuffle(&mut rng);
    for key in &keys_to_remove {
        index1.remove(key);
    }
    keys_to_remove.shuffle(&mut rng);
    for key in &keys_to_remove {
        index2.remove(key);
    }

    for key in &keys_to_remove {
        assert!(index1.get(key).is_none());
        assert!(index2.get(key).is_none());
    }

    assert!(index2.merkle_root() != Hash::zero());
    assert_eq!(index2.merkle_root(), index1.merkle_root());

    for item in &data {
        index1.put(&item.0, item.1.clone());
    }
    data.shuffle(&mut rng);
    for item in &data {
        index2.put(&item.0, item.1.clone());
    }

    for item in &data {
        let v1 = index1.get(&item.0);
        let v2 = index2.get(&item.0);
        assert_eq!(v1.as_ref(), Some(&item.1));
        assert_eq!(v2.as_ref(), Some(&item.1));
    }
    assert_eq!(index2.merkle_root(), index1.merkle_root());
    assert_eq!(index2.merkle_root(), saved_hash);
}

#[test]
fn test_fuzz_insert_after_delete() {
    let db = TemporaryDB::default();
    let mut storage = db.fork();
    let mut index = ProofMapIndex::new(IDX_NAME, &mut storage);

    let data = generate_random_data(100);

    for item in &data[0..50] {
        index.put(&item.0, item.1.clone());
    }
    let saved_hash = index.merkle_root();
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
    assert_eq!(index.merkle_root(), saved_hash);
}

#[test]
fn test_iter() {
    let db = TemporaryDB::default();
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

#[test]
fn test_tree_with_hashed_key() {
    let db = TemporaryDB::default();
    use std::iter::FromIterator;

    #[derive(Debug, Copy, Clone, PartialEq)]
    struct Point {
        x: u16,
        y: u16,
    }

    impl Point {
        fn new(x: u16, y: u16) -> Self {
            Self { x, y }
        }
    }

    impl CryptoHash for Point {
        fn hash(&self) -> Hash {
            let mut buffer = [0; 4];
            LittleEndian::write_u16(&mut buffer[0..2], self.x);
            LittleEndian::write_u16(&mut buffer[2..4], self.y);
            exonum_crypto::hash(&buffer)
        }
    }

    impl HashedKey for Point {}

    fn hash_isolated_node(key: &ProofPath, h: &Hash) -> Hash {
        HashStream::new()
            .update(&key.as_bytes())
            .update(h.as_ref())
            .hash()
    }

    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);

    table.put(&Point::new(1, 2), vec![1, 2, 3]);
    table.put(&Point::new(3, 4), vec![2, 3, 4]);

    assert_eq!(table.get(&Point::new(1, 2)), Some(vec![1, 2, 3]));
    assert_eq!(table.get(&Point::new(2, 2)), None);

    let keys: HashSet<_> = table.keys().collect();
    assert_eq!(
        keys,
        HashSet::from_iter(vec![Point::new(3, 4).hash(), Point::new(1, 2).hash()])
    );

    let kvs: HashSet<_> = table.iter().collect();
    assert_eq!(
        kvs,
        HashSet::from_iter(vec![
            (Point::new(3, 4).hash(), vec![2, 3, 4]),
            (Point::new(1, 2).hash(), vec![1, 2, 3]),
        ])
    );

    let proof = table.get_proof(Point::new(1, 2));
    assert_eq!(
        proof.proof_unchecked(),
        vec![(ProofPath::new(&Point::new(3, 4)), hash(&vec![2, 3, 4]))]
    );
    let proof = proof.check().unwrap();
    assert_eq!(
        proof.all_entries().collect::<Vec<_>>(),
        vec![(&Point::new(1, 2), Some(&vec![1, 2, 3]))]
    );
    assert_eq!(proof.merkle_root(), table.merkle_root());

    let key = Point::new(3, 4);
    let other_key = Point::new(1, 2);
    table.remove(&key);
    let keys: Vec<_> = table.keys().collect();
    assert_eq!(keys, vec![other_key.hash()]);
    assert_eq!(table.get(&key), None);
    assert_eq!(table.get(&other_key), Some(vec![1, 2, 3]));
    assert_eq!(
        table.merkle_root(),
        hash_isolated_node(&ProofPath::new(&other_key.hash()), &hash(&vec![1, 2, 3]))
    );
}
