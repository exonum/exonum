extern crate rand;
use rand::{thread_rng, Rng};
use std::collections::HashSet;

use ::crypto::{Hash, hash};
use storage::{Database, MemoryDB, List, MerkleTable};
use serde_json;
use super::{split_range, index_of_first_element_in_subtree};
use super::proofnode::{proof_indices_values, Proofnode};
use serde::Serialize;
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
    let storage = MemoryDB::new().fork();
    let table = MerkleTable::new(vec![255], &storage);

    assert!(table.is_empty().unwrap());
    assert_eq!(table.len().unwrap(), 0);
    table.append(vec![1]).unwrap();
    assert!(!table.is_empty().unwrap());
    assert_eq!(table.len().unwrap(), 1);

    table.append(vec![2]).unwrap();
    assert_eq!(table.len().unwrap(), 2);

    table.append(vec![3]).unwrap();
    assert_eq!(table.len().unwrap(), 3);

    assert_eq!(table.get(0u64).unwrap(), Some(vec![1]));
    assert_eq!(table.get(1).unwrap(), Some(vec![2]));
    assert_eq!(table.get(2).unwrap(), Some(vec![3]));
}

#[test]
fn test_height() {
    let storage = MemoryDB::new().fork();
    let table = MerkleTable::new(vec![255], &storage);

    table.append(vec![1]).unwrap();
    assert_eq!(table.height().unwrap(), 1);

    table.append(vec![2]).unwrap();
    assert_eq!(table.height().unwrap(), 2);

    table.append(vec![3]).unwrap();
    assert_eq!(table.height().unwrap(), 3);

    table.append(vec![4]).unwrap();
    assert_eq!(table.height().unwrap(), 3);

    assert_eq!(table.len().unwrap(), 4);
    assert_eq!(table.get(0u64).unwrap(), Some(vec![1]));
    assert_eq!(table.get(1).unwrap(), Some(vec![2]));
    assert_eq!(table.get(2).unwrap(), Some(vec![3]));
    assert_eq!(table.get(3).unwrap(), Some(vec![4]));

    table.set(1, vec![10]).unwrap();
    assert_eq!(table.get(1).unwrap(), Some(vec![10]));
}

#[test]
fn generate_proof_in_table_containing_hashes() {
    let storage = MemoryDB::new().fork();
    let table = MerkleTable::new(vec![255], &storage);
    let num_vals = 10u32;
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
    println!("{}", serde_json::to_string(&proof_info).unwrap());
}

#[test]
fn randomly_generate_proofs() {
    let storage = MemoryDB::new().fork();
    let table = MerkleTable::new(vec![255], &storage);
    let num_vals = 100u64;
    let values = generate_fully_random_data_keys(num_vals as usize);
    let mut rng = thread_rng();
    for value in &values {
        table.append(value.clone()).unwrap();
    }
    table.get(0u64).unwrap();
    let table_root_hash = table.root_hash().unwrap();
    let table_len = table.len().unwrap() as usize;

    for _ in 0..50 {
        let start_range = rng.gen_range(0u64, num_vals);
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
        println!("{}", serde_json::to_string(&proof_info).unwrap());

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
    let storage = MemoryDB::new().fork();
    let table = MerkleTable::new(vec![255], &storage);
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
        println!("{}", serde_json::to_string(&proof_info).unwrap());

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
        println!("{}", serde_json::to_string(&proof_info).unwrap());
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
        println!("{}", serde_json::to_string(&proof_info).unwrap());
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
    assert_eq!(table.get(0u64).unwrap(), Some(vec![1, 2]));
}

#[test]
#[should_panic]
fn test_proof_illegal_lower_bound() {
    let storage = MemoryDB::new().fork();
    let table = MerkleTable::new(vec![255], &storage);
    table.construct_path_for_range(0u64, 1u64).unwrap();
    table.append(vec![1]).unwrap();
}

#[test]
#[should_panic]
fn test_proof_illegal_bound_empty() {
    let storage = MemoryDB::new().fork();
    let table = MerkleTable::new(vec![255], &storage);
    for i in 0u8..8 {
        table.append(vec![i]).unwrap();
    }
    table.construct_path_for_range(8u64, 9).unwrap();
}

#[test]
#[should_panic]
fn test_proof_illegal_range() {
    let storage = MemoryDB::new().fork();
    let table = MerkleTable::new(vec![255], &storage);
    for i in 0u8..4 {
        table.append(vec![i]).unwrap();
    }
    table.construct_path_for_range(2u64, 2).unwrap();
}

#[test]
fn test_proof_structure() {
    let storage = MemoryDB::new().fork();
    let table = MerkleTable::new(vec![255], &storage);
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

    for i in 0u8...4 {
        table.append(vec![i, i + 1, i + 2]).unwrap();
    }

    assert_eq!(table.root_hash().unwrap(), h12345);
    let range_proof = table.construct_path_for_range(4u64, 5).unwrap();
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
}

#[test]
fn test_hash_in_values() {
    let storage = MemoryDB::new().fork();
    let table = MerkleTable::new(vec![255], &storage);

    let h = hash(&[1, 2, 3, 4]);
    table.append(h).unwrap();
    assert_eq!(table.get(0u64).unwrap(), Some(h));
}

#[test]
fn test_hash_set_value_simple() {
    let h1 = hash(&[1]);
    let h2 = hash(&[2]);

    let s = MemoryDB::new().fork();
    let t = MerkleTable::new(vec![255], &s);
    assert_eq!(t.get(0u64).unwrap(), None);
    t.append(vec![1]).unwrap();
    assert_eq!(t.root_hash().unwrap(), h1);

    t.set(0, vec![2]).unwrap();
    assert_eq!(t.root_hash().unwrap(), h2);
}

#[test]
fn test_hash_set_value() {
    let s1 = MemoryDB::new().fork();
    let t1 = MerkleTable::new(vec![255], &s1);
    assert_eq!(t1.get(0u64).unwrap(), None);
    t1.append(vec![1]).unwrap();
    t1.append(vec![2]).unwrap();
    t1.append(vec![3]).unwrap();
    t1.append(vec![4]).unwrap();

    t1.set(0, vec![4]).unwrap();
    t1.set(1, vec![7]).unwrap();
    t1.set(2, vec![5]).unwrap();
    t1.set(3, vec![1]).unwrap();

    let s2 = MemoryDB::new().fork();
    let t2 = MerkleTable::new(vec![255], &s2);
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
