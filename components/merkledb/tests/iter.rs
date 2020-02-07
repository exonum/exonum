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
//!
//! The test generates several indexes with their contents, and adds this content
//! in two stages (in order to test all kinds of access during iteration: from the fork,
//! flushed fork and snapshot). After each stage, the iterator tests are run for several kinds
//! of access:
//!
//! - non-flushed `Fork`
//! - flushed `Fork`
//! - `Patch`
//! - `Box<dyn Snapshot>`
//!
//! Each access kind is tested in the raw variation and within a `Prefixed` access.

use proptest::{
    collection::{hash_map, vec},
    num, prop_assert, prop_assert_eq, prop_oneof, proptest, sample, strategy,
    strategy::Strategy,
    test_runner::TestCaseResult,
};

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
};

use exonum_merkledb::{
    access::{Access, AccessExt, Prefixed, RawAccessMut},
    generic::{ErasedAccess, IntoErased},
    indexes::IndexIterator,
    BinaryKey, BinaryValue, Database, Fork, IndexAddress, IndexType, ObjectHash, Snapshot,
    TemporaryDB,
};

/// Possible index names.
const INDEX_NAMES: &[&str] = &["foo", "bar", "b"];
/// Maximum number of generated indexes.
const MAX_INDEXES: usize = 5;
/// Maximum number of generated entries within the index.
const MAX_ENTRIES: usize = 8;

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

/// Contents of an index.
#[derive(Debug, Clone)]
struct IndexContent<K, V> {
    addr: IndexAddress,
    ty: IndexType,
    /// Initial index entries.
    entries: Vec<(K, V)>,
    /// Entries to add later (keys may coincide with keys in `entries`).
    more_entries: Vec<(K, V)>,
    /// Should index entries be extended on the next call to `fill`?
    extend_entries: bool,
}

impl<K, V> IndexContent<K, V>
where
    K: Clone + Eq + Ord + BinaryKey<Owned = K> + ObjectHash + Debug,
    V: Clone + PartialEq + BinaryValue + ObjectHash + Debug,
{
    /// Generates a vector of indexes with randomized entries. Addresses of indexes are guaranteed
    /// to be different.
    fn generate_vec(
        entries: impl Strategy<Value = Vec<(K, V)>> + Clone,
        max_size: usize,
    ) -> impl Strategy<Value = Vec<Self>> {
        let ty = prop_oneof![
            strategy::Just(IndexType::Map),
            strategy::Just(IndexType::ProofMap),
            strategy::Just(IndexType::List),
            strategy::Just(IndexType::ProofList),
            strategy::Just(IndexType::SparseList),
            strategy::Just(IndexType::KeySet),
            strategy::Just(IndexType::ValueSet),
        ];

        hash_map(
            generate_address(),
            (ty, entries.clone(), entries),
            1..=max_size,
        )
        .prop_map(|indexes| {
            indexes
                .into_iter()
                .map(|(addr, (ty, entries, more_entries))| Self {
                    addr,
                    ty,
                    entries,
                    more_entries,
                    extend_entries: false,
                })
                .collect()
        })
    }

    fn key_to_index(key: &K) -> u64 {
        let key_hash = key.object_hash();
        let mut index_bytes = [0_u8; 8];
        index_bytes.copy_from_slice(&key_hash[..8]);
        u64::from_le_bytes(index_bytes)
    }

    fn reference(&self) -> BTreeMap<K, &V> {
        self.entries
            .iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect()
    }

    fn more_keys(&self) -> impl Iterator<Item = K> + '_ {
        self.more_entries.iter().map(|(key, _)| key.to_owned())
    }

    fn fill<F>(&mut self, fork: &F)
    where
        F: Access,
        F::Base: RawAccessMut,
    {
        let entries = if self.extend_entries {
            &self.more_entries
        } else {
            &self.entries
        };

        match self.ty {
            IndexType::Map => {
                let mut map = fork.get_map(self.addr.clone());
                for (key, value) in entries {
                    map.put(key, value.to_owned());
                }
            }
            IndexType::ProofMap => {
                let mut map = fork.get_proof_map(self.addr.clone());
                for (key, value) in entries {
                    map.put(key, value.to_owned());
                }
            }

            IndexType::List => {
                let mut list = fork.get_list(self.addr.clone());
                let values = entries.iter().map(|(_, value)| value.to_owned());
                list.extend(values);
            }
            IndexType::ProofList => {
                let mut list = fork.get_proof_list(self.addr.clone());
                let values = entries.iter().map(|(_, value)| value.to_owned());
                list.extend(values);
            }
            IndexType::SparseList => {
                let mut list = fork.get_sparse_list(self.addr.clone());
                for (key, value) in entries {
                    let index = Self::key_to_index(key);
                    list.set(index, value.to_owned());
                }
            }

            IndexType::KeySet => {
                let mut set = fork.get_key_set(self.addr.clone());
                for (key, _) in entries {
                    set.insert(key);
                }
            }
            IndexType::ValueSet => {
                let mut set = fork.get_value_set(self.addr.clone());
                for (_, value) in entries {
                    set.insert(value.to_owned());
                }
            }

            _ => unreachable!(),
        }

        if self.extend_entries {
            self.entries.extend(self.more_entries.drain(..));
        }
    }

    fn check<A: Access>(&self, access: &A) -> TestCaseResult {
        match self.ty {
            IndexType::Map => {
                let map = access.get_map::<_, K, V>(self.addr.clone());
                let reference = self.reference();
                compare_maps(&map, &reference, self.more_keys())
            }
            IndexType::ProofMap => {
                let map = access.get_proof_map::<_, K, V>(self.addr.clone());
                let reference = self.reference();
                compare_maps(&map, &reference, self.more_keys())
            }

            IndexType::List => {
                let list = access.get_list::<_, V>(self.addr.clone());
                let reference: Vec<_> = self.entries.iter().map(|(_, value)| value).collect();
                compare_lists(&list, &reference)
            }
            IndexType::ProofList => {
                let list = access.get_proof_list::<_, V>(self.addr.clone());
                let reference: Vec<_> = self.entries.iter().map(|(_, value)| value).collect();
                compare_lists(&list, &reference)
            }
            IndexType::SparseList => {
                let list = access.get_sparse_list::<_, V>(self.addr.clone());
                let reference = self
                    .entries
                    .iter()
                    .map(|(key, value)| (Self::key_to_index(key), value))
                    .collect();
                let more_indexes = self.entries.iter().map(|(key, _)| Self::key_to_index(key));
                compare_maps(&list, &reference, more_indexes)
            }

            IndexType::KeySet => {
                let set = access.get_key_set::<_, K>(self.addr.clone());
                let reference = self.entries.iter().map(|(key, _)| key.to_owned()).collect();
                compare_sets(&set, &reference, self.more_keys())
            }
            IndexType::ValueSet => {
                let set = access.get_value_set::<_, V>(self.addr.clone());
                let reference = self
                    .entries
                    .iter()
                    .map(|(_, value)| (value.object_hash(), value))
                    .collect();
                let more_keys = self.more_keys().map(|key| key.object_hash());
                compare_maps(&set, &reference, more_keys)
            }

            _ => unreachable!(),
        }
    }
}

fn compare_maps<I>(
    index: &I,
    reference: &BTreeMap<I::Key, &I::Value>,
    more_keys: impl Iterator<Item = I::Key>,
) -> TestCaseResult
where
    I: IndexIterator,
    I::Key: Sized + Ord + Eq + BinaryKey<Owned = I::Key>,
    I::Value: Clone + PartialEq,
{
    let ref_iter = reference
        .iter()
        .map(|(key, &value)| (key.to_owned(), value.to_owned()));
    prop_assert!(index.index_iter(None).eq(ref_iter));

    let starts = reference
        .keys()
        .map(Cow::Borrowed)
        .chain(more_keys.map(Cow::Owned));
    for start in starts {
        let start = start.as_ref();
        let ref_iter = reference
            .range(start..)
            .map(|(key, &value)| (key.to_owned(), value.to_owned()));
        prop_assert!(index.index_iter(Some(start)).eq(ref_iter));
    }
    Ok(())
}

fn compare_lists<I>(index: &I, reference: &[&I::Value]) -> TestCaseResult
where
    I: IndexIterator<Key = u64>,
    I::Value: PartialEq + Clone,
{
    let ref_iter = reference.iter().copied().map(Clone::clone);
    let ref_iter = (0_u64..).zip(ref_iter);
    prop_assert!(index.index_iter(None).eq(ref_iter));

    let len = reference.len();
    let starts = (0..=len).chain(vec![len * 2, len * 100]);
    for start in starts {
        let ref_iter = reference.iter().copied().skip(start).map(Clone::clone);
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

fn compare_sets<I>(
    index: &I,
    reference: &BTreeSet<I::Key>,
    more_keys: impl Iterator<Item = I::Key>,
) -> TestCaseResult
where
    I: IndexIterator<Value = ()>,
    I::Key: Sized + Ord + Eq + ToOwned<Owned = I::Key>,
{
    let ref_iter = reference.iter().map(ToOwned::to_owned);
    let actual_iter = index.index_iter(None).map(|(key, ())| key);
    prop_assert!(actual_iter.eq(ref_iter));

    let starts = reference
        .iter()
        .map(Cow::Borrowed)
        .chain(more_keys.map(Cow::Owned));
    for start in starts {
        let start = start.as_ref();
        let ref_iter = reference.range(start..).map(ToOwned::to_owned);
        let actual_iter = index.index_iter(Some(start)).map(|(key, ())| key);
        prop_assert!(actual_iter.eq(ref_iter));
    }

    Ok(())
}

fn check_iterators_run<K, V>(
    db: &TemporaryDB,
    content: &mut [IndexContent<K, V>],
    fork_extractor: fn(&Fork) -> ErasedAccess<'_>,
    snapshot_extractor: fn(&dyn Snapshot) -> ErasedAccess<'_>,
) -> TestCaseResult
where
    K: Clone + Eq + Ord + BinaryKey<Owned = K> + ObjectHash + Debug,
    V: Clone + PartialEq + BinaryValue + ObjectHash + Debug,
{
    let mut fork = db.fork();
    let fork_access = fork_extractor(&fork);
    for index_content in &mut *content {
        index_content.fill(&fork_access);
    }
    for index_content in &*content {
        index_content.check(&fork_access)?;
    }
    fork.flush();
    let fork_access = fork_extractor(&fork);
    for index_content in &*content {
        index_content.check(&fork_access)?;
    }

    let patch = fork.into_patch();
    let patch_access = snapshot_extractor(&patch);
    for index_content in &*content {
        index_content.check(&patch_access)?;
    }
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    let snapshot_access = snapshot_extractor(&snapshot);
    for index_content in &*content {
        index_content.check(&snapshot_access)?;
    }

    Ok(())
}

fn check_iterators<K, V>(
    db: &TemporaryDB,
    mut content: Vec<IndexContent<K, V>>,
    fork_extractor: fn(&Fork) -> ErasedAccess<'_>,
    snapshot_extractor: fn(&dyn Snapshot) -> ErasedAccess<'_>,
) -> TestCaseResult
where
    K: Clone + Eq + Ord + BinaryKey<Owned = K> + ObjectHash + Debug,
    V: Clone + PartialEq + BinaryValue + ObjectHash + Debug,
{
    check_iterators_run(db, &mut content, fork_extractor, snapshot_extractor)?;

    for index in &mut content {
        index.extend_entries = true;
    }
    check_iterators_run(db, &mut content, fork_extractor, snapshot_extractor)
}

fn check_iterators_global<K, V>(
    db: &TemporaryDB,
    content: Vec<IndexContent<K, V>>,
) -> TestCaseResult
where
    K: Clone + Eq + Ord + BinaryKey<Owned = K> + ObjectHash + Debug,
    V: Clone + PartialEq + BinaryValue + ObjectHash + Debug,
{
    check_iterators(
        db,
        content,
        |fork| fork.into_erased(),
        |snapshot| snapshot.into_erased(),
    )
}

fn check_iterators_prefixed<K, V>(
    db: &TemporaryDB,
    content: Vec<IndexContent<K, V>>,
) -> TestCaseResult
where
    K: Clone + Eq + Ord + BinaryKey<Owned = K> + ObjectHash + Debug,
    V: Clone + PartialEq + BinaryValue + ObjectHash + Debug,
{
    const NAMESPACE: &str = "namespace";
    check_iterators(
        db,
        content,
        |fork| Prefixed::new(NAMESPACE, fork).into_erased(),
        |snapshot| Prefixed::new(NAMESPACE, snapshot).into_erased(),
    )
}

fn test_iterators<K, V>(
    entries: impl Strategy<Value = Vec<(K, V)>> + Clone,
    check: fn(&TemporaryDB, content: Vec<IndexContent<K, V>>) -> TestCaseResult,
) where
    K: Clone + Eq + Ord + BinaryKey<Owned = K> + ObjectHash + Debug,
    V: Clone + PartialEq + BinaryValue + ObjectHash + Debug,
{
    let db = TemporaryDB::new();
    let content_strategy = IndexContent::generate_vec(entries, MAX_INDEXES);
    proptest!(|(content in content_strategy)| {
        let result = check(&db, content);
        // Clear database in any case; otherwise, side-effect errors will prevent determining
        // the true error cause.
        db.clear().unwrap();
        result?;
    });
}

mod string_vec_entries {
    use super::*;

    fn generate_entries() -> impl Strategy<Value = Vec<(String, Vec<u8>)>> + Clone {
        let value = vec(num::u8::ANY, 1..8);
        vec(("[A-Za-z]{1,8}", value), 0..=MAX_ENTRIES)
    }

    #[test]
    fn iterators() {
        test_iterators(generate_entries(), check_iterators_global);
    }

    #[test]
    fn iterators_for_prefixed_indexes() {
        test_iterators(generate_entries(), check_iterators_prefixed);
    }
}

mod u8_string_entries {
    use super::*;

    fn generate_entries() -> impl Strategy<Value = Vec<(u8, String)>> + Clone {
        vec((num::u8::ANY, "[0-9]{8}"), 0..=MAX_ENTRIES)
    }

    #[test]
    fn iterators() {
        test_iterators(generate_entries(), check_iterators_global);
    }

    #[test]
    fn iterators_for_prefixed_indexes() {
        test_iterators(generate_entries(), check_iterators_prefixed);
    }
}
