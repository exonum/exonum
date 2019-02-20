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

// cspell:ignore oneof

//! Property testing for map index and proof map index as a rust collection.
use std::collections::HashMap;
use std::hash::Hash;

use modifier::Modifier;
use proptest::{
    collection::vec, num, prop_assert, prop_assert_eq, prop_oneof, strategy, strategy::Strategy,
    test_runner::TestCaseResult,
};

use exonum_merkledb::{BinaryValue, Fork, MapIndex, ObjectHash, ProofMapIndex};

use crate::common::ACTIONS_MAX_LEN;

mod common;

#[derive(Debug, Clone)]
enum MapAction<K, V> {
    // Should be applied to a small subset of keys (like modulo 8 for int).
    Put(K, V),
    // Should be applied to a small subset of keys (like modulo 8 for int).
    Remove(K),
    Clear,
    MergeFork,
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

mod map_index {
    use super::*;

    impl<'a, V> Modifier<MapIndex<&'a Fork, u8, V>> for MapAction<u8, V>
    where
        V: BinaryValue,
    {
        fn modify(self, map: &mut MapIndex<&'a Fork, u8, V>) {
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

    fn compare_collections(
        map_index: &MapIndex<&Fork, u8, i32>,
        ref_map: &HashMap<u8, i32>,
    ) -> TestCaseResult {
        for k in ref_map.keys() {
            prop_assert!(map_index.contains(k));
        }
        for (k, v) in map_index.iter() {
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

    proptest_compare_collections!(proptest_compare_to_rust_map, MapIndex, HashMap, MapAction);
}

mod proof_map_index {
    use super::*;

    impl<'a, V> Modifier<ProofMapIndex<&'a Fork, [u8; 32], V>> for MapAction<[u8; 32], V>
    where
        V: BinaryValue + ObjectHash,
    {
        fn modify(self, map: &mut ProofMapIndex<&Fork, [u8; 32], V>) {
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

    fn compare_collections(
        map_index: &ProofMapIndex<&Fork, [u8; 32], i32>,
        ref_map: &HashMap<[u8; 32], i32>,
    ) -> TestCaseResult {
        for k in ref_map.keys() {
            prop_assert!(map_index.contains(k));
        }
        for (k, v) in map_index.iter() {
            prop_assert_eq!(Some(&v), ref_map.get(&k));
        }
        Ok(())
    }

    fn generate_action() -> impl Strategy<Value = MapAction<[u8; 32], i32>> {
        prop_oneof![
            ((0..8u8), num::i32::ANY).prop_map(|(i, v)| MapAction::Put([i; 32], v)),
            (0..8u8).prop_map(|i| MapAction::Remove([i; 32])),
            strategy::Just(MapAction::Clear),
            strategy::Just(MapAction::MergeFork),
        ]
    }

    proptest_compare_collections!(
        proptest_compare_to_rust_map,
        ProofMapIndex,
        HashMap,
        MapAction
    );
}
