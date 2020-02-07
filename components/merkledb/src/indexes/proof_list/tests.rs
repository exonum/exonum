// Copyright 2020 The Exonum Team
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

// spell-checker:ignore upup

#![allow(clippy::too_many_lines)]

use exonum_crypto::Hash;
use rand::{thread_rng, Rng};
use serde_json::{self, json};

use std::cmp;

use super::{key::ProofListKey, tree_height_by_length, ListProof, ListProofError, ProofListIndex};
use crate::{access::CopyAccessExt, BinaryValue, Database, HashTag, ObjectHash, TemporaryDB};

const IDX_NAME: &str = "idx_name";

fn random_values<R: Rng>(rng: &mut R, len: usize) -> Vec<Vec<u8>> {
    use std::collections::HashSet;

    let mut exists = HashSet::new();
    let generator = |_| {
        let mut new_val: Vec<u8> = vec![0; 10];
        rng.fill_bytes(&mut new_val);

        while exists.contains(&new_val) {
            rng.fill_bytes(&mut new_val);
        }
        exists.insert(new_val.clone());
        new_val
    };

    (0..len).map(generator).collect::<Vec<_>>()
}

#[test]
fn list_methods() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);

    assert!(index.is_empty());
    assert_eq!(index.len(), 0);
    index.push(vec![1]);
    assert!(!index.is_empty());
    assert_eq!(index.len(), 1);

    index.push(vec![2]);
    assert_eq!(index.len(), 2);

    index.extend(vec![vec![3]]);
    assert_eq!(index.len(), 3);

    assert_eq!(index.get(0), Some(vec![1]));
    assert_eq!(index.get(1), Some(vec![2]));
    assert_eq!(index.get(2), Some(vec![3]));
}

#[test]
fn get_and_iter_from_with_overly_large_index() {
    const LARGE_INDEXES: &[u64] = &[
        1_u64 << 56,
        (1_u64 << 56) + 10,
        1_u64 << 57,
        1_u64 << 60,
        u64::max_value() - 10,
        u64::max_value(),
    ];

    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut list = fork.get_proof_list::<_, u64>(IDX_NAME);
    list.extend(0..10);

    for &index in LARGE_INDEXES {
        assert_eq!(list.get(index), None);
        assert_eq!(list.iter_from(index).count(), 0);
    }
}

#[test]
fn extend_is_equivalent_to_sequential_pushes() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);

    for _ in 0..10 {
        index.clear();
        let values: [u8; 32] = thread_rng().gen();
        for &value in &values {
            index.push(value);
        }
        let hash_after_pushes = index.object_hash();

        index.clear();
        index.extend(values.iter().cloned());
        assert_eq!(index.object_hash(), hash_after_pushes);
    }

    // Try extending list in several calls.
    for _ in 0..10 {
        index.clear();
        let values: [u8; 32] = thread_rng().gen();
        for &value in &values {
            index.push(value);
        }
        let hash_after_pushes = index.object_hash();

        index.clear();
        let mut iter = values.iter().cloned();
        index.extend(iter.by_ref().take(5));
        index.extend(iter.by_ref().take(8));
        index.extend(iter.by_ref().take(3));
        index.extend(iter);
        assert_eq!(index.object_hash(), hash_after_pushes);
    }

    // Try mixing extensions and pushes
    for _ in 0..10 {
        index.clear();
        let values: [u8; 32] = thread_rng().gen();
        for &value in &values {
            index.push(value);
        }
        let hash_after_pushes = index.object_hash();

        index.clear();
        let mut iter = values.iter().cloned();
        index.extend(iter.by_ref().take(5));
        for value in iter.by_ref().take(3) {
            index.push(value);
        }
        index.extend(iter.by_ref().take(7));
        index.push(iter.by_ref().next().unwrap());
        index.extend(iter);
        assert_eq!(index.object_hash(), hash_after_pushes);
    }
}

#[test]
fn tree_height() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);

    assert_eq!(index.height(), 0);
    index.push(vec![1]);
    assert_eq!(index.height(), 1);
    index.push(vec![2]);
    assert_eq!(index.height(), 2);
    index.push(vec![3]);
    assert_eq!(index.height(), 3);
    index.push(vec![4]);
    assert_eq!(index.height(), 3);
}

#[test]
fn iter() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut list_index = fork.get_proof_list(IDX_NAME);

    list_index.extend(vec![1_u8, 2, 3]);

    assert_eq!(list_index.iter().collect::<Vec<u8>>(), vec![1, 2, 3]);
    assert_eq!(list_index.iter_from(0).collect::<Vec<u8>>(), vec![1, 2, 3]);
    assert_eq!(list_index.iter_from(1).collect::<Vec<u8>>(), vec![2, 3]);
    assert_eq!(
        list_index.iter_from(3).collect::<Vec<u8>>(),
        Vec::<u8>::new()
    );
}

#[test]
fn iter_from_with_large_index() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut list_index = fork.get_proof_list(IDX_NAME);

    list_index.extend(1_u8..10);
    // This key should be present in the Merkle tree, but should not be returned
    // from the iterator.
    let key = ProofListKey::new(1, 0).as_db_key();
    assert!(key >= 1 << 56);
    let items = list_index.iter_from(key);
    assert_eq!(items.count(), 0);
}

#[test]
fn simple_proof() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);

    let h0 = HashTag::hash_leaf(&2_u64.to_bytes());
    let h1 = HashTag::hash_leaf(&4_u64.to_bytes());
    let h2 = HashTag::hash_leaf(&6_u64.to_bytes());
    let h01 = HashTag::hash_node(&h0, &h1);
    let h22 = HashTag::hash_single_node(&h2);
    let h012 = HashTag::hash_node(&h01, &h22);

    assert_eq!(index.object_hash(), HashTag::empty_list_hash());

    index.push(2_u64);

    assert_eq!(index.object_hash(), HashTag::hash_list_node(1, h0));
    let proof = index.get_proof(0);
    assert_eq!(proof, ListProof::new(vec![(0, 2_u64)], index.len()));
    let proof = proof.check().unwrap();
    assert_eq!(proof.index_hash(), index.object_hash());
    assert_eq!(*proof.entries(), [(0, 2)]);

    index.push(4_u64);
    assert_eq!(index.object_hash(), HashTag::hash_list_node(2, h01));

    let proof = index.get_proof(0);
    assert_eq!(proof, {
        let mut proof = ListProof::new(vec![(0, 2_u64)], index.len());
        proof.push_hash(1, 1, h1);
        proof
    });
    let proof = proof.check().unwrap();
    assert_eq!(proof.index_hash(), index.object_hash());
    assert_eq!(*proof.entries(), [(0, 2)]);

    let proof = index.get_proof(1);
    assert_eq!(proof, {
        let mut proof = ListProof::new(vec![(1, 4_u64)], index.len());
        proof.push_hash(1, 0, h0);
        proof
    });

    let proof = proof.check().unwrap();
    assert_eq!(proof.index_hash(), index.object_hash());
    assert_eq!(*proof.entries(), [(1, 4)]);

    let proof = index.get_range_proof(0..2);
    assert_eq!(
        proof,
        ListProof::new(vec![(0, 2_u64), (1, 4_u64)], index.len())
    );
    assert_eq!(
        *proof
            .check_against_hash(index.object_hash())
            .unwrap()
            .entries(),
        [(0, 2), (1, 4)]
    );

    index.push(6_u64);
    assert_eq!(index.object_hash(), HashTag::hash_list_node(3, h012));

    let proof = index.get_proof(0);
    assert_eq!(proof, {
        let mut proof = ListProof::new(vec![(0, 2_u64)], index.len());
        proof.push_hash(1, 1, h1);
        proof.push_hash(2, 1, h22);
        proof
    });
    assert_eq!(
        *proof
            .check_against_hash(index.object_hash())
            .unwrap()
            .entries(),
        [(0, 2)]
    );

    let proof = index.get_range_proof(1..3);
    assert_eq!(proof, {
        let mut proof = ListProof::new(vec![(1, 4_u64), (2, 6_u64)], index.len());
        proof.push_hash(1, 0, h0);
        proof
    });
    assert_eq!(
        *proof
            .check_against_hash(index.object_hash())
            .unwrap()
            .entries(),
        [(1, 4_u64), (2, 6_u64)]
    );

    let proof = index.get_range_proof(0..2);
    assert_eq!(proof, {
        let mut proof = ListProof::new(vec![(0, 2_u64), (1, 4_u64)], index.len());
        proof.push_hash(2, 1, h22);
        proof
    });
    assert_eq!(
        *proof
            .check_against_hash(index.object_hash())
            .unwrap()
            .entries(),
        [(0, 2_u64), (1, 4_u64)]
    );
}

#[test]
fn proofs_in_empty_list() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let index: ProofListIndex<_, u32> = fork.get_proof_list(IDX_NAME);
    let proof = index.get_range_proof(..);
    proof.check_against_hash(index.object_hash()).unwrap();
    assert!(proof.check_against_hash(index.object_hash()).is_ok());
    let proof = index.get_range_proof(..0);
    assert!(proof.check_against_hash(index.object_hash()).is_ok());
}

#[test]
fn empty_proof_ranges() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);
    index.extend(vec![1_u32, 2, 3]);
    let proof = index.get_range_proof(1..1);
    assert!(proof
        .check_against_hash(index.object_hash())
        .unwrap()
        .entries()
        .is_empty());
    let proof = index.get_range_proof(1..=0);
    assert!(proof
        .check_against_hash(index.object_hash())
        .unwrap()
        .entries()
        .is_empty());
}

#[test]
fn random_proofs() {
    const LIST_SIZE: usize = 1 << 10;
    const MAX_RANGE_SIZE: u64 = 128;

    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);

    let mut rng = thread_rng();
    let values = random_values(&mut rng, LIST_SIZE);

    for value in &values {
        index.push(value.clone());
    }

    let index_hash = index.object_hash();
    for _ in 0..10 {
        let start = rng.gen_range(0, LIST_SIZE as u64);
        let end = rng.gen_range(start + 1, LIST_SIZE as u64 + 1);
        let end = cmp::min(end, start + MAX_RANGE_SIZE);

        let range_proof = index.get_range_proof(start..end);
        let checked_proof = range_proof.check_against_hash(index_hash).unwrap();
        let expected_entries = (start..end).zip(&values[start as usize..end as usize]);
        assert!(checked_proof
            .entries()
            .iter()
            .map(|(i, value)| (*i, value))
            .eq(expected_entries));
    }
}

#[test]
fn proofs_with_overly_large_indexes() {
    const LARGE_INDEXES: &[u64] = &[
        1_u64 << 56,
        (1_u64 << 56) + 10,
        1_u64 << 57,
        1_u64 << 60,
        u64::max_value() - 10,
        u64::max_value(),
    ];

    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);
    index.extend(0_u64..1_000);

    for &i in LARGE_INDEXES {
        let proof = index.get_proof(i);
        let checked_proof = proof.check_against_hash(index.object_hash()).unwrap();
        assert!(checked_proof.entries().is_empty());
    }
}

#[test]
fn proofs_with_overly_large_index_ranges() {
    const LARGE_INDEXES: &[u64] = &[
        1_u64 << 56,
        (1_u64 << 56) + 10,
        1_u64 << 57,
        1_u64 << 60,
        u64::max_value() - 10,
    ];

    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);
    index.extend(0_u64..10);

    for &i in LARGE_INDEXES {
        let proof = index.get_range_proof(i..);
        let checked_proof = proof.check_against_hash(index.object_hash()).unwrap();
        assert!(checked_proof.entries().is_empty());

        let proof = index.get_range_proof(i..(i + 5));
        let checked_proof = proof.check_against_hash(index.object_hash()).unwrap();
        assert!(checked_proof.entries().is_empty());

        let proof = index.get_range_proof((i - 5)..=i);
        let checked_proof = proof.check_against_hash(index.object_hash()).unwrap();
        assert!(checked_proof.entries().is_empty());
    }

    let proof = index.get_range_proof(..=u64::max_value());
    let checked_proof = proof.check_against_hash(index.object_hash()).unwrap();
    assert_eq!(checked_proof.entries().len(), 10);

    let proof = index.get_range_proof(7..u64::max_value());
    let checked_proof = proof.check_against_hash(index.object_hash()).unwrap();
    assert_eq!(checked_proof.entries().len(), 3);
}

#[test]
fn index_and_proof_roots() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);
    assert_eq!(index.object_hash(), HashTag::empty_list_hash());

    let h1 = HashTag::hash_leaf(&[1, 2]);
    let h2 = HashTag::hash_leaf(&[2, 3]);
    let h3 = HashTag::hash_leaf(&[3, 4]);
    let h4 = HashTag::hash_leaf(&[4, 5]);
    let h5 = HashTag::hash_leaf(&[5, 6]);
    let h6 = HashTag::hash_leaf(&[6, 7]);
    let h7 = HashTag::hash_leaf(&[7, 8]);
    let h8 = HashTag::hash_leaf(&[8, 9]);

    let h12 = HashTag::hash_node(&h1, &h2);
    let h3up = HashTag::hash_single_node(&h3);
    let h123 = HashTag::hash_node(&h12, &h3up);

    let h34 = HashTag::hash_node(&h3, &h4);
    let h1234 = HashTag::hash_node(&h12, &h34);

    let h5up = HashTag::hash_single_node(&h5);
    let h5upup = HashTag::hash_single_node(&h5up);
    let h12345 = HashTag::hash_node(&h1234, &h5upup);

    let h56 = HashTag::hash_node(&h5, &h6);
    let h56up = HashTag::hash_single_node(&h56);
    let h123456 = HashTag::hash_node(&h1234, &h56up);

    let h7up = HashTag::hash_single_node(&h7);
    let h567 = HashTag::hash_node(&h56, &h7up);
    let h1234567 = HashTag::hash_node(&h1234, &h567);

    let h78 = HashTag::hash_node(&h7, &h8);
    let h5678 = HashTag::hash_node(&h56, &h78);
    let h12345678 = HashTag::hash_node(&h1234, &h5678);

    let expected_hash_comb = vec![
        (vec![1, 2], HashTag::hash_list_node(1, h1), 0),
        (vec![2, 3], HashTag::hash_list_node(2, h12), 1),
        (vec![3, 4], HashTag::hash_list_node(3, h123), 2),
        (vec![4, 5], HashTag::hash_list_node(4, h1234), 3),
        (vec![5, 6], HashTag::hash_list_node(5, h12345), 4),
        (vec![6, 7], HashTag::hash_list_node(6, h123456), 5),
        (vec![7, 8], HashTag::hash_list_node(7, h1234567), 6),
        (vec![8, 9], HashTag::hash_list_node(8, h12345678), 7),
    ];

    for (inserted, exp_root, proof_ind) in expected_hash_comb {
        index.push(inserted);
        assert_eq!(index.object_hash(), exp_root);

        let range_proof = index.get_range_proof(proof_ind..=proof_ind);
        assert_eq!(
            range_proof
                .check_against_hash(index.object_hash())
                .unwrap()
                .entries()
                .len(),
            1
        );
        let js = serde_json::to_string(&range_proof).unwrap();
        let deserialized_proof: ListProof<Vec<u8>> = serde_json::from_str(&js).unwrap();
        assert_eq!(deserialized_proof, range_proof);

        let range_proof = index.get_range_proof(..=proof_ind);
        assert_eq!(
            range_proof
                .check_against_hash(index.object_hash())
                .unwrap()
                .entries()
                .len(),
            (proof_ind + 1) as usize
        );
        let js = serde_json::to_string(&range_proof).unwrap();
        let deserialized_proof: ListProof<Vec<u8>> = serde_json::from_str(&js).unwrap();
        assert_eq!(deserialized_proof, range_proof);

        let range_proof = index.get_range_proof(0..1);
        assert_eq!(
            range_proof
                .check_against_hash(index.object_hash())
                .unwrap()
                .entries()
                .len(),
            1
        );
        let js = serde_json::to_string(&range_proof).unwrap();
        let deserialized_proof: ListProof<Vec<u8>> = serde_json::from_str(&js).unwrap();
        assert_eq!(deserialized_proof, range_proof);
    }

    let range_proof = index.get_range_proof(..);
    let entries = range_proof
        .check_against_hash(index.object_hash())
        .unwrap()
        .entries();
    let expected_entries = (0..8).zip(vec![
        vec![1, 2],
        vec![2, 3],
        vec![3, 4],
        vec![4, 5],
        vec![5, 6],
        vec![6, 7],
        vec![7, 8],
        vec![8, 9],
    ]);
    assert!(entries
        .iter()
        .zip(expected_entries)
        .all(|(actual, expected)| *actual == expected));

    let mut range_proof = index.get_range_proof(3..5);
    assert_eq!(
        range_proof
            .check_against_hash(index.object_hash())
            .unwrap()
            .entries()
            .len(),
        2
    );
    range_proof = index.get_range_proof(2..6);
    assert_eq!(
        range_proof
            .check_against_hash(index.object_hash())
            .unwrap()
            .entries()
            .len(),
        4
    );
    assert_eq!(index.get(0), Some(vec![1, 2]));
}

#[test]
#[should_panic(expected = "the range start is 2, but the range end is 1")]
fn proof_illegal_range() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);
    for i in 0_u8..4 {
        index.push(vec![i]);
    }
    index.get_range_proof(2..1);
}

#[test]
#[should_panic(expected = "the range start is 2, but the range end is 1")]
fn proof_illegal_inclusive_range() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);
    for i in 0_u8..4 {
        index.push(vec![i]);
    }
    index.get_range_proof(2..=0); // `2..=1` is a legal empty range; cf. `Vec` slicing
}

#[test]
fn ranges_work_similar_to_vec_slicing() {
    use std::{
        fmt,
        ops::RangeBounds,
        panic::{self, UnwindSafe},
        slice::SliceIndex,
    };

    #[allow(clippy::needless_pass_by_value)] // references to ranges look awkward
    fn check_bounds(
        v: &[u32],
        slice_bounds: impl SliceIndex<[u32], Output = [u32]> + Clone + UnwindSafe + fmt::Debug,
        index_bounds: impl RangeBounds<u64> + UnwindSafe,
    ) {
        let panic_hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));

        let slice_bounds_ = slice_bounds.clone();
        let slicing_res = panic::catch_unwind(|| v[slice_bounds_].to_vec());
        let proof_res = panic::catch_unwind(|| {
            let fork = TemporaryDB::new().fork();
            let mut index = fork.get_proof_list(IDX_NAME);
            index.extend(v.to_vec());
            (index.object_hash(), index.get_range_proof(index_bounds))
        });

        panic::set_hook(panic_hook);

        if let Ok((index_hash, proof)) = proof_res {
            let checked_proof = proof.check_against_hash(index_hash).unwrap();

            match slicing_res {
                Ok(slice) => {
                    assert_eq!(
                        checked_proof
                            .entries()
                            .iter()
                            .map(|(_, value)| *value)
                            .collect::<Vec<_>>(),
                        slice
                    );
                }
                Err(e) => {
                    // Slicing is more strict if the range does not completely fit
                    // in the slice indexes.
                    assert!(
                        e.downcast_ref::<String>().unwrap().contains("out of range"),
                        "{:?}",
                        slice_bounds
                    );
                }
            }
        } else {
            assert!(slicing_res.is_err(), "{:?}", slice_bounds);
        }
    }

    let v = vec![1, 2, 3, 4, 5];
    check_bounds(&v, .., ..);
    for start in 0_usize..10 {
        for end in 0_usize..10 {
            check_bounds(&v, start..end, (start as u64)..(end as u64));
            check_bounds(&v, start..=end, (start as u64)..=(end as u64));
            if start == 0 {
                check_bounds(&v, ..end, ..(end as u64));
                check_bounds(&v, ..=end, ..=(end as u64));
            }
            if end == 0 && start <= v.len() {
                check_bounds(&v, start.., (start as u64)..);
            }
        }
    }
}

#[test]
fn proof_with_range_start_exceeding_list_size() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);
    for i in 0_u8..4 {
        index.push(vec![i]);
    }

    let proof = index.get_range_proof(8..10_000_000);
    assert!(proof
        .check_against_hash(index.object_hash())
        .unwrap()
        .entries()
        .is_empty());

    let proof = index.get_range_proof(8..);
    assert!(proof
        .check_against_hash(index.object_hash())
        .unwrap()
        .entries()
        .is_empty());
}

#[test]
fn proof_with_range_end_exceeding_list_size() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = fork.get_proof_list(IDX_NAME);
    for i in 0_u8..4 {
        index.push(vec![i]);
    }

    let proof = index.get_range_proof(2..10);
    assert_eq!(
        proof
            .check_against_hash(index.object_hash())
            .unwrap()
            .entries()
            .len(),
        2 // elements 2 and 3
    );
}

#[test]
fn setting_elements_leads_to_correct_list_hash() {
    let db = TemporaryDB::new();
    let hash1 = {
        let fork = db.fork();
        let mut list = fork.get_proof_list(IDX_NAME);
        list.push(vec![1]);
        list.push(vec![2]);
        list.push(vec![3]);
        list.push(vec![4]);

        list.set(0, vec![4]);
        list.set(1, vec![7]);
        list.set(2, vec![5]);
        list.set(3, vec![1]);
        list.object_hash()
    };
    let hash2 = {
        let fork = db.fork();
        let mut list = fork.get_proof_list(IDX_NAME);
        list.push(vec![4]);
        list.push(vec![7]);
        list.push(vec![5]);
        list.push(vec![1]);
        list.object_hash()
    };
    assert_eq!(hash1, hash2);
}

#[test]
fn setting_elements_leads_to_correct_list_hash_randomized() {
    const LIST_LEN: usize = 32;

    let mut rng = thread_rng();
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut list = fork.get_proof_list(IDX_NAME);

    for _ in 0..10 {
        // Prepare two copies of values with sufficient intersection.
        let values: [u16; LIST_LEN] = rng.gen();
        let mut new_values: [u16; LIST_LEN] = rng.gen();
        for i in 0..LIST_LEN {
            if rng.gen::<bool>() {
                new_values[i] = values[i];
            }
        }
        let proof_ranges: Vec<_> = (0..50)
            .map(|_| {
                let start = rng.gen_range(0, LIST_LEN as u64);
                let end = rng.gen_range(start, LIST_LEN as u64) + 1;
                start..end
            })
            .collect();

        list.clear();
        list.extend(new_values.iter().cloned());
        let list_hash = list.object_hash();
        let expected_proofs: Vec<_> = proof_ranges
            .iter()
            .map(|range| list.get_range_proof(range.clone()))
            .collect();

        list.clear();
        list.extend(values.iter().cloned());
        for i in 0..values.len() {
            if values[i] != new_values[i] {
                list.set(i as u64, new_values[i]);
            }
        }
        assert_eq!(list.object_hash(), list_hash);
        for (i, range) in proof_ranges.into_iter().enumerate() {
            let proof = list.get_range_proof(range.clone());
            assert_eq!(
                proof, expected_proofs[i],
                "Unexpected proof for range {:?}",
                range
            );
        }
    }
}

#[test]
fn truncating_list() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut list = fork.get_proof_list(IDX_NAME);
    list.extend(0_u32..30);
    list.truncate(5);
    assert_eq!(list.len(), 5);
    assert_eq!(list.get(3), Some(3));
    assert_eq!(list.get(7), None);
    assert!(list.iter().eq(0_u32..5));
    assert!(list.iter_from(3).eq(3_u32..5));

    // Check that the branches are removed.
    let level_lengths = vec![5, 5, 3, 2, 1];
    for height in 1..tree_height_by_length(30) {
        let level_len = level_lengths
            .get(height as usize)
            .copied()
            .unwrap_or_default();
        if level_len > 0 {
            assert!(list
                .get_branch(ProofListKey::new(height, level_len - 1))
                .is_some());
        }
        for index in level_len..(level_len + 30) {
            let key = ProofListKey::new(height, index);
            assert!(
                list.get_branch(key).is_none(),
                "Branch wasn't removed: {:?}",
                key
            );
        }
    }
}

#[test]
fn truncating_list_leads_to_expected_hash() {
    let mut rng = thread_rng();
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut list = fork.get_proof_list(IDX_NAME);

    for _ in 0..10 {
        let values: [u32; 32] = rng.gen();
        let truncated_len = rng.gen_range(5, 25);
        let proof_ranges: Vec<_> = (0..50)
            .map(|_| {
                let start = rng.gen_range(0, truncated_len as u64);
                let end = rng.gen_range(start, truncated_len as u64) + 1;
                start..end
            })
            .collect();

        list.clear();
        list.extend(values[..truncated_len].iter().copied());
        let list_hash = list.object_hash();
        let expected_proofs: Vec<_> = proof_ranges
            .iter()
            .map(|range| list.get_range_proof(range.clone()))
            .collect();

        list.clear();
        list.extend(values.iter().copied());
        list.truncate(truncated_len as u64);
        assert_eq!(list.object_hash(), list_hash);
        for (i, range) in proof_ranges.into_iter().enumerate() {
            let proof = list.get_range_proof(range.clone());
            assert_eq!(
                proof, expected_proofs[i],
                "Unexpected proof for range {:?}",
                range
            );
        }
    }

    // Check different values of `truncated_len` (including extreme ones).
    let values: [u32; 17] = rng.gen();
    for truncated_len in 0..=values.len() {
        list.clear();
        list.extend(values[..truncated_len].iter().copied());
        let list_hash = list.object_hash();

        list.clear();
        list.extend(values.iter().copied());
        list.truncate(truncated_len as u64);
        assert_eq!(list.object_hash(), list_hash);
    }
}

#[test]
fn popping_element_from_list() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut list = fork.get_proof_list(IDX_NAME);
    list.extend(0_i32..10);

    let mut count = 0;
    while let Some(last) = list.pop() {
        count += 1;
        assert_eq!(last, 10 - count);
        assert_eq!(list.len(), 10 - count as u64);
    }
    assert!(list.is_empty());
    assert_eq!(list.object_hash(), HashTag::empty_list_hash());
}

#[test]
fn proof_json_serialization() {
    let mut proof = ListProof::new(vec![(1, "foo".to_owned()), (2, "bar".to_owned())], 5);
    proof.push_hash(1, 0, HashTag::hash_leaf(&[4]));
    proof.push_hash(2, 1, HashTag::hash_leaf(&[2]));
    proof.push_hash(3, 1, HashTag::hash_leaf(&[1]));

    let json = serde_json::to_value(&proof).unwrap();
    assert_eq!(
        json,
        json!({
            "entries": [(1, "foo"), (2, "bar")],
            "proof": [
                { "height": 1, "index": 0, "hash": HashTag::hash_leaf(&[4]) },
                { "height": 2, "index": 1, "hash": HashTag::hash_leaf(&[2]) },
                { "height": 3, "index": 1, "hash": HashTag::hash_leaf(&[1]) },
            ],
            "length": 5,
        })
    );

    let proof_from_json: ListProof<String> = serde_json::from_value(json).unwrap();
    assert_eq!(proof_from_json, proof);
}

#[test]
fn unordered_proofs() {
    let json = json!({
        "entries": [(2, "foo"), (1, "bar")],
        "proof": [],
        "length": 3,
    });
    let proof: ListProof<String> = serde_json::from_value(json).unwrap();
    assert_eq!(proof.check().unwrap_err(), ListProofError::Unordered);

    let json = json!({
        "entries": [(2, "foo")],
        "proof": [
            { "height": 1, "index": 3, "hash": Hash::zero() },
            { "height": 1, "index": 1, "hash": Hash::zero() },
        ],
        "length": 5,
    });
    let proof: ListProof<String> = serde_json::from_value(json).unwrap();
    assert_eq!(proof.check().unwrap_err(), ListProofError::Unordered);

    let json = json!({
        "entries": [(2, "foo")],
        "proof": [
            { "height": 2, "index": 1, "hash": Hash::zero() },
            { "height": 2, "index": 3, "hash": Hash::zero() },
            { "height": 1, "index": 2, "hash": Hash::zero() },
        ],
        "length": 100,
    });
    let proof: ListProof<String> = serde_json::from_value(json).unwrap();
    assert_eq!(proof.check().unwrap_err(), ListProofError::Unordered);
}

#[test]
fn non_empty_proof_for_empty_tree() {
    let json = json!({
        "entries": [(1, "bar")],
        "proof": [],
        "length": 0,
    });
    let proof: ListProof<String> = serde_json::from_value(json).unwrap();
    assert_eq!(proof.check().unwrap_err(), ListProofError::NonEmptyProof);

    let json = json!({
        "entries": [],
        "proof": [{ "height": 1, "index": 1, "hash": Hash::zero() }],
        "length": 0,
    });
    let proof: ListProof<String> = serde_json::from_value(json).unwrap();
    assert_eq!(proof.check().unwrap_err(), ListProofError::NonEmptyProof);
}

#[test]
fn proofs_with_unexpected_branches() {
    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [(2, 2)],
        "proof": [
            { "height": 10, "index": 2, "hash": Hash::zero() },
        ],
        "length": 10,
    }))
    .unwrap();
    assert_eq!(proof.check().unwrap_err(), ListProofError::UnexpectedBranch);

    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [(2, 2)],
        "proof": [
            { "height": 5, "index": 0, "hash": Hash::zero() },
        ],
        "length": 10,
    }))
    .unwrap();
    assert_eq!(proof.check().unwrap_err(), ListProofError::UnexpectedBranch);

    let mut proof = ListProof::new(vec![(1, "foo".to_owned()), (2, "bar".to_owned())], 3);
    proof.push_hash(2, 2, Hash::zero());
    assert_eq!(proof.check().unwrap_err(), ListProofError::UnexpectedBranch);

    let mut proof = ListProof::new(vec![(1, "foo".to_owned()), (2, "bar".to_owned())], 3);
    proof.push_hash(1, 4, Hash::zero());
    assert_eq!(proof.check().unwrap_err(), ListProofError::UnexpectedBranch);

    let mut proof = ListProof::new(vec![(1, "foo".to_owned()), (2, "bar".to_owned())], 5);
    proof.push_hash(1, 6, Hash::zero());
}

#[test]
fn proofs_with_unexpected_leaf() {
    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [(2, 2)],
        "proof": [
            { "height": 0, "index": 1, "hash": Hash::zero() },
            { "height": 1, "index": 1, "hash": Hash::zero() },
        ],
        "length": 5,
    }))
    .unwrap();
    assert_eq!(proof.check().unwrap_err(), ListProofError::UnexpectedLeaf);
}

#[test]
fn proofs_with_missing_entry() {
    let proof = ListProof::new(vec![(1, 1_u64), (2, 2)], 3);
    // (1, 0) is missing
    assert_eq!(proof.check().unwrap_err(), ListProofError::MissingHash);

    let mut proof = ListProof::new(vec![(1, 1_u64)], 7);
    proof.push_hash(1, 0, Hash::zero());
    // (2, 1) is missing
    assert_eq!(proof.check().unwrap_err(), ListProofError::MissingHash);

    let mut proof = ListProof::new(vec![(1, 1_u64), (2, 2)], 9);
    proof.push_hash(1, 0, Hash::zero());
    proof.push_hash(1, 3, Hash::zero());
    // (3, 1) is missing
    assert_eq!(proof.check().unwrap_err(), ListProofError::MissingHash);

    let mut proof = ListProof::new(vec![(1, 1_u64), (2, 2), (4, 4)], 8);
    proof.push_hash(1, 0, Hash::zero());
    proof.push_hash(1, 3, Hash::zero());
    proof.push_hash(2, 3, Hash::zero());
    assert_eq!(proof.check().unwrap_err(), ListProofError::MissingHash);
}

#[test]
fn invalid_proofs_with_no_values() {
    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [],
        "proof": [],
        "length": 1,
    }))
    .unwrap();
    assert_eq!(
        proof.check().unwrap_err(),
        ListProofError::MissingHash // we expected 1 hash
    );

    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [],
        "proof": [
            { "height": 0, "index": 1, "hash": Hash::zero() },
            { "height": 1, "index": 1, "hash": Hash::zero() },
        ],
        "length": 5,
    }))
    .unwrap();
    assert_eq!(
        proof.check().unwrap_err(),
        ListProofError::UnexpectedBranch // we expected 1 hash, got 2
    );

    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [],
        "proof": [
            { "height": 0, "index": 1, "hash": Hash::zero() },
        ],
        "length": 5,
    }))
    .unwrap();
    assert_eq!(
        proof.check().unwrap_err(),
        ListProofError::UnexpectedBranch // the hash is at an incorrect position
    );
}

#[test]
fn proof_with_out_of_bound_elements() {
    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [],
        "proof": [],
        "length": (1_u64 << 56) + 1,
    }))
    .unwrap();
    assert_eq!(
        proof.check().unwrap_err(),
        ListProofError::OutOfBounds // length is overly large
    );
    assert_eq!(proof.hash_ops().unwrap_err(), ListProofError::OutOfBounds);

    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [(0, 0), (1_u64 << 56, 42)],
        "proof": [],
        "length": 100,
    }))
    .unwrap();
    assert_eq!(
        proof.check().unwrap_err(),
        ListProofError::OutOfBounds // one of the entry indexes is overly large
    );
    assert_eq!(proof.hash_ops().unwrap_err(), ListProofError::OutOfBounds);

    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [],
        "proof": [
            { "height": 57, "index": 0, "hash": Hash::zero() },
        ],
        "length": 100,
    }))
    .unwrap();
    assert_eq!(
        proof.check().unwrap_err(),
        ListProofError::OutOfBounds // `height` is overly large
    );
    assert_eq!(proof.hash_ops().unwrap_err(), ListProofError::OutOfBounds);

    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [],
        "proof": [
            { "height": 1, "index": 1_u64 << 56, "hash": Hash::zero() },
        ],
        "length": 100,
    }))
    .unwrap();
    assert_eq!(
        proof.check().unwrap_err(),
        ListProofError::OutOfBounds // `index` is overly large
    );
    assert_eq!(proof.hash_ops().unwrap_err(), ListProofError::OutOfBounds);
}

mod root_hash {
    use crate::{
        access::CopyAccessExt, hash::HashTag, BinaryValue, Database, ObjectHash, TemporaryDB,
    };
    use exonum_crypto::{self, Hash};

    /// Cross-verify `object_hash()` with `ProofListIndex` against expected root hash value.
    fn assert_object_hash_correct<V>(values: &[V])
    where
        V: BinaryValue + Clone,
    {
        let root_actual = HashTag::hash_list(values);
        let root_index = proof_list_index_root(values);
        assert_eq!(root_actual, root_index);
    }

    fn proof_list_index_root<V>(hashes: &[V]) -> Hash
    where
        V: BinaryValue + Clone,
    {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut index = fork.get_proof_list("merkle_root");
        index.extend(hashes.iter().cloned());
        index.object_hash()
    }

    fn to_list_of_hashes(bytes: &[&[u8]]) -> Vec<Hash> {
        bytes
            .iter()
            .map(|chunk| exonum_crypto::hash(chunk))
            .collect()
    }

    #[test]
    fn object_hash_single() {
        assert_object_hash_correct(&to_list_of_hashes(&[b"1"]));
    }

    #[test]
    fn object_hash_even() {
        assert_object_hash_correct(&to_list_of_hashes(&[b"1", b"2", b"3", b"4"]));
    }

    #[test]
    fn object_hash_odd() {
        assert_object_hash_correct(&to_list_of_hashes(&[b"1", b"2", b"3", b"4", b"5"]));
    }

    #[test]
    fn object_hash_with_integers() {
        let numbers = [2_u32, 3, 5, 8, 13, 21, 34, 55];
        for i in 1..numbers.len() {
            assert_object_hash_correct(&numbers[..i]);
        }
    }

    #[test]
    fn object_hash_with_bytes() {
        let bytes: Vec<_> = [b"foo" as &[_], b"bar", b"bark", b"lorem", b"ipsum"]
            .iter()
            .map(|slice| slice.to_vec())
            .collect();
        for i in 1..bytes.len() {
            assert_object_hash_correct(&bytes[..i]);
        }
    }

    #[test]
    fn object_hash_with_strings() {
        const STRING: &str =
            "All human beings are born free and equal in dignity and rights. \
             They are endowed with reason and conscience and should act towards one another \
             in a spirit of brotherhood.";

        let words: Vec<_> = STRING.split_whitespace().map(str::to_owned).collect();
        for i in 1..words.len() {
            assert_object_hash_correct(&words[..i]);
        }
    }

    #[test]
    fn object_hash_empty() {
        assert_object_hash_correct(&to_list_of_hashes(&[]));
    }
}
