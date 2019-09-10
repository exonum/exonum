// Copyright 2019 The Exonum Team
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

use exonum_crypto::Hash;
use rand::{thread_rng, Rng};
use serde_json::{self, json};

use std::cmp;

use super::{ListProof, ListProofError, ProofListIndex};
use crate::{BinaryValue, Database, HashTag, ObjectHash, TemporaryDB};

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
    let mut index = ProofListIndex::new(IDX_NAME, &fork);

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
fn tree_height() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = ProofListIndex::new(IDX_NAME, &fork);

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
    let mut list_index = ProofListIndex::new(IDX_NAME, &fork);

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
fn simple_proof() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = ProofListIndex::new(IDX_NAME, &fork);

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
    assert_eq!(*proof.validate(index.object_hash()).unwrap(), [(0, 2)]);

    index.push(4_u64);
    assert_eq!(index.object_hash(), HashTag::hash_list_node(2, h01));

    let proof = index.get_proof(0);
    assert_eq!(proof, {
        let mut proof = ListProof::new(vec![(0, 2_u64)], index.len());
        proof.push_hash(1, 1, h1);
        proof
    });
    assert_eq!(*proof.validate(index.object_hash()).unwrap(), [(0, 2)]);

    let proof = index.get_proof(1);
    assert_eq!(proof, {
        let mut proof = ListProof::new(vec![(1, 4_u64)], index.len());
        proof.push_hash(1, 0, h0);
        proof
    });
    assert_eq!(*proof.validate(index.object_hash()).unwrap(), [(1, 4)]);

    let proof = index.get_range_proof(0..2);
    assert_eq!(
        proof,
        ListProof::new(vec![(0, 2_u64), (1, 4_u64)], index.len())
    );
    assert_eq!(
        *proof.validate(index.object_hash()).unwrap(),
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
    assert_eq!(*proof.validate(index.object_hash()).unwrap(), [(0, 2)]);

    let proof = index.get_range_proof(1..3);
    assert_eq!(proof, {
        let mut proof = ListProof::new(vec![(1, 4_u64), (2, 6_u64)], index.len());
        proof.push_hash(1, 0, h0);
        proof
    });
    assert_eq!(
        *proof.validate(index.object_hash()).unwrap(),
        [(1, 4_u64), (2, 6_u64)]
    );

    let proof = index.get_range_proof(0..2);
    assert_eq!(proof, {
        let mut proof = ListProof::new(vec![(0, 2_u64), (1, 4_u64)], index.len());
        proof.push_hash(2, 1, h22);
        proof
    });
    assert_eq!(
        *proof.validate(index.object_hash()).unwrap(),
        [(0, 2_u64), (1, 4_u64)]
    );
}

#[test]
fn random_proofs() {
    const LIST_SIZE: usize = 1 << 10;
    const MAX_RANGE_SIZE: u64 = 128;

    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = ProofListIndex::new(IDX_NAME, &fork);

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
        let (indices, actual_values): (Vec<_>, Vec<_>) = range_proof
            .validate(index_hash)
            .unwrap()
            .to_vec()
            .into_iter()
            .unzip();
        assert_eq!(indices, (start..end).collect::<Vec<_>>());

        let expected_values = &values[start as usize..end as usize];
        assert_eq!(expected_values, actual_values.as_slice());
    }
}

#[test]
fn index_and_proof_roots() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = ProofListIndex::new(IDX_NAME, &fork);
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
        assert_eq!(range_proof.validate(index.object_hash()).unwrap().len(), 1);
        let js = serde_json::to_string(&range_proof).unwrap();
        let deserialized_proof: ListProof<Vec<u8>> = serde_json::from_str(&js).unwrap();
        assert_eq!(deserialized_proof, range_proof);

        let range_proof = index.get_range_proof(..=proof_ind);
        assert_eq!(
            range_proof.validate(index.object_hash()).unwrap().len(),
            (proof_ind + 1) as usize
        );
        let js = serde_json::to_string(&range_proof).unwrap();
        let deserialized_proof: ListProof<Vec<u8>> = serde_json::from_str(&js).unwrap();
        assert_eq!(deserialized_proof, range_proof);

        let range_proof = index.get_range_proof(0..1);
        assert_eq!(range_proof.validate(index.object_hash()).unwrap().len(), 1);
        let js = serde_json::to_string(&range_proof).unwrap();
        let deserialized_proof: ListProof<Vec<u8>> = serde_json::from_str(&js).unwrap();
        assert_eq!(deserialized_proof, range_proof);
    }

    let range_proof = index.get_range_proof(..);
    let (indices, val_refs): (Vec<_>, Vec<_>) = range_proof
        .validate(index.object_hash())
        .unwrap()
        .iter()
        .cloned()
        .unzip();
    assert_eq!(indices, (0..8).collect::<Vec<_>>());
    let expected_values = vec![
        vec![1, 2],
        vec![2, 3],
        vec![3, 4],
        vec![4, 5],
        vec![5, 6],
        vec![6, 7],
        vec![7, 8],
        vec![8, 9],
    ];
    for (expected, actual) in expected_values.into_iter().zip(val_refs) {
        assert_eq!(expected[..], actual[..]);
    }

    let mut range_proof = index.get_range_proof(3..5);
    assert_eq!(range_proof.validate(index.object_hash()).unwrap().len(), 2);
    range_proof = index.get_range_proof(2..6);
    assert_eq!(range_proof.validate(index.object_hash()).unwrap().len(), 4);
    assert_eq!(index.get(0), Some(vec![1, 2]));
}

#[test]
#[should_panic(expected = "the range start is 2, but the range end is 2")]
fn proof_illegal_range() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = ProofListIndex::new(IDX_NAME, &fork);
    for i in 0_u8..4 {
        index.push(vec![i]);
    }
    index.get_range_proof(2..2);
}

#[test]
fn proof_with_range_end_exceeding_list_size() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = ProofListIndex::new(IDX_NAME, &fork);
    for i in 0_u8..4 {
        index.push(vec![i]);
    }

    let proof = index.get_range_proof(2..10);
    assert_eq!(
        proof.validate(index.object_hash()).unwrap().len(),
        2 // elements 2 and 3
    );
}

#[test]
fn proof_with_range_start_exceeding_list_size() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let mut index = ProofListIndex::new(IDX_NAME, &fork);
    for i in 0_u8..4 {
        index.push(vec![i]);
    }

    let proof = index.get_range_proof(8..10_000_000);
    assert!(proof.validate(index.object_hash()).unwrap().is_empty());

    let proof = index.get_range_proof(8..);
    assert!(proof.validate(index.object_hash()).unwrap().is_empty());
}

#[test]
fn same_merkle_root() {
    let db = TemporaryDB::new();
    let hash1 = {
        let fork = db.fork();
        let mut list = ProofListIndex::new(IDX_NAME, &fork);
        list.push(vec![1]);
        list.push(vec![2]);
        list.push(vec![3]);
        list.push(vec![4]);

        list.set(0, vec![4]);
        list.set(1, vec![7]);
        list.set(2, vec![5]);
        list.set(3, vec![1]);
        list.merkle_root()
    };
    let hash2 = {
        let fork = db.fork();
        let mut list = ProofListIndex::new(IDX_NAME, &fork);
        list.push(vec![4]);
        list.push(vec![7]);
        list.push(vec![5]);
        list.push(vec![1]);
        list.merkle_root()
    };
    assert_eq!(hash1, hash2);
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
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::Unordered
    );

    let json = json!({
        "entries": [(2, "foo")],
        "proof": [
            { "height": 1, "index": 3, "hash": Hash::zero() },
            { "height": 1, "index": 1, "hash": Hash::zero() },
        ],
        "length": 5,
    });
    let proof: ListProof<String> = serde_json::from_value(json).unwrap();
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::Unordered
    );

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
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::Unordered
    );
}

#[test]
fn non_empty_proof_for_empty_tree() {
    let json = json!({
        "entries": [(1, "bar")],
        "proof": [],
        "length": 0,
    });
    let proof: ListProof<String> = serde_json::from_value(json).unwrap();
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::NonEmptyProof
    );

    let json = json!({
        "entries": [],
        "proof": [{ "height": 1, "index": 1, "hash": Hash::zero() }],
        "length": 0,
    });
    let proof: ListProof<String> = serde_json::from_value(json).unwrap();
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::NonEmptyProof
    );
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
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::UnexpectedBranch
    );

    let proof: ListProof<u64> = serde_json::from_value(json!({
        "entries": [(2, 2)],
        "proof": [
            { "height": 5, "index": 0, "hash": Hash::zero() },
        ],
        "length": 10,
    }))
    .unwrap();
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::UnexpectedBranch
    );

    let mut proof = ListProof::new(vec![(1, "foo".to_owned()), (2, "bar".to_owned())], 3);
    proof.push_hash(2, 2, Hash::zero());
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::UnexpectedBranch
    );

    let mut proof = ListProof::new(vec![(1, "foo".to_owned()), (2, "bar".to_owned())], 3);
    proof.push_hash(1, 4, Hash::zero());
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::UnexpectedBranch
    );

    let mut proof = ListProof::new(vec![(1, "foo".to_owned()), (2, "bar".to_owned())], 5);
    proof.push_hash(1, 6, Hash::zero());
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::UnexpectedBranch
    );
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
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::UnexpectedLeaf
    );
}

#[test]
fn proofs_with_missing_entry() {
    let proof = ListProof::new(vec![(1, 1_u64), (2, 2)], 3);
    // (1, 0) is missing
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::MissingHash
    );

    let mut proof = ListProof::new(vec![(1, 1_u64)], 7);
    proof.push_hash(1, 0, Hash::zero());
    // (2, 1) is missing
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::MissingHash
    );

    let mut proof = ListProof::new(vec![(1, 1_u64), (2, 2)], 9);
    proof.push_hash(1, 0, Hash::zero());
    proof.push_hash(1, 3, Hash::zero());
    // (3, 1) is missing
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::MissingHash
    );

    let mut proof = ListProof::new(vec![(1, 1_u64), (2, 2), (4, 4)], 8);
    proof.push_hash(1, 0, Hash::zero());
    proof.push_hash(1, 3, Hash::zero());
    proof.push_hash(2, 3, Hash::zero());
    assert_eq!(
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::MissingHash
    );
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
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
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
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
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
        proof.validate(HashTag::empty_list_hash()).unwrap_err(),
        ListProofError::UnexpectedBranch // the hash is at an incorrect position
    );
}

mod root_hash {
    use crate::{
        hash::HashTag, proof_list_index::ProofListIndex, Database, ObjectHash, TemporaryDB,
    };
    use exonum_crypto::{self, Hash};

    /// Cross-verify `object_hash()` with `ProofListIndex` against expected root hash value.
    fn assert_object_hash_correct(hashes: &[Hash]) {
        let root_actual = HashTag::hash_list(hashes);
        let root_index = proof_list_index_root(hashes);
        assert_eq!(root_actual, root_index);
    }

    fn proof_list_index_root(hashes: &[Hash]) -> Hash {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut index = ProofListIndex::new("merkle_root", &fork);
        index.extend(hashes.iter().cloned());
        index.object_hash()
    }

    fn hash_list(bytes: &[&[u8]]) -> Vec<Hash> {
        bytes
            .iter()
            .map(|chunk| exonum_crypto::hash(chunk))
            .collect()
    }

    #[test]
    fn object_hash_single() {
        assert_object_hash_correct(&hash_list(&[b"1"]));
    }

    #[test]
    fn object_hash_even() {
        assert_object_hash_correct(&hash_list(&[b"1", b"2", b"3", b"4"]));
    }

    #[test]
    fn object_hash_odd() {
        assert_object_hash_correct(&hash_list(&[b"1", b"2", b"3", b"4", b"5"]));
    }

    #[test]
    fn object_hash_empty() {
        assert_object_hash_correct(&hash_list(&[]));
    }
}
