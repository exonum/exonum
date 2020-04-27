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

//! Property testing for proofs of existence / absence in `ProofMapIndex`.
//!
//! To adjust the number of test cases for each test, set the `PROPTEST_CASES` environment
//! variable as per `proptest` docs. The number of test cases for large tests will be scaled
//! back automatically. A reasonable value for `PROPTEST_CASES` is `256`
//! (default; results in running time ~30 sec for larger tests) or more. The run time
//! scales linearly with the number of cases.

// cspell:ignore proptest

use exonum_merkledb::{
    access::{CopyAccessExt, RawAccess},
    proof_map::{Hashed, ToProofPath},
    BinaryKey, BinaryValue, Database, MapProof, ObjectHash, ProofMapIndex, TemporaryDB,
};
use proptest::{
    prelude::prop::{
        array,
        collection::{btree_map, vec},
    },
    prelude::*,
    test_runner::{Config, TestCaseError, TestCaseResult},
};

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    ops::{Range, RangeInclusive},
};

use crate::key::Key;

mod key;

const INDEX_NAME: &str = "index";

type Data = BTreeMap<[u8; 32], u64>;

fn check_map_proof<T, K, V>(
    proof: &MapProof<K, V>,
    key: Option<K>,
    table: &ProofMapIndex<T, K, V>,
) -> TestCaseResult
where
    T: RawAccess,
    K: BinaryKey + ObjectHash + PartialEq + Debug,
    V: BinaryValue + PartialEq + Debug,
{
    let entry = key.map(|key| {
        let value = table.get(&key).unwrap();
        (key, value)
    });
    let proof = proof
        .check_against_hash(table.object_hash())
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
    prop_assert!(proof.entries().eq(entry.as_ref().map(|(k, v)| (k, v))));
    Ok(())
}

fn check_map_multiproof<T, K, V>(
    proof: &MapProof<K, V, Hashed>,
    keys: BTreeSet<&K>,
    table: &ProofMapIndex<T, K, V>,
) -> TestCaseResult
where
    T: RawAccess,
    K: BinaryKey + ObjectHash + PartialEq + Debug,
    V: BinaryValue + PartialEq + Debug,
{
    let mut entries: Vec<(&K, V)> = Vec::new();
    let mut missing_keys: Vec<&K> = Vec::new();

    for key in keys {
        if table.contains(key) {
            let value = table.get(key).unwrap();
            entries.push((key, value));
        } else {
            missing_keys.push(key);
        }
    }

    // Sort entries and missing keys by the order imposed by the `ProofPath`
    // serialization of the keys
    entries.sort_unstable_by(|(x, _), (y, _)| {
        Hashed::transform_key(*x)
            .partial_cmp(&Hashed::transform_key(*y))
            .unwrap()
    });
    missing_keys.sort_unstable_by(|&x, &y| {
        Hashed::transform_key(x)
            .partial_cmp(&Hashed::transform_key(y))
            .unwrap()
    });

    let unchecked_proof = proof;
    let proof = proof
        .check()
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
    prop_assert!(proof
        .all_entries()
        .eq(unchecked_proof.all_entries_unchecked()));
    prop_assert_eq!(proof.index_hash(), table.object_hash());

    let mut actual_keys: Vec<&K> = proof.missing_keys().collect();
    actual_keys.sort_unstable_by(|&x, &y| {
        Hashed::transform_key(x)
            .partial_cmp(&Hashed::transform_key(y))
            .unwrap()
    });
    prop_assert_eq!(missing_keys, actual_keys);

    let mut actual_entries: Vec<(&K, &V)> = proof.entries().collect();
    actual_entries.sort_unstable_by(|&(x, _), &(y, _)| {
        Hashed::transform_key(x)
            .partial_cmp(&Hashed::transform_key(y))
            .unwrap()
    });
    prop_assert!(entries.iter().map(|(k, v)| (*k, v)).eq(actual_entries));
    Ok(())
}

/// Writes raw data to a database.
fn write_data(db: &TemporaryDB, data: Data) {
    let fork = db.fork();
    {
        let mut table: ProofMapIndex<_, Key, _> = fork.get_proof_map(INDEX_NAME);
        table.clear();
        for (key, value) in data {
            table.put(&key.into(), value);
        }
    }
    db.merge(fork.into_patch()).unwrap();
}

/// Creates data for a random-filled `ProofMapIndex<_, [u8; 32], u64>`.
fn index_data(
    key_bytes: impl Strategy<Value = u8>,
    sizes: Range<usize>,
) -> impl Strategy<Value = Data> {
    btree_map(array::uniform32(key_bytes), any::<u64>(), sizes)
}

fn absent_keys(key_bytes: RangeInclusive<u8>) -> impl Strategy<Value = Vec<Key>> {
    vec(array::uniform32(key_bytes).prop_map(Key), 20)
}

/// Generates data to test a proof of presence.
fn data_for_proof_of_presence(
    key_bytes: impl Strategy<Value = u8>,
    sizes: Range<usize>,
) -> impl Strategy<Value = (Key, Data)> {
    index_data(key_bytes, sizes)
        .prop_flat_map(|data| (0..data.len(), Just(data)))
        .prop_map(|(index, data)| (*data.keys().nth(index).unwrap(), data))
        .prop_map(|(index, data)| (index.into(), data))
}

fn data_for_multiproof(
    key_bytes: impl Strategy<Value = u8>,
    sizes: Range<usize>,
) -> impl Strategy<Value = (Vec<Key>, Data)> {
    index_data(key_bytes, sizes)
        .prop_flat_map(|data| (vec(0..data.len(), data.len() / 5), Just(data)))
        .prop_map(|(indexes, data)| {
            // Note that keys may coincide; this is intentional.
            let keys: Vec<Key> = indexes
                .into_iter()
                .map(|i| *data.keys().nth(i).unwrap())
                .map(Key)
                .collect();
            (keys, data)
        })
}

fn test_proof(db: &TemporaryDB, key: Key) -> TestCaseResult {
    let snapshot = db.snapshot();
    let table: ProofMapIndex<_, Key, u64> = snapshot.get_proof_map(INDEX_NAME);
    let proof = table.get_proof(key);
    let expected_key = if table.contains(&key) {
        Some(key)
    } else {
        None
    };
    check_map_proof(&proof, expected_key, &table)
}

fn test_multiproof(db: &TemporaryDB, keys: &[Key]) -> TestCaseResult {
    let snapshot = db.snapshot();
    let table: ProofMapIndex<_, Key, u64> = snapshot.get_proof_map(INDEX_NAME);
    let proof = table.get_multiproof(keys.to_vec());
    let unique_keys: BTreeSet<_> = keys.iter().collect();
    check_map_multiproof(&proof, unique_keys, &table)
}

#[derive(Debug, Clone)]
struct TestParams {
    key_bytes: RangeInclusive<u8>,
    index_sizes: Range<usize>,
    test_cases_divider: u32,
}

impl TestParams {
    fn key_bytes(&self) -> RangeInclusive<u8> {
        self.key_bytes.clone()
    }

    fn index_sizes(&self) -> Range<usize> {
        self.index_sizes.clone()
    }

    fn config(&self) -> Config {
        Config::with_cases(Config::default().cases / self.test_cases_divider)
    }

    fn proof_of_presence(&self) {
        let db = TemporaryDB::new();
        let strategy = data_for_proof_of_presence(self.key_bytes(), self.index_sizes());
        proptest!(self.config(), |((key, data) in strategy)| {
            write_data(&db, data);
            test_proof(&db, key)?;
        });
    }

    fn proof_of_absence(&self) {
        let db = TemporaryDB::new();
        let key_strategy = array::uniform32(self.key_bytes()).prop_map(Key);
        let data_strategy = index_data(self.key_bytes(), self.index_sizes());
        proptest!(self.config(), |(key in key_strategy, data in data_strategy)| {
            write_data(&db, data);
            test_proof(&db, key)?;
        });
    }

    fn multiproof_of_existing_elements(&self) {
        let db = TemporaryDB::new();
        let strategy = data_for_multiproof(self.key_bytes(), self.index_sizes());
        proptest!(self.config(), |((keys, data) in strategy)| {
            write_data(&db, data);
            test_multiproof(&db, &keys)?;
        });
    }

    fn multiproof_of_absent_elements(&self) {
        let db = TemporaryDB::new();
        let keys_strategy = absent_keys(self.key_bytes());
        let data_strategy = index_data(self.key_bytes(), self.index_sizes());
        proptest!(self.config(), |(keys in keys_strategy, data in data_strategy)| {
            write_data(&db, data);
            test_multiproof(&db, &keys)?;
        });
    }

    fn mixed_multiproof(&self) {
        let db = TemporaryDB::new();
        let strategy = data_for_multiproof(self.key_bytes(), self.index_sizes());
        let absent_keys_strategy = absent_keys(self.key_bytes());
        proptest!(
            self.config(),
            |((mut keys, data) in strategy, absent_keys in absent_keys_strategy)| {
                write_data(&db, data);
                keys.extend_from_slice(&absent_keys);
                test_multiproof(&db, &keys)?;
            }
        );
    }
}

mod small_index {
    use super::*;

    const PARAMS: TestParams = TestParams {
        key_bytes: 0..=255,
        index_sizes: 10..100,
        test_cases_divider: 1,
    };

    #[test]
    fn proof_of_presence() {
        PARAMS.proof_of_presence();
    }

    #[test]
    fn proof_of_absence() {
        PARAMS.proof_of_absence();
    }

    #[test]
    fn multiproof_of_existing_elements() {
        PARAMS.multiproof_of_existing_elements();
    }

    #[test]
    fn multiproof_of_absent_elements() {
        PARAMS.multiproof_of_absent_elements();
    }

    #[test]
    fn mixed_multiproof() {
        PARAMS.mixed_multiproof();
    }
}

mod small_index_skewed {
    use super::*;

    const PARAMS: TestParams = TestParams {
        key_bytes: 0..=2,
        index_sizes: 10..100,
        test_cases_divider: 1,
    };

    #[test]
    fn proof_of_presence() {
        PARAMS.proof_of_presence();
    }

    #[test]
    fn proof_of_absence() {
        PARAMS.proof_of_absence();
    }

    #[test]
    fn multiproof_of_existing_elements() {
        PARAMS.multiproof_of_existing_elements();
    }

    #[test]
    fn multiproof_of_absent_elements() {
        PARAMS.multiproof_of_absent_elements();
    }

    #[test]
    fn mixed_multiproof() {
        PARAMS.mixed_multiproof();
    }
}

mod large_index {
    use super::*;

    const PARAMS: TestParams = TestParams {
        key_bytes: 0..=255,
        index_sizes: 5_000..10_000,
        test_cases_divider: 32,
    };

    #[test]
    fn proof_of_presence() {
        PARAMS.proof_of_presence();
    }

    #[test]
    fn proof_of_absence() {
        PARAMS.proof_of_absence();
    }

    #[test]
    fn multiproof_of_existing_elements() {
        PARAMS.multiproof_of_existing_elements();
    }

    #[test]
    fn multiproof_of_absent_elements() {
        PARAMS.multiproof_of_absent_elements();
    }

    #[test]
    fn mixed_multiproof() {
        PARAMS.mixed_multiproof();
    }
}

mod large_index_skewed {
    use super::*;

    const PARAMS: TestParams = TestParams {
        key_bytes: 0..=2,
        index_sizes: 5_000..10_000,
        test_cases_divider: 32,
    };

    #[test]
    fn proof_of_presence() {
        PARAMS.proof_of_presence();
    }

    #[test]
    fn proof_of_absence() {
        PARAMS.proof_of_absence();
    }

    #[test]
    fn multiproof_of_existing_elements() {
        PARAMS.multiproof_of_existing_elements();
    }

    #[test]
    fn multiproof_of_absent_elements() {
        PARAMS.multiproof_of_absent_elements();
    }

    #[test]
    fn mixed_multiproof() {
        PARAMS.mixed_multiproof();
    }
}
