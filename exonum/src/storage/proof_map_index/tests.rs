extern crate rand;

use std::collections::HashSet;

use rand::{thread_rng, Rng};

use crypto::{hash, Hash};
use super::super::{Database, MemoryDB};
use ::encoding::serialize::json::reexport::to_string;

use super::{DBKey, ProofMapIndex};
use super::proof::MapProof;
use super::key::{KEY_SIZE, LEAF_KEY_PREFIX};

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

#[test]
fn insert_trivial() {
    let mut storage1 = MemoryDB::new().fork();
    let mut storage2 = MemoryDB::new().fork();

    let mut index1 = ProofMapIndex::new(vec![255], &mut storage1);
    index1.put(&[255; 32], vec![1]);
    index1.put(&[254; 32], vec![2]);

    let mut index2 = ProofMapIndex::new(vec![255], &mut storage2);
    index2.put(&[254; 32], vec![2]);
    index2.put(&[255; 32], vec![1]);

    assert_eq!(index1.get(&[255; 32]), Some(vec![1]));
    assert_eq!(index1.get(&[254; 32]), Some(vec![2]));
    assert_eq!(index2.get(&[255; 32]), Some(vec![1]));
    assert_eq!(index2.get(&[254; 32]), Some(vec![2]));

    assert!(index1.root_hash() != Hash::zero());
    assert_eq!(index1.root_hash(), index2.root_hash());
}

#[test]
fn insert_same_key() {
    let mut storage = MemoryDB::new().fork();
    let mut table = ProofMapIndex::new(vec![255], &mut storage);
    assert_eq!(table.root_hash(), Hash::zero());
    let root_prefix = &[&[LEAF_KEY_PREFIX], vec![255; 32].as_slice(), &[0u8]].concat();
    let hash = hash(&[root_prefix, hash(&[2]).as_ref()].concat());

    table.put(&[255; 32], vec![1]);
    table.put(&[255; 32], vec![2]);
    assert_eq!(table.get(&[255; 32]), Some(vec![2]));
    assert_eq!(table.root_hash(), hash);
}

#[test]
fn insert_simple() {
    let mut storage1 = MemoryDB::new().fork();
    let mut storage2 = MemoryDB::new().fork();

    let mut index1 = ProofMapIndex::new(vec![255], &mut storage1);
    index1.put(&[255; 32], vec![3]);
    index1.put(&[254; 32], vec![2]);
    index1.put(&[250; 32], vec![1]);
    index1.put(&[254; 32], vec![5]);

    let mut index2 = ProofMapIndex::new(vec![255], &mut storage2);
    index2.put(&[250; 32], vec![1]);
    index2.put(&[254; 32], vec![2]);
    index2.put(&[255; 32], vec![3]);
    index2.put(&[254; 32], vec![5]);

    assert!(index1.root_hash() != Hash::zero());
    assert_eq!(index1.root_hash(), index2.root_hash());
}

#[test]
fn insert_reverse() {
    let mut storage1 = MemoryDB::new().fork();
    let mut index1 = ProofMapIndex::new(vec![255], &mut storage1);
    index1.put(&[42; 32], vec![1]);
    index1.put(&[64; 32], vec![2]);
    index1.put(&[240; 32], vec![3]);
    index1.put(&[245; 32], vec![4]);
    index1.put(&[250; 32], vec![5]);
    index1.put(&[255; 32], vec![6]);

    let mut storage2 = MemoryDB::new().fork();
    let mut index2 = ProofMapIndex::new(vec![255], &mut storage2);
    index2.put(&[255; 32], vec![6]);
    index2.put(&[250; 32], vec![5]);
    index2.put(&[245; 32], vec![4]);
    index2.put(&[240; 32], vec![3]);
    index2.put(&[64; 32], vec![2]);
    index2.put(&[42; 32], vec![1]);


    assert!(index2.root_hash() != Hash::zero());
    assert_eq!(index2.root_hash(), index1.root_hash());
}

#[test]
fn remove_trivial() {
    let mut storage1 = MemoryDB::new().fork();
    let mut index1 = ProofMapIndex::new(vec![255], &mut storage1);
    index1.put(&[255; 32], vec![6]);
    index1.remove(&[255; 32]);

    let mut storage2 = MemoryDB::new().fork();
    let mut index2 = ProofMapIndex::new(vec![255], &mut storage2);
    index2.put(&[255; 32], vec![6]);
    index2.remove(&[255; 32]);

    assert_eq!(index1.root_hash(), Hash::zero());
    assert_eq!(index2.root_hash(), Hash::zero());
}

#[test]
fn remove_simple() {
    let mut storage1 = MemoryDB::new().fork();
    let mut index1 = ProofMapIndex::new(vec![255], &mut storage1);
    index1.put(&[255; 32], vec![1]);
    index1.put(&[250; 32], vec![2]);
    index1.put(&[245; 32], vec![3]);

    index1.remove(&[255; 32]);
    index1.remove(&[245; 32]);

    let mut storage2 = MemoryDB::new().fork();
    let mut index2 = ProofMapIndex::new(vec![255], &mut storage2);
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

#[test]
fn remove_reverse() {
    let mut storage1 = MemoryDB::new().fork();
    let mut index1 = ProofMapIndex::new(vec![255], &mut storage1);
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

    let mut storage2 = MemoryDB::new().fork();
    let mut index2 = ProofMapIndex::new(vec![255], &mut storage2);
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

#[test]
fn fuzz_insert() {
    let mut data = generate_random_data(100);
    let mut rng = rand::thread_rng();

    let mut storage1 = MemoryDB::new().fork();
    let mut index1 = ProofMapIndex::new(vec![255], &mut storage1);
    for item in &data {
        index1.put(&item.0, item.1.clone());
    }

    let mut storage2 = MemoryDB::new().fork();
    let mut index2 = ProofMapIndex::new(vec![255], &mut storage2);
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

#[test]
fn build_proof_in_empty_tree() {
    let mut storage = MemoryDB::new().fork();
    let mut table = ProofMapIndex::new(vec![255], &mut storage);

    table.put(&[230; 32], vec![1]); //just to notify the compiler of the types used; same key is added and then removed from tree
    table.remove(&[230; 32]);

    let search_res = table.get_proof(&[244; 32]);
    match search_res {
        MapProof::Empty => {}
        _ => assert!(false),
    }
    {
        let check_res = search_res.verify_root_proof_consistency(&[244u8; 32], table.root_hash());
        assert!(check_res.unwrap().is_none());
    }
}

#[test]
fn build_proof_in_leaf_tree() {
    let mut storage = MemoryDB::new().fork();
    let mut table = ProofMapIndex::new(vec![255], &mut storage);
    let root_key = [230u8; 32];
    let root_val = vec![2];
    let searched_key = [244; 32];

    table.put(&root_key, root_val.clone());
    let table_root = table.root_hash();
    let proof_path = table.get_proof(&searched_key);

    {
        let check_res = proof_path
            .verify_root_proof_consistency(&searched_key, table_root)
            .unwrap();
        assert!(check_res.is_none());
    }

    match proof_path {
        MapProof::LeafRootExclusive(key, hash_val) => {
            assert_eq!(key, DBKey::leaf(&root_key));
            assert_eq!(hash_val, hash(&root_val));
        }
        _ => assert!(false),
    }

    let proof_path = table.get_proof(&root_key);
    assert_eq!(table_root, proof_path.compute_proof_root());
    {
        let check_res = proof_path
            .verify_root_proof_consistency(&root_key, table_root)
            .unwrap();
        assert_eq!(check_res.unwrap(), &root_val);
    }
    match proof_path {
        MapProof::LeafRootInclusive(key, val) => {
            assert_eq!(key, DBKey::leaf(&root_key));
            assert_eq!(val, root_val);
        }
        _ => assert!(false),
    }
}

#[test]
fn fuzz_insert_build_proofs_in_table_filled_with_hashes() {
    let data: Vec<(Hash, Hash)> = generate_fully_random_data_keys(100)
        .into_iter()
        .map(|el| {
                 let (key, val) = el;
                 (hash(&key), hash(&val))
             })
        .collect::<Vec<_>>();

    let mut storage = MemoryDB::new().fork();
    let mut table = ProofMapIndex::new(vec![255], &mut storage);
    for item in &data {
        table.put(&item.0, item.1.clone());
    }

    let table_root_hash = table.root_hash();
    let item = data[0];
    let proof_path_to_key = table.get_proof(&item.0);
    assert_eq!(proof_path_to_key.compute_proof_root(), table_root_hash);
    let check_res = proof_path_to_key.verify_root_proof_consistency(&item.0, table_root_hash);
    let proved_value: Option<&Hash> = check_res.unwrap();
    assert_eq!(proved_value.unwrap(), &item.1);
}

#[test]
fn fuzz_insert_build_proofs() {
    let _ = ::helpers::init_logger();
    let data = generate_fully_random_data_keys(100);

    let mut storage = MemoryDB::new().fork();
    let mut table = ProofMapIndex::new(vec![255], &mut storage);
    for item in &data {
        table.put(&item.0, item.1.clone());
    }

    let table_root_hash = table.root_hash();

    for item in &data {
        let proof_path_to_key = table.get_proof(&item.0);
        assert_eq!(proof_path_to_key.compute_proof_root(), table_root_hash);
        let check_res = proof_path_to_key.verify_root_proof_consistency(&item.0, table_root_hash);
        let proved_value: Option<&Vec<u8>> = check_res.unwrap();
        assert_eq!(proved_value.unwrap(), &item.1);
        let json_repre = to_string(&proof_path_to_key).unwrap();
        info!("{}", json_repre);
    }
}

#[test]
fn fuzz_delete_build_proofs() {
    let data = generate_fully_random_data_keys(100);
    let mut rng = rand::thread_rng();

    let mut storage1 = MemoryDB::new().fork();
    let mut index1 = ProofMapIndex::new(vec![255], &mut storage1);
    for item in &data {
        index1.put(&item.0, item.1.clone());
    }

    let mut keys_to_remove = data.iter()
        .take(50)
        .map(|item| item.0.clone())
        .collect::<Vec<_>>();

    rng.shuffle(&mut keys_to_remove);
    for key in &keys_to_remove {
        index1.remove(key);
    }
    let table_root_hash = index1.root_hash();
    for key in &keys_to_remove {
        let proof_path_to_key = index1.get_proof(key);
        assert_eq!(proof_path_to_key.compute_proof_root(), table_root_hash);
        let check_res = proof_path_to_key.verify_root_proof_consistency(key, table_root_hash);
        assert!(check_res.is_ok());
        let proved_value: Option<&Vec<u8>> = check_res.unwrap();
        assert!(proved_value.is_none());
    }
}

#[test]
fn fuzz_delete() {
    let mut data = generate_random_data(100);
    let mut rng = rand::thread_rng();

    let mut storage1 = MemoryDB::new().fork();
    let mut index1 = ProofMapIndex::new(vec![255], &mut storage1);
    for item in &data {
        index1.put(&item.0, item.1.clone());
    }

    let mut storage2 = MemoryDB::new().fork();
    let mut index2 = ProofMapIndex::new(vec![255], &mut storage2);
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

#[test]
fn fuzz_insert_after_delete() {
    let mut storage1 = MemoryDB::new().fork();
    let mut index1 = ProofMapIndex::new(vec![255], &mut storage1);

    let data = generate_random_data(100);

    for item in &data[0..50] {
        index1.put(&item.0, item.1.clone());
    }
    let saved_hash = index1.root_hash();
    for item in &data[50..] {
        index1.put(&item.0, item.1.clone());
    }
    for item in &data[50..] {
        index1.remove(&item.0);
    }

    for item in &data[0..50] {
        let v1 = index1.get(&item.0);
        assert_eq!(v1.as_ref(), Some(&item.1));
    }
    for item in &data[50..] {
        let v1 = index1.get(&item.0);
        assert_eq!(v1.as_ref(), None);
    }
    assert_eq!(index1.root_hash(), saved_hash);
}


#[test]
fn test_iter() {
    let mut fork = MemoryDB::new().fork();
    let mut map_index = ProofMapIndex::new(vec![255], &mut fork);

    let k0 = [0; 32];
    let k1 = [1; 32];
    let k2 = [2; 32];
    let k3 = [3; 32];
    let k4 = [4; 32];

    map_index.put(&k1, 1u8);
    map_index.put(&k2, 2u8);
    map_index.put(&k3, 3u8);

    assert_eq!(map_index.iter().collect::<Vec<([u8; 32], u8)>>(),
               vec![(k1, 1), (k2, 2), (k3, 3)]);

    assert_eq!(map_index.iter_from(&k0).collect::<Vec<([u8; 32], u8)>>(),
               vec![(k1, 1), (k2, 2), (k3, 3)]);
    assert_eq!(map_index.iter_from(&k1).collect::<Vec<([u8; 32], u8)>>(),
               vec![(k1, 1), (k2, 2), (k3, 3)]);
    assert_eq!(map_index.iter_from(&k2).collect::<Vec<([u8; 32], u8)>>(),
               vec![(k2, 2), (k3, 3)]);
    assert_eq!(map_index.iter_from(&k4).collect::<Vec<([u8; 32], u8)>>(),
               Vec::<([u8; 32], u8)>::new());

    assert_eq!(map_index.keys().collect::<Vec<[u8; 32]>>(),
               vec![k1, k2, k3]);

    assert_eq!(map_index.keys_from(&k0).collect::<Vec<[u8; 32]>>(),
               vec![k1, k2, k3]);
    assert_eq!(map_index.keys_from(&k1).collect::<Vec<[u8; 32]>>(),
               vec![k1, k2, k3]);
    assert_eq!(map_index.keys_from(&k2).collect::<Vec<[u8; 32]>>(),
               vec![k2, k3]);
    assert_eq!(map_index.keys_from(&k4).collect::<Vec<[u8; 32]>>(),
               Vec::<[u8; 32]>::new());

    assert_eq!(map_index.values().collect::<Vec<u8>>(), vec![1, 2, 3]);

    assert_eq!(map_index.values_from(&k0).collect::<Vec<u8>>(),
               vec![1, 2, 3]);
    assert_eq!(map_index.values_from(&k1).collect::<Vec<u8>>(),
               vec![1, 2, 3]);
    assert_eq!(map_index.values_from(&k2).collect::<Vec<u8>>(), vec![2, 3]);
    assert_eq!(map_index.values_from(&k4).collect::<Vec<u8>>(),
               Vec::<u8>::new());
}
