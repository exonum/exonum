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

//! Property testing for map index and proof map index as a rust collection.

// cspell:ignore oneof

use modifier::Modifier;
use proptest::{
    collection::vec, num, prop_assert, prop_assert_eq, prop_oneof, proptest, strategy,
    strategy::Strategy, test_runner::TestCaseResult,
};

use std::{collections::HashMap, hash::Hash, rc::Rc};

use exonum_merkledb::{
    access::AccessExt, BinaryValue, Fork, HashTag, MapIndex, ObjectHash, ProofMapIndex, TemporaryDB,
};

use crate::{
    common::{compare_collections, AsForkAction, ForkAction, FromFork, ACTIONS_MAX_LEN},
    key::Key,
};

mod common;
mod key;

#[derive(Debug, Clone)]
enum MapAction<K, V> {
    // Should be applied to a small subset of keys (like modulo 8 for int).
    Put(K, V),
    // Should be applied to a small subset of keys (like modulo 8 for int).
    Remove(K),
    Clear,
    FlushFork,
    MergeFork,
}

impl<K, V> AsForkAction for MapAction<K, V> {
    fn as_fork_action(&self) -> Option<ForkAction> {
        match self {
            MapAction::FlushFork => Some(ForkAction::Flush),
            MapAction::MergeFork => Some(ForkAction::Merge),
            _ => None,
        }
    }
}

impl<K, V> Modifier<HashMap<K, V>> for MapAction<K, V>
where
    K: Eq + Hash,
{
    fn modify(self, map: &mut HashMap<K, V>) {
        match self {
            MapAction::Put(k, v) => {
                map.insert(k, v);
            }
            MapAction::Remove(k) => {
                map.remove(&k);
            }
            MapAction::Clear => {
                map.clear();
            }
            _ => unreachable!(),
        }
    }
}

impl<V> Modifier<MapIndex<Rc<Fork>, u8, V>> for MapAction<u8, V>
where
    V: BinaryValue,
{
    fn modify(self, map: &mut MapIndex<Rc<Fork>, u8, V>) {
        match self {
            MapAction::Put(k, v) => {
                map.put(&k, v);
            }
            MapAction::Remove(k) => {
                map.remove(&k);
            }
            MapAction::Clear => {
                map.clear();
            }
            _ => unreachable!(),
        }
    }
}

impl<V> Modifier<ProofMapIndex<Rc<Fork>, Key, V>> for MapAction<Key, V>
where
    V: BinaryValue + ObjectHash,
{
    fn modify(self, map: &mut ProofMapIndex<Rc<Fork>, Key, V>) {
        match self {
            MapAction::Put(k, v) => {
                map.put(&k, v);
            }
            MapAction::Remove(k) => {
                map.remove(&k);
            }
            MapAction::Clear => {
                map.clear();
                assert_eq!(map.object_hash(), HashTag::empty_map_hash());
            }
            _ => unreachable!(),
        }
    }
}

impl<V: BinaryValue> FromFork for MapIndex<Rc<Fork>, u8, V> {
    fn from_fork(fork: Rc<Fork>) -> Self {
        fork.get_map("test")
    }

    fn clear(&mut self) {
        self.clear();
    }
}

impl<V: BinaryValue + ObjectHash> FromFork for ProofMapIndex<Rc<Fork>, Key, V> {
    fn from_fork(fork: Rc<Fork>) -> Self {
        fork.get_proof_map("test")
    }

    fn clear(&mut self) {
        self.clear();
    }
}

fn compare_map(map: &MapIndex<Rc<Fork>, u8, i32>, ref_map: &HashMap<u8, i32>) -> TestCaseResult {
    for k in ref_map.keys() {
        prop_assert!(map.contains(k));
    }
    for (k, v) in map.iter() {
        prop_assert_eq!(Some(&v), ref_map.get(&k));
    }
    Ok(())
}

fn compare_proof_map(
    map: &ProofMapIndex<Rc<Fork>, Key, i32>,
    ref_map: &HashMap<Key, i32>,
) -> TestCaseResult {
    for k in ref_map.keys() {
        prop_assert!(map.contains(k));
    }
    for (k, v) in map.iter() {
        prop_assert_eq!(Some(&v), ref_map.get(&k));
    }
    Ok(())
}

fn generate_action() -> impl Strategy<Value = MapAction<u8, i32>> {
    prop_oneof![
        (num::u8::ANY, num::i32::ANY).prop_map(|(i, v)| MapAction::Put(i, v)),
        num::u8::ANY.prop_map(MapAction::Remove),
        strategy::Just(MapAction::Clear),
        strategy::Just(MapAction::MergeFork),
    ]
}

fn generate_proof_action() -> impl Strategy<Value = MapAction<Key, i32>> {
    prop_oneof![
        ((0..8u8), num::i32::ANY).prop_map(|(i, v)| MapAction::Put([i; 32].into(), v)),
        (0..8u8).prop_map(|i| MapAction::Remove([i; 32].into())),
        strategy::Just(MapAction::Clear),
        strategy::Just(MapAction::FlushFork),
        strategy::Just(MapAction::MergeFork),
    ]
}

#[test]
fn compare_map_to_hash_map() {
    let db = TemporaryDB::new();
    proptest!(|(ref actions in vec(generate_action(), 1..ACTIONS_MAX_LEN))| {
        compare_collections(&db, actions, compare_map)?;
    });
}

#[test]
fn compare_proof_map_to_hash_map() {
    let db = TemporaryDB::new();
    proptest!(|(ref actions in vec(generate_proof_action(), 1..ACTIONS_MAX_LEN))| {
        compare_collections(&db, actions, compare_proof_map)?;
    });
}
