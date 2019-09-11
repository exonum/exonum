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

//! Property testing for proofs of existence / absence in `ProofMapIndex`.
//!
//! To adjust the number of test cases for each test, set the `PROPTEST_CASES` environment
//! variable as per `proptest` docs. The number of test cases for large tests will be scaled
//! back automatically. A reasonable value for `PROPTEST_CASES` is `256`
//! (default; results in running time ~30 sec for larger tests) or more. The run time
//! scales linearly with the number of cases.

// cspell:ignore proptest

use exonum_merkledb::{
    proof_map_index::ProofPath, BinaryKey, Database, IndexAccess, MapProof, ProofMapIndex,
    TemporaryDB,
};
use proptest::{
    prelude::prop::{
        array,
        collection::{btree_map, vec},
    },
    prelude::*,
    test_runner::Config,
};

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    ops::Range,
};

use exonum_merkledb::{BinaryValue, ObjectHash};

const INDEX_NAME: &str = "index";

fn check_map_proof<T, K, V>(proof: &MapProof<K, V>, key: Option<K>, table: &ProofMapIndex<T, K, V>)
where
    T: IndexAccess,
    K: BinaryKey + ObjectHash + PartialEq + Debug,
    V: BinaryValue + PartialEq + Debug,
{
    let entry = key.map(|key| {
        let value = table.get(&key).unwrap();
        (key, value)
    });
    let proof = proof.check_against_hash(table.object_hash()).unwrap();
    assert!(proof.entries().eq(entry.as_ref().map(|(k, v)| (k, v))));
}

fn check_map_multiproof<T, K, V>(
    proof: &MapProof<K, V>,
    keys: BTreeSet<K>,
    table: &ProofMapIndex<T, K, V>,
) where
    T: IndexAccess,
    K: BinaryKey + ObjectHash + PartialEq + Debug,
    V: BinaryValue + PartialEq + Debug,
{
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
    entries.sort_unstable_by(|(x, _), (y, _)| {
        ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap()
    });
    missing_keys
        .sort_unstable_by(|x, y| ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap());

    let unchecked_proof = proof;
    let proof = proof.check().unwrap();
    assert!(proof
        .all_entries()
        .eq(unchecked_proof.all_entries_unchecked()));
    assert_eq!(proof.object_hash(), table.object_hash());

    let mut actual_keys: Vec<_> = proof.missing_keys().collect();
    actual_keys
        .sort_unstable_by(|&x, &y| ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap());
    assert!(missing_keys.iter().eq(actual_keys));

    let mut actual_entries: Vec<_> = proof.entries().collect();
    actual_entries.sort_unstable_by(|&(x, _), &(y, _)| {
        ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap()
    });
    assert!(entries.iter().map(|(k, v)| (k, v)).eq(actual_entries));
}

// Creates data a random-filled `ProofMapIndex<_, [u8; 32], u64>`.
fn index_data<S>(key_bytes: S, elements_len: Range<usize>) -> BoxedStrategy<BTreeMap<[u8; 32], u64>>
where
    S: 'static + Strategy<Value = u8>,
{
    btree_map(array::uniform32(key_bytes), any::<u64>(), elements_len).boxed()
}

// Converts raw data to a database.
fn data_to_db(data: BTreeMap<[u8; 32], u64>) -> TemporaryDB {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut table = ProofMapIndex::new(INDEX_NAME, &fork);
        for (key, value) in data {
            table.put(&key, value);
        }
    }
    db.merge(fork.into_patch()).unwrap();
    db
}

macro_rules! proof_map_tests {
    (cases = $cases:expr,sizes = $sizes:expr,bytes = $bytes:expr) => {
        proptest! {
            #![proptest_config(Config::with_cases($cases))]

            #[test]
            fn proof_of_presence(
                (key, ref db) in index_data($bytes, $sizes)
                    .prop_flat_map(|data| (0..data.len(), Just(data)))
                    .prop_map(|(index, data)| {
                        (*data.keys().nth(index).unwrap(), data_to_db(data))
                    })
            ) {
                let snapshot = db.snapshot();
                let table: ProofMapIndex<_, [u8; 32], u64> =
                    ProofMapIndex::new(INDEX_NAME, &snapshot);
                let proof = table.get_proof(key);
                check_map_proof(&proof, Some(key), &table);
            }


            #[test]
            fn proof_of_absence(
                ref db in index_data($bytes, $sizes).prop_map(data_to_db),
                key in array::uniform32($bytes)
            ) {
                let snapshot = db.snapshot();
                let table: ProofMapIndex<_, [u8; 32], u64> =
                    ProofMapIndex::new(INDEX_NAME, &snapshot);
                prop_assume!(!table.contains(&key));

                let proof = table.get_proof(key);
                check_map_proof(&proof, None, &table);
            }

            #[test]
            fn multiproof_of_existing_elements(
                (ref keys, ref db) in index_data($bytes, $sizes)
                    .prop_flat_map(|data| {
                        (vec(0..data.len(), data.len() / 5), Just(data))
                    })
                    .prop_map(|(indexes, data)| {
                        // Note that keys may coincide; this is intentional.
                        let keys: Vec<_> = indexes
                            .into_iter()
                            .map(|i| *data.keys().nth(i).unwrap())
                            .collect();
                        (keys, data_to_db(data))
                    })
            ) {
                            let snapshot = db.snapshot();
                let table: ProofMapIndex<_, [u8; 32], u64> =
                    ProofMapIndex::new(INDEX_NAME, &snapshot);
                let proof = table.get_multiproof(keys.clone());

                let unique_keys: BTreeSet<_> = keys.iter().cloned().collect();
                check_map_multiproof(&proof, unique_keys, &table);
            }

            #[test]
            fn multiproof_of_nonexisting_elements(
                ref db in index_data($bytes, $sizes).prop_map(data_to_db),
                ref keys in vec(array::uniform32($bytes), 20)
            ) {
                                            let snapshot = db.snapshot();
                let table: ProofMapIndex<_, [u8; 32], u64> =
                    ProofMapIndex::new(INDEX_NAME, &snapshot);
                prop_assume!(keys.iter().all(|key| !table.contains(key)));

                let proof = table.get_multiproof(keys.clone());
                let unique_keys: BTreeSet<_> = keys.iter().cloned().collect();
                check_map_multiproof(&proof, unique_keys, &table);
            }

            #[test]
            fn mixed_multiproof(
                (ref keys, ref db) in index_data($bytes, $sizes)
                    .prop_flat_map(|data| {
                        (vec(0..data.len(), data.len() / 5), Just(data))
                    })
                    .prop_map(|(indexes, data)| {
                        // Note that keys may coincide; this is intentional.
                        let keys: Vec<_> = indexes
                            .into_iter()
                            .map(|i| *data.keys().nth(i).unwrap())
                            .collect();
                        (keys, data_to_db(data))
                    }),
                ref absent_keys in vec(array::uniform32($bytes), 20)
            ) {
                                        let snapshot = db.snapshot();
                let table: ProofMapIndex<_, [u8; 32], u64> =
                    ProofMapIndex::new(INDEX_NAME, &snapshot);

                let mut all_keys = keys.clone();
                all_keys.extend_from_slice(absent_keys);
                let proof = table.get_multiproof(all_keys.clone());

                let unique_keys: BTreeSet<_> = all_keys.into_iter().collect();
                check_map_multiproof(&proof, unique_keys, &table);
            }
        }
    };
}

mod small_index {
    use super::*;

    proof_map_tests!(
        cases = Config::default().cases,
        sizes = 10..100,
        bytes = 0_u8..
    );
}

mod small_index_skewed {
    use super::*;

    proof_map_tests!(
        cases = Config::default().cases,
        sizes = 10..100,
        bytes = 0_u8..3
    );
}

mod large_index {
    use super::*;

    proof_map_tests!(
        cases = Config::default().cases >> 5,
        sizes = 5_000..10_000,
        bytes = 0_u8..
    );
}

mod large_index_skewed {
    use super::*;

    proof_map_tests!(
        cases = Config::default().cases >> 5,
        sizes = 5_000..10_000,
        bytes = 0_u8..3
    );
}
