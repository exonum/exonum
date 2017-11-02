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

use std::collections::HashSet;
use rand::{thread_rng, Rng};
use crypto::{hash, Hash, HashStream};
use storage::db::Database;
use encoding::serialize::json::reexport::to_string;
use encoding::serialize::reexport::{Serialize, Serializer};

use super::{DBKey, ProofMapIndex};
use super::proof::MapProof;
use super::key::{KEY_SIZE, LEAF_KEY_PREFIX};

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

fn build_proof_in_empty_tree(db: Box<Database>) {
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);

    // Just to notify the compiler of the types used; same key is added and then removed from tree.
    table.put(&[230; 32], vec![1]);
    table.remove(&[230; 32]);

    let search_res = table.get_proof(&[244; 32]);
    match search_res {
        MapProof::Empty => {}
        _ => assert!(false),
    }
    {
        let check_res = search_res.validate(&[244u8; 32], table.root_hash());
        assert!(check_res.unwrap().is_none());
    }
}

fn build_proof_in_leaf_tree(db: Box<Database>) {
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);
    let root_key = [230u8; 32];
    let root_val = vec![2];
    let searched_key = [244; 32];

    table.put(&root_key, root_val.clone());
    let table_root = table.root_hash();
    let proof_path = table.get_proof(&searched_key);

    {
        let check_res = proof_path.validate(&searched_key, table_root).unwrap();
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
    assert_eq!(table_root, proof_path.root_hash());
    {
        let check_res = proof_path.validate(&root_key, table_root).unwrap();
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

fn fuzz_insert_build_proofs_in_table_filled_with_hashes(db: Box<Database>) {
    let data: Vec<(Hash, Hash)> = generate_fully_random_data_keys(100)
        .into_iter()
        .map(|el| {
            let (key, val) = el;
            (hash(&key), hash(&val))
        })
        .collect::<Vec<_>>();

    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);
    for item in &data {
        table.put(&item.0, item.1.clone());
    }

    let table_root_hash = table.root_hash();
    let item = data[0];
    let proof_path_to_key = table.get_proof(&item.0);
    assert_eq!(proof_path_to_key.root_hash(), table_root_hash);

    let proof_info = ProofInfo {
        root_hash: table_root_hash,
        searched_key: &item.0,
        proof: &proof_path_to_key,
        key_found: true,
    };

    let json_repre = to_string(&proof_info).unwrap();
    assert!(json_repre.len() > 0);
    let check_res = proof_path_to_key.validate(&item.0, table_root_hash);
    let proved_value: Option<&Hash> = check_res.unwrap();
    assert_eq!(proved_value.unwrap(), &item.1);
}

fn fuzz_insert_build_proofs(db: Box<Database>) {
    let data = generate_fully_random_data_keys(100);
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(IDX_NAME, &mut storage);
    for item in &data {
        table.put(&item.0, item.1.clone());
    }

    let table_root_hash = table.root_hash();

    for item in &data {
        let proof_path_to_key = table.get_proof(&item.0);
        assert_eq!(proof_path_to_key.root_hash(), table_root_hash);
        let check_res = proof_path_to_key.validate(&item.0, table_root_hash);
        let proved_value: Option<&Vec<u8>> = check_res.unwrap();
        assert_eq!(proved_value.unwrap(), &item.1);

        let proof_info = ProofInfo {
            root_hash: table_root_hash,
            searched_key: &item.0,
            proof: &proof_path_to_key,
            key_found: true,
        };

        let json_repre = to_string(&proof_info).unwrap();
        assert!(json_repre.len() > 0);
    }
}

fn fuzz_delete_build_proofs(db: Box<Database>) {
    let data = generate_fully_random_data_keys(100);
    let mut rng = rand::thread_rng();
    let mut storage = db.fork();
    let mut index = ProofMapIndex::new(IDX_NAME, &mut storage);
    for item in &data {
        index.put(&item.0, item.1.clone());
    }

    let mut keys_to_remove = data.iter()
        .take(50)
        .map(|item| item.0.clone())
        .collect::<Vec<_>>();

    rng.shuffle(&mut keys_to_remove);
    for key in &keys_to_remove {
        index.remove(key);
    }
    let table_root_hash = index.root_hash();
    for key in &keys_to_remove {
        let proof_path_to_key = index.get_proof(key);
        assert_eq!(proof_path_to_key.root_hash(), table_root_hash);
        let check_res = proof_path_to_key.validate(key, table_root_hash);
        assert!(check_res.is_ok());
        let proved_value: Option<&Vec<u8>> = check_res.unwrap();
        assert!(proved_value.is_none());
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

fn bytes_to_hex<T: AsRef<[u8]> + ?Sized>(bytes: &T) -> String {
    let strs: Vec<String> = bytes
        .as_ref()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    strs.join("")
}

fn serialize_str_u8<S, A>(data: &A, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    A: AsRef<[u8]>,
{
    serializer.serialize_str(&bytes_to_hex(data.as_ref()))
}

#[derive(Serialize)]
struct ProofInfo<'a, A: AsRef<[u8]>, V: Serialize + 'a> {
    root_hash: Hash,
    #[serde(serialize_with = "serialize_str_u8")]
    searched_key: A,
    proof: &'a MapProof<V>,
    key_found: bool,
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
    fn test_build_proof_in_leaf_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_proof_in_leaf_tree(db);
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
    fn test_build_proof_in_leaf_tree() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let db = create_database(path);
        super::build_proof_in_leaf_tree(db);
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
