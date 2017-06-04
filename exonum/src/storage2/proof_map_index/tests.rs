extern crate rand;

use std::collections::HashSet;

use rand::{thread_rng, Rng};

use ::crypto::{hash, Hash};
use ::storage::{Map, Database, MemoryDB};
use ::storage::utils::bytes_to_hex;
use serde_json;
use serde::{Serialize, Serializer};

use super::super::MapTable;

use super::{BitSlice, BranchNode, MerklePatriciaTable, LEAF_KEY_PREFIX};
use super::proofpathtokey::RootProofNode;
use super::ChildKind::{Left, Right};
use super::KEY_SIZE;

impl BranchNode {
    fn new(hashes: [&Hash; 2], prefixes: [&BitSlice; 2]) -> BranchNode {
        let mut node = Self::empty();
        unsafe {
            let mut cursor = node.write_hash(0, hashes[0]);
            cursor = node.write_hash(cursor, hashes[1]);
            cursor = node.write_slice(cursor, prefixes[0]);
            cursor = node.write_slice(cursor, prefixes[1]);
            debug_assert_eq!(cursor, 132);
        }
        node
    }
}

fn serialize_str_u8<S, A>(data: &A, serializer: &mut S) -> Result<(), S::Error>
    where S: Serializer,
          A: AsRef<[u8]>
{
    serializer.serialize_str(&bytes_to_hex(data.as_ref()))
}
#[derive(Serialize)]
struct ProofInfo<'a, A: AsRef<[u8]>, V: Serialize + 'a> {
    root_hash: Hash,
    #[serde(serialize_with = "serialize_str_u8")]
    searched_key: A,
    proof: &'a RootProofNode<V>,
    key_found: bool,
}

// Makes large data set with unique keys
fn generate_random_data(len: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut rng = thread_rng();

    let mut exists_keys = HashSet::new();
    let mut base: Vec<u8> = vec![0; KEY_SIZE];
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

    (0..len)
        .map(kv_generator)
        .collect::<Vec<_>>()
}

// Makes large data set with unique keys
fn generate_fully_random_data_keys(len: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut rng = thread_rng();

    let mut exists_keys = HashSet::new();

    let kv_generator = |_| {
        let mut v = vec![0; 8];
        let mut new_key: Vec<u8> = vec![0; KEY_SIZE];
        rng.fill_bytes(&mut new_key);

        while exists_keys.contains(&new_key) {
            rng.fill_bytes(&mut new_key);
        }
        exists_keys.insert(new_key.clone());
        rng.fill_bytes(&mut v);
        (new_key, v)
    };

    (0..len)
        .map(kv_generator)
        .collect::<Vec<_>>()
}

#[test]
fn bitslice_mid() {
    let v1 = vec![1, 2, 255, 12];
    let b = BitSlice::from_bytes(&v1);

    assert_eq!(b.len(), 32);

    assert_eq!(b.at(0), Left);
    assert_eq!(b.at(7), Right);
    assert_eq!(b.at(8), Left);
    assert_eq!(b.at(14), Right);
    assert_eq!(b.at(15), Left);
    assert_eq!(b.at(16), Right);
    assert_eq!(b.at(20), Right);
    assert_eq!(b.at(23), Right);
    assert_eq!(b.at(31), Left);

    let b2 = b.mid(8);
    assert_eq!(b2.len(), 24);
    assert_eq!(b2.at(0), Left);
    assert_eq!(b2.at(6), Right);
    assert_eq!(b2.at(7), Left);
    assert_eq!(b2.at(12), Right);
    assert_eq!(b2.at(15), Right);

    let b3 = b2.mid(24);
    assert_eq!(b3.len(), 0);
}

#[test]
fn bitslice_truncate() {
    let v1 = vec![128];
    let b = BitSlice::from_bytes(&v1);

    assert_eq!(b.len(), 8);
    assert_eq!(b.truncate(1).at(0), Right);
    assert_eq!(b.truncate(1).len(), 1);
}

#[test]
fn bitslice_len() {
    let v1 = vec![255;32];
    let b = BitSlice::from_bytes(&v1);

    assert_eq!(b.len(), 256);
}

#[test]
#[should_panic]
fn bitslice_at_overflow() {
    let v1 = vec![8, 16, 32, 64];
    let b = BitSlice::from_bytes(&v1);
    b.at(32);
}

#[test]
#[should_panic]
fn bitslice_mid_overflow() {
    let v1 = vec![255;32];
    let b = BitSlice::from_bytes(&v1);
    b.mid(255).mid(2);
}

#[test]
#[should_panic]
fn bitslice_mid_at_overflow() {
    let v1 = vec![255;32];
    let b = BitSlice::from_bytes(&v1);
    b.mid(1).at(255);
}

#[test]
fn bitslice_common_prefix() {
    let b1 = BitSlice::from_bytes(b"abcd");
    let b2 = BitSlice::from_bytes(b"abde");
    assert_eq!(b1.common_prefix(&b1), 32);

    let c = b1.common_prefix(&b2);
    assert_eq!(c, 21);
    let c = b2.common_prefix(&b1);
    assert_eq!(c, 21);

    let b1 = b1.mid(9);
    let b2 = b2.mid(9);
    let c = b1.common_prefix(&b2);
    assert_eq!(c, 12);

    let v1 = &vec![255; 32];
    let v2 = &vec![254; 32];
    let b3 = BitSlice::from_bytes(v1);
    let b4 = BitSlice::from_bytes(v2);
    assert_eq!(b3.common_prefix(&b4), 7);
    assert_eq!(b4.common_prefix(&b3), 7);
    assert_eq!(b3.common_prefix(&b3), 256);
    let b3 = b3.mid(30);
    assert_eq!(b3.common_prefix(&b3), 226);
    let b3 = b3.truncate(200);
    assert_eq!(b3.common_prefix(&b3), 200);
    let b5 = BitSlice::from_bytes(v1);
    assert_eq!(b5.truncate(0).common_prefix(&b3), 0);
}

#[test]
fn bitslice_to_db_key() {
    let b = BitSlice::from_bytes(b"baceaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

    assert_eq!(b.to_db_key(),
               vec![1, 98, 97, 99, 101, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97,
                    97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 0]);
    assert_eq!(b.truncate(16).to_db_key(),
               vec![0, 98, 97, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 16]);
    assert_eq!(b.truncate(31).to_db_key(),
               vec![0, 98, 97, 99, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31]);
    assert_eq!(b.mid(8).truncate(8).to_db_key(),
               vec![0, 98, 97, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 16]);
    assert_eq!(b.mid(8).to_db_key(),
               vec![1, 98, 97, 99, 101, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97,
                    97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 0]);
    assert_eq!(b.mid(12).to_db_key(),
               vec![1, 98, 97, 99, 101, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97,
                    97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 0]);
    assert_eq!(b.mid(12).truncate(4).to_db_key(),
               vec![0, 98, 97, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 16]);
    assert_eq!(b.mid(12).truncate(19).to_db_key(),
               vec![0, 98, 97, 99, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31]);

    let b = BitSlice::from_bytes(b"p");
    assert_eq!(b.mid(4).to_db_key(),
               vec![1, 112, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn bitslice_from_db_key() {
    let b = BitSlice::from_bytes(b"qwertyuiopasdfghjklzxcvbnm123456");
    assert_eq!(b.len(), 256);
    assert_eq!(b.mid(4),
               BitSlice::from_db_key(&b.mid(4).to_db_key()).mid(4));
    assert_eq!(b.truncate(24),
               BitSlice::from_db_key(&b.truncate(24).to_db_key()));
}

#[test]
fn bitslice_is_leaf_key() {
    let b = BitSlice::from_bytes(b"qwertyuiopasdfghjklzxcvbnm123456");
    assert_eq!(b.len(), 256);
    assert_eq!(b.mid(4).is_leaf_key(), true);
    assert_eq!(b.mid(8).is_leaf_key(), true);
    assert_eq!(b.mid(250).is_leaf_key(), true);
    assert_eq!(b.truncate(16).is_leaf_key(), false);
}

#[test]
fn branch_node() {
    let mut rng = thread_rng();
    let mut gen_seq = || rng.gen_iter::<u8>().take(32).collect::<Vec<u8>>();

    let lh = Hash::from_slice(&gen_seq()).unwrap();
    let rh = Hash::from_slice(&gen_seq()).unwrap();
    let lp = BitSlice {
        data: &gen_seq(),
        from: 0,
        to: 16,
    };
    let rp = BitSlice {
        data: &gen_seq(),
        from: 31,
        to: 64,
    };
    let node = BranchNode::new([&lh, &rh], [&lp, &rp]);

    assert_eq!(node.child_hash(Left), &lh);
    assert_eq!(node.child_hash(Right), &rh);
    assert_eq!(node.child_slice(Left).to_db_key(), lp.to_db_key());
    assert_eq!(node.child_slice(Left).to, lp.to);
    assert_eq!(node.child_slice(Right).to_db_key(), rp.to_db_key());
    assert_eq!(node.child_slice(Right).to, rp.to);
    // assert_eq!(node.child_db_key(Left), lp.to_db_key().as_slice());
    // assert_eq!(node.child_db_key(Right), rp.to_db_key().as_slice());
}

#[test]
fn insert_trivial() {
    let storage1 = MemoryDB::new().fork();
    let storage2 = MemoryDB::new().fork();

    let table1 = MerklePatriciaTable::new(vec![255], &storage1);
    table1.put(&vec![255; 32], vec![1]).unwrap();
    table1.put(&vec![254; 32], vec![2]).unwrap();

    let table2 = MerklePatriciaTable::new(vec![255], &storage2);
    table2.put(&vec![254; 32], vec![2]).unwrap();
    table2.put(&vec![255; 32], vec![1]).unwrap();

    assert_eq!(table1.get(&vec![255; 32]).unwrap(), Some(vec![1]));
    assert_eq!(table1.get(&vec![254; 32]).unwrap(), Some(vec![2]));
    assert_eq!(table2.get(&vec![255; 32]).unwrap(), Some(vec![1]));
    assert_eq!(table2.get(&vec![254; 32]).unwrap(), Some(vec![2]));

    // assert_eq!(table1.find_key(&vec![]).unwrap(), Some(vec![254; 32])); //FIXME
    assert_eq!(table1.find_key(&vec![254; 32]).unwrap(),
               Some(vec![254; 32]));
    assert_eq!(table1.find_key(&vec![255; 32]).unwrap(),
               Some(vec![255; 32]));

    assert!(table1.root_hash().unwrap() != Hash::zero());
    assert_eq!(table1.root_hash().unwrap(), table2.root_hash().unwrap());
}

#[test]
fn insert_same_key() {
    let storage = MemoryDB::new().fork();
    let table = MerklePatriciaTable::new(vec![255], &storage);
    assert_eq!(table.root_hash().unwrap(), Hash::zero());
    let root_prefix = &[&[LEAF_KEY_PREFIX], vec![255; 32].as_slice(), &[0u8]].concat();
    let hash = hash(&[root_prefix, hash(&[2]).as_ref()].concat());

    table.put(&vec![255; 32], vec![1]).unwrap();
    table.put(&vec![255; 32], vec![2]).unwrap();
    assert_eq!(table.get(&vec![255; 32]).unwrap(), Some(vec![2]));
    assert_eq!(table.root_hash().unwrap(), hash);
}

#[test]
fn insert_simple() {
    let storage1 = MemoryDB::new().fork();
    let storage2 = MemoryDB::new().fork();

    let table1 = MerklePatriciaTable::new(vec![255], &storage1);
    table1.put(&vec![255; 32], vec![3]).unwrap();
    table1.put(&vec![254; 32], vec![2]).unwrap();
    table1.put(&vec![250; 32], vec![1]).unwrap();
    table1.put(&vec![254; 32], vec![5]).unwrap();

    let table2 = MerklePatriciaTable::new(vec![255], &storage2);
    table2.put(&vec![250; 32], vec![1]).unwrap();
    table2.put(&vec![254; 32], vec![2]).unwrap();
    table2.put(&vec![255; 32], vec![3]).unwrap();
    table2.put(&vec![254; 32], vec![5]).unwrap();

    assert!(table1.root_hash().unwrap() != Hash::zero());
    assert_eq!(table1.root_hash().unwrap(), table2.root_hash().unwrap());
}

#[test]
fn insert_reverse() {
    let storage1 = MemoryDB::new().fork();
    let table1 = MerklePatriciaTable::new(vec![255], &storage1);
    table1.put(&vec![42; 32], vec![1]).unwrap();
    table1.put(&vec![64; 32], vec![2]).unwrap();
    table1.put(&vec![240; 32], vec![3]).unwrap();
    table1.put(&vec![245; 32], vec![4]).unwrap();
    table1.put(&vec![250; 32], vec![5]).unwrap();
    table1.put(&vec![255; 32], vec![6]).unwrap();

    let storage2 = MemoryDB::new().fork();
    let table2 = MerklePatriciaTable::new(vec![255], &storage2);
    table2.put(&vec![255; 32], vec![6]).unwrap();
    table2.put(&vec![250; 32], vec![5]).unwrap();
    table2.put(&vec![245; 32], vec![4]).unwrap();
    table2.put(&vec![240; 32], vec![3]).unwrap();
    table2.put(&vec![64; 32], vec![2]).unwrap();
    table2.put(&vec![42; 32], vec![1]).unwrap();


    assert!(table2.root_hash().unwrap() != Hash::zero());
    assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());
}

#[test]
fn remove_trivial() {
    let storage1 = MemoryDB::new().fork();
    let table1 = MerklePatriciaTable::new(vec![255], &storage1);
    table1.put(&vec![255; 32], vec![6]).unwrap();
    table1.delete(&vec![255; 32]).unwrap();

    let storage2 = MemoryDB::new().fork();
    let table2 = MerklePatriciaTable::new(vec![255], &storage2);
    table2.put(&vec![255; 32], vec![6]).unwrap();
    table2.delete(&vec![255; 32]).unwrap();

    assert_eq!(table1.root_hash().unwrap(), Hash::zero());
    assert_eq!(table2.root_hash().unwrap(), Hash::zero());
}

#[test]
fn remove_simple() {
    let storage1 = MemoryDB::new().fork();
    let table1 = MerklePatriciaTable::new(vec![255], &storage1);
    table1.put(&vec![255; 32], vec![1]).unwrap();
    table1.put(&vec![250; 32], vec![2]).unwrap();
    table1.put(&vec![245; 32], vec![3]).unwrap();

    table1.delete(&vec![255; 32]).unwrap();
    table1.delete(&vec![245; 32]).unwrap();

    let storage2 = MemoryDB::new().fork();
    let table2 = MerklePatriciaTable::new(vec![255], &storage2);
    table2.put(&vec![250; 32], vec![2]).unwrap();
    table2.put(&vec![255; 32], vec![1]).unwrap();
    table2.put(&vec![245; 32], vec![3]).unwrap();

    table2.delete(&vec![255; 32]).unwrap();
    table2.delete(&vec![245; 32]).unwrap();

    assert_eq!(table2.get(&vec![250; 32]).unwrap(), Some(vec![2]));
    assert_eq!(table1.get(&vec![250; 32]).unwrap(), Some(vec![2]));
    assert!(table1.get(&vec![255; 32]).unwrap().is_none());
    assert!(table2.get(&vec![255; 32]).unwrap().is_none());
    assert!(table1.get(&vec![245; 32]).unwrap().is_none());
    assert!(table2.get(&vec![245; 32]).unwrap().is_none());

    assert_eq!(table1.root_hash().unwrap(), table2.root_hash().unwrap());
}

#[test]
fn remove_reverse() {
    let storage1 = MemoryDB::new().fork();
    let table1 = MerklePatriciaTable::new(vec![255], &storage1);
    table1.put(&vec![42; 32], vec![1]).unwrap();
    table1.put(&vec![64; 32], vec![2]).unwrap();
    table1.put(&vec![240; 32], vec![3]).unwrap();
    table1.put(&vec![245; 32], vec![4]).unwrap();
    table1.put(&vec![250; 32], vec![5]).unwrap();
    table1.put(&vec![255; 32], vec![6]).unwrap();

    table1.delete(&vec![255; 32]).unwrap();
    table1.delete(&vec![250; 32]).unwrap();
    table1.delete(&vec![245; 32]).unwrap();
    table1.delete(&vec![240; 32]).unwrap();
    table1.delete(&vec![64; 32]).unwrap();
    table1.delete(&vec![42; 32]).unwrap();

    let storage2 = MemoryDB::new().fork();
    let table2 = MerklePatriciaTable::new(vec![255], &storage2);
    table2.put(&vec![255; 32], vec![6]).unwrap();
    table2.put(&vec![250; 32], vec![5]).unwrap();
    table2.put(&vec![245; 32], vec![4]).unwrap();
    table2.put(&vec![240; 32], vec![3]).unwrap();
    table2.put(&vec![64; 32], vec![2]).unwrap();
    table2.put(&vec![42; 32], vec![1]).unwrap();

    table2.delete(&vec![42; 32]).unwrap();
    table2.delete(&vec![64; 32]).unwrap();
    table2.delete(&vec![240; 32]).unwrap();
    table2.delete(&vec![245; 32]).unwrap();
    table2.delete(&vec![250; 32]).unwrap();
    table2.delete(&vec![255; 32]).unwrap();

    assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());
}

#[test]
fn fuzz_insert() {
    let mut data = generate_random_data(100);
    let mut rng = rand::thread_rng();

    let storage1 = MemoryDB::new().fork();
    let table1 = MerklePatriciaTable::new(vec![255], &storage1);
    for item in &data {
        table1.put(&item.0, item.1.clone()).unwrap();
    }

    let storage2 = MemoryDB::new().fork();
    let table2 = MerklePatriciaTable::new(vec![255], &storage2);
    rng.shuffle(&mut data);
    for item in &data {
        table2.put(&item.0, item.1.clone()).unwrap();
    }

    for item in &data {
        let v1 = table1.get(&item.0).unwrap();
        let v2 = table2.get(&item.0).unwrap();
        assert_eq!(v1.as_ref(), Some(&item.1));
        assert_eq!(v2.as_ref(), Some(&item.1));
    }

    assert!(table2.root_hash().unwrap() != Hash::zero());
    assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());

    // Test same keys
    rng.shuffle(&mut data);
    for item in &data {
        table1.put(&item.0, vec![1]).unwrap();
    }
    rng.shuffle(&mut data);
    for item in &data {
        table2.put(&item.0, vec![1]).unwrap();
    }

    for item in &data {
        let v1 = table1.get(&item.0).unwrap();
        let v2 = table2.get(&item.0).unwrap();
        assert_eq!(v1.as_ref(), Some(&vec![1]));
        assert_eq!(v2.as_ref(), Some(&vec![1]));
    }
    assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());
}

#[test]
fn build_proof_in_empty_tree() {
    let storage = MemoryDB::new().fork();
    let table = MerklePatriciaTable::new(vec![255], &storage);

    table.put(&vec![230;32], vec![1]).unwrap(); //just to notify the compiler of the types used; same key is added and then removed from tree
    table.delete(&vec![230;32]).unwrap();

    let search_res = table.construct_path_to_key(&vec![244; 32]).unwrap();
    match search_res {
        RootProofNode::Empty => {}
        _ => assert!(false),
    }
    {
        let check_res =
            search_res.verify_root_proof_consistency(&vec![244;32], table.root_hash().unwrap())
                .unwrap();
        assert!(check_res.is_none());
    }
    let proof_info = ProofInfo {
        root_hash: table.root_hash().unwrap(),
        searched_key: &vec![244; 32],
        proof: &search_res,
        key_found: false,
    };
    let json_repre = serde_json::to_string(&proof_info).unwrap();
    println!("{}", json_repre);
}

#[test]
fn build_proof_in_leaf_tree() {
    let storage = MemoryDB::new().fork();
    let table = MerklePatriciaTable::new(vec![255], &storage);
    let root_key = vec![230;32];
    let root_val = vec![2];
    let searched_key = vec![244; 32];

    table.put(&root_key, root_val.clone()).unwrap();
    let table_root = table.root_hash().unwrap();
    let proof_path = table.construct_path_to_key(&searched_key).unwrap();

    {
        let check_res = proof_path.verify_root_proof_consistency(&searched_key, table_root)
            .unwrap();
        assert!(check_res.is_none());
    }
    {
        let proof_info = ProofInfo {
            root_hash: table_root,
            searched_key: &searched_key,
            proof: &proof_path,
            key_found: false,
        };
        let json_repre = serde_json::to_string(&proof_info).unwrap();
        println!("{}", json_repre);
    }

    match proof_path {
        RootProofNode::LeafRootExclusive(key, hash_val) => {
            assert_eq!(key.db_key_data, BitSlice::from_bytes(&root_key).to_db_key());
            assert_eq!(hash_val, hash(&root_val));
        }
        _ => assert!(false),
    }

    let proof_path = table.construct_path_to_key(&root_key).unwrap();
    assert_eq!(table_root, proof_path.compute_proof_root());
    {
        let check_res = proof_path.verify_root_proof_consistency(&root_key, table_root)
            .unwrap();
        assert_eq!(*check_res.unwrap(), root_val);
    }
    {
        let proof_info = ProofInfo {
            root_hash: table_root,
            searched_key: &root_key,
            proof: &proof_path,
            key_found: true,
        };
        let json_repre = serde_json::to_string(&proof_info).unwrap();
        println!("{}", json_repre);
    }
    match proof_path {
        RootProofNode::LeafRootInclusive(key, val) => {
            assert_eq!(key.db_key_data, BitSlice::from_bytes(&root_key).to_db_key());
            assert_eq!(val, root_val);
        }
        _ => assert!(false),
    }
}

#[test]
fn fuzz_insert_build_proofs_in_table_filled_with_hashes() {
    let data: Vec<(Hash, Hash)> = generate_fully_random_data_keys(100).into_iter().map(|el| {
        let (key, val) = el;
        (hash(&key), hash(&val))
    }).collect::<Vec<_>>();

    let storage = MemoryDB::new().fork();
    let table = MerklePatriciaTable::new(vec![255], &storage);
    for item in &data {
        table.put(&item.0, item.1.clone()).unwrap();
    }

    let table_root_hash = table.root_hash().unwrap();
    let item = data[0];
    let proof_path_to_key = table.construct_path_to_key(&item.0.as_ref()).unwrap();
    assert_eq!(proof_path_to_key.compute_proof_root(), table_root_hash);
    let check_res =
    proof_path_to_key.verify_root_proof_consistency(&item.0, table_root_hash);
    let proved_value: Option<&Hash> = check_res.unwrap();
    assert_eq!(*proved_value.unwrap(), item.1);

    let proof_info = ProofInfo {
        root_hash: table_root_hash,
        searched_key: item.0,
        proof: &proof_path_to_key,
        key_found: true,
    };

    let json_repre = serde_json::to_string(&proof_info).unwrap();
    println!("{}", json_repre);
}

#[test]
fn fuzz_insert_build_proofs() {
    let data = generate_fully_random_data_keys(100);

    let storage = MemoryDB::new().fork();
    let table = MerklePatriciaTable::new(vec![255], &storage);
    for item in &data {
        table.put(&item.0, item.1.clone()).unwrap();
    }

    let table_root_hash = table.root_hash().unwrap();

    for item in &data {
        let proof_path_to_key = table.construct_path_to_key(&item.0).unwrap();
        assert_eq!(proof_path_to_key.compute_proof_root(), table_root_hash);
        let check_res =
            proof_path_to_key.verify_root_proof_consistency(&item.0, table_root_hash);
        let proved_value: Option<&Vec<u8>> = check_res.unwrap();
        assert_eq!(*proved_value.unwrap(), item.1);

        let proof_info = ProofInfo {
            root_hash: table_root_hash,
            searched_key: &item.0,
            proof: &proof_path_to_key,
            key_found: true,
        };

        let json_repre = serde_json::to_string(&proof_info).unwrap();
        println!("{}", json_repre);
    }
}

#[test]
fn fuzz_delete_build_proofs() {
    let data = generate_fully_random_data_keys(100);
    let mut rng = rand::thread_rng();

    let storage1 = MemoryDB::new().fork();
    let table1 = MerklePatriciaTable::new(vec![255], &storage1);
    for item in &data {
        table1.put(&item.0, item.1.clone()).unwrap();
    }

    let mut keys_to_remove = data.iter()
        .take(50)
        .map(|item| item.0.clone())
        .collect::<Vec<_>>();

    rng.shuffle(&mut keys_to_remove);
    for key in &keys_to_remove {
        table1.delete(key).unwrap();
    }
    let table_root_hash = table1.root_hash().unwrap();
    for key in &keys_to_remove {
        let proof_path_to_key = table1.construct_path_to_key(key).unwrap();
        assert_eq!(proof_path_to_key.compute_proof_root(), table_root_hash);
        let check_res = proof_path_to_key.verify_root_proof_consistency(key, table_root_hash);
        assert!(check_res.is_ok());
        let proved_value: Option<&Vec<u8>> = check_res.unwrap();
        assert!(proved_value.is_none());

        let proof_info = ProofInfo {
            root_hash: table_root_hash,
            searched_key: key,
            proof: &proof_path_to_key,
            key_found: false,
        };
        let json_repre = serde_json::to_string(&proof_info).unwrap();
        println!("{}", json_repre);
    }
}

#[test]
fn fuzz_delete() {
    let mut data = generate_random_data(100);
    let mut rng = rand::thread_rng();

    let storage1 = MemoryDB::new().fork();
    let table1 = MerklePatriciaTable::new(vec![255], &storage1);
    for item in &data {
        table1.put(&item.0, item.1.clone()).unwrap();
    }

    let storage2 = MemoryDB::new().fork();
    let table2 = MerklePatriciaTable::new(vec![255], &storage2);
    rng.shuffle(&mut data);
    for item in &data {
        table2.put(&item.0, item.1.clone()).unwrap();
    }

    let saved_hash = table1.root_hash().unwrap();

    let mut keys_to_remove = data.iter()
        .take(50)
        .map(|item| item.0.clone())
        .collect::<Vec<_>>();

    rng.shuffle(&mut keys_to_remove);
    for key in &keys_to_remove {
        table1.delete(key).unwrap();
    }
    rng.shuffle(&mut keys_to_remove);
    for key in &keys_to_remove {
        table2.delete(key).unwrap();
    }

    for key in &keys_to_remove {
        assert!(table1.get(key).unwrap().is_none());
        assert!(table2.get(key).unwrap().is_none());
    }

    assert!(table2.root_hash().unwrap() != Hash::zero());
    assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());

    for item in &data {
        table1.put(&item.0, item.1.clone()).unwrap();
    }
    rng.shuffle(&mut data);
    for item in &data {
        table2.put(&item.0, item.1.clone()).unwrap();
    }

    for item in &data {
        let v1 = table1.get(&item.0).unwrap();
        let v2 = table2.get(&item.0).unwrap();
        assert_eq!(v1.as_ref(), Some(&item.1));
        assert_eq!(v2.as_ref(), Some(&item.1));
    }
    assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());
    assert_eq!(table2.root_hash().unwrap(), saved_hash);
}

#[test]
fn fuzz_insert_after_delete() {
    let storage1 = MemoryDB::new().fork();
    let table1 = MerklePatriciaTable::new(vec![255], &storage1);

    let data = generate_random_data(100);

    for item in &data[0..50] {
        table1.put(&item.0, item.1.clone()).unwrap();
    }
    let saved_hash = table1.root_hash().unwrap();
    for item in &data[50..] {
        table1.put(&item.0, item.1.clone()).unwrap();
    }
    for item in &data[50..] {
        table1.delete(&item.0).unwrap();
    }

    for item in &data[0..50] {
        let v1 = table1.get(&item.0).unwrap();
        assert_eq!(v1.as_ref(), Some(&item.1));
    }
    for item in &data[50..] {
        let v1 = table1.get(&item.0).unwrap();
        assert_eq!(v1.as_ref(), None);
    }
    assert_eq!(table1.root_hash().unwrap(), saved_hash);
}
