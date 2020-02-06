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

//! Property testing for index iterators.

use proptest::{
    collection::{hash_map, vec},
    num, prop_assert, prop_assert_eq, prop_oneof, proptest, sample, strategy,
    strategy::Strategy,
    test_runner::TestCaseResult,
};

use std::collections::{BTreeMap, BTreeSet};

use exonum_merkledb::{
    access::{Access, AccessExt, Prefixed, RawAccessMut},
    generic::{ErasedAccess, IntoErased},
    indexes::IndexIterator,
    BinaryKey, Database, Fork, IndexAddress, IndexType, ObjectHash, Snapshot, TemporaryDB,
};

/// Possible index names.
const INDEX_NAMES: &[&str] = &[
    "foo",
    "bar",
    "b",
    "overly_long_prefix_still_should_work_though",
];

const MAX_INDEXES: usize = 10;

/// Generates an `IndexAddress` optionally placed in a group.
fn generate_address() -> impl Strategy<Value = IndexAddress> {
    let index_name = sample::select(INDEX_NAMES).prop_map(IndexAddress::from_root);
    prop_oneof![
        // Non-prefixed addresses
        index_name.clone(),
        // Prefixed addresses
        (index_name, 1_u8..8).prop_map(|(addr, prefix)| addr.append_key(&prefix)),
    ]
}

fn generate_entries() -> impl Strategy<Value = Vec<(String, Vec<u8>)>> {
    vec((".{1,16}", vec(num::u8::ANY, 1..8)), 0..20)
}

#[derive(Debug, Clone)]
struct IndexContent {
    addr: IndexAddress,
    ty: IndexType,
    entries: Vec<(String, Vec<u8>)>,
}

impl IndexContent {
    fn generate_vec(max_size: usize) -> impl Strategy<Value = Vec<Self>> {
        let ty = prop_oneof![
            strategy::Just(IndexType::Map),
            strategy::Just(IndexType::ProofMap),
            strategy::Just(IndexType::List),
            strategy::Just(IndexType::ProofList),
            strategy::Just(IndexType::KeySet),
            strategy::Just(IndexType::ValueSet),
        ];

        hash_map(generate_address(), (ty, generate_entries()), 1..max_size).prop_map(|indexes| {
            indexes
                .into_iter()
                .map(|(addr, (ty, entries))| Self { addr, ty, entries })
                .collect()
        })
    }

    fn fill<F>(&self, fork: &F)
    where
        F: Access,
        F::Base: RawAccessMut,
    {
        match self.ty {
            IndexType::Map => {
                let mut map = fork.get_map(self.addr.clone());
                for (key, value) in &self.entries {
                    map.put(key, value.to_owned());
                }
            }
            IndexType::ProofMap => {
                let mut map = fork.get_proof_map(self.addr.clone());
                for (key, value) in &self.entries {
                    map.put(key, value.to_owned());
                }
            }

            IndexType::List => {
                let mut list = fork.get_list(self.addr.clone());
                let values = self.entries.iter().map(|(_, value)| value.to_vec());
                list.extend(values);
            }
            IndexType::ProofList => {
                let mut list = fork.get_proof_list(self.addr.clone());
                let values = self.entries.iter().map(|(_, value)| value.to_vec());
                list.extend(values);
            }

            IndexType::KeySet => {
                let mut set = fork.get_key_set(self.addr.clone());
                for (key, _) in &self.entries {
                    set.insert(key);
                }
            }
            IndexType::ValueSet => {
                let mut set = fork.get_value_set(self.addr.clone());
                for (key, _) in &self.entries {
                    set.insert(key.to_owned());
                }
            }

            _ => unreachable!(),
        }
    }

    fn check<A: Access>(&self, access: &A) -> TestCaseResult {
        match self.ty {
            IndexType::Map => {
                let map = access.get_map::<_, String, Vec<u8>>(self.addr.clone());
                let reference = self
                    .entries
                    .iter()
                    .map(|(k, v)| (k.to_owned(), v.to_vec()))
                    .collect();
                compare_maps(&map, &reference)
            }
            IndexType::ProofMap => {
                let map = access.get_proof_map::<_, String, Vec<u8>>(self.addr.clone());
                let reference = self
                    .entries
                    .iter()
                    .map(|(k, v)| (k.to_owned(), v.to_vec()))
                    .collect();
                compare_maps(&map, &reference)
            }

            IndexType::List => {
                let list = access.get_list::<_, Vec<u8>>(self.addr.clone());
                let reference: Vec<_> = self
                    .entries
                    .iter()
                    .map(|(_, value)| value.to_vec())
                    .collect();
                compare_lists(&list, &reference)
            }
            IndexType::ProofList => {
                let list = access.get_proof_list::<_, Vec<u8>>(self.addr.clone());
                let reference: Vec<_> = self
                    .entries
                    .iter()
                    .map(|(_, value)| value.to_vec())
                    .collect();
                compare_lists(&list, &reference)
            }

            IndexType::KeySet => {
                let set = access.get_key_set::<_, String>(self.addr.clone());
                let reference = self.entries.iter().map(|(key, _)| key.to_owned()).collect();
                compare_sets(&set, &reference)
            }
            IndexType::ValueSet => {
                let set = access.get_value_set::<_, String>(self.addr.clone());
                let reference = self
                    .entries
                    .iter()
                    .map(|(key, _)| (key.object_hash(), key.to_owned()))
                    .collect();
                compare_maps(&set, &reference)
            }

            _ => unreachable!(),
        }
    }
}

fn compare_maps<I>(index: &I, reference: &BTreeMap<I::Key, I::Value>) -> TestCaseResult
where
    I: IndexIterator,
    I::Key: Sized + Ord + Eq + BinaryKey<Owned = I::Key>,
    I::Value: Clone + PartialEq,
{
    let ref_iter = reference
        .iter()
        .map(|(key, value)| (key.to_owned(), value.clone()));
    prop_assert!(index.index_iter(None).eq(ref_iter));

    for start in reference.keys() {
        let ref_iter = reference
            .range(start..)
            .map(|(key, value)| (key.to_owned(), value.clone()));
        prop_assert!(index.index_iter(Some(start)).eq(ref_iter));
    }
    Ok(())
}

fn compare_lists<I>(index: &I, reference: &[Vec<u8>]) -> TestCaseResult
where
    I: IndexIterator<Key = u64, Value = Vec<u8>>,
{
    let ref_iter = reference.iter().map(|value| value.to_vec());
    let ref_iter = (0_u64..).zip(ref_iter);
    prop_assert!(index.index_iter(None).eq(ref_iter));

    let len = reference.len();
    let starts = (0..=len).chain(vec![len * 2, len * 100]);
    for start in starts {
        let ref_iter = reference.iter().skip(start).map(|value| value.to_vec());
        let start = start as u64;
        let ref_iter = (start..).zip(ref_iter);
        prop_assert!(index.index_iter(Some(&start)).eq(ref_iter));
    }

    let large_starts = (10..64)
        .map(|pow| 1_u64 << pow)
        .chain((0..10).map(|diff| u64::max_value() - diff));
    for start in large_starts {
        prop_assert_eq!(index.index_iter(Some(&start)).count(), 0);
    }

    Ok(())
}

fn compare_sets<I>(index: &I, reference: &BTreeSet<String>) -> TestCaseResult
where
    I: IndexIterator<Key = String, Value = ()>,
{
    let ref_iter = reference.iter().map(String::as_str);
    let actual_iter = index.index_iter(None).map(|(key, ())| key);
    prop_assert!(actual_iter.eq(ref_iter));

    let starts = reference.iter();
    for start in starts {
        let ref_iter = reference.range::<String, _>(start..).map(String::as_str);
        let actual_iter = index.index_iter(Some(start)).map(|(key, ())| key);
        prop_assert!(actual_iter.eq(ref_iter));
    }

    Ok(())
}

fn check_iterators(
    db: &TemporaryDB,
    content: Vec<IndexContent>,
    fork_extractor: fn(&Fork) -> ErasedAccess<'_>,
    snapshot_extractor: fn(&dyn Snapshot) -> ErasedAccess<'_>,
) -> TestCaseResult {
    let mut fork = db.fork();
    let fork_access = fork_extractor(&fork);
    for index_content in &content {
        index_content.fill(&fork_access);
    }
    for index_content in &content {
        index_content.check(&fork_access)?;
    }
    fork.flush();
    let fork_access = fork_extractor(&fork);
    for index_content in &content {
        index_content.check(&fork_access)?;
    }

    let patch = fork.into_patch();
    let patch_access = snapshot_extractor(&patch);
    for index_content in &content {
        index_content.check(&patch_access)?;
    }
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    let snapshot_access = snapshot_extractor(&snapshot);
    for index_content in &content {
        index_content.check(&snapshot_access)?;
    }

    Ok(())
}

fn check_iterators_global(db: &TemporaryDB, content: Vec<IndexContent>) -> TestCaseResult {
    check_iterators(
        db,
        content,
        |fork| fork.into_erased(),
        |snapshot| snapshot.into_erased(),
    )
}

fn check_iterators_prefixed(db: &TemporaryDB, content: Vec<IndexContent>) -> TestCaseResult {
    const NAMESPACE: &str = "namespace";
    check_iterators(
        db,
        content,
        |fork| Prefixed::new(NAMESPACE, fork).into_erased(),
        |snapshot| Prefixed::new(NAMESPACE, snapshot).into_erased(),
    )
}

#[test]
fn test_iterators() {
    let db = TemporaryDB::new();
    proptest!(|(content in IndexContent::generate_vec(MAX_INDEXES))| {
        check_iterators_global(&db, content)?;
        db.clear().unwrap();
    });
}

#[test]
fn test_iterators_for_prefixed_indexes() {
    let db = TemporaryDB::new();
    proptest!(|(content in IndexContent::generate_vec(MAX_INDEXES))| {
        check_iterators_prefixed(&db, content)?;
        db.clear().unwrap();
    });
}
