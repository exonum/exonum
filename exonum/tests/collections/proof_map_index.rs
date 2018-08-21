// Copyright 2018 The Exonum Team
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

//! Property testing for proof map index as a rust collection.

use exonum::storage::{Database, Fork, MemoryDB, ProofMapIndex, StorageValue};
use modifier::Modifier;
use proptest::{collection::vec, num, strategy, strategy::Strategy, test_runner::TestCaseResult};

use std::collections::HashMap;

use super::{MapAction, ACTIONS_MAX_LEN};

impl<'a, V> Modifier<ProofMapIndex<&'a mut Fork, [u8; 32], V>> for MapAction<[u8; 32], V>
where
    V: StorageValue,
{
    fn modify(self, map: &mut ProofMapIndex<&mut Fork, [u8; 32], V>) {
        match self {
            MapAction::Put(mut k, v) => {
                k[0] = k[0] % 8;
                map.put(&k, v);
            }
            MapAction::Remove(mut k) => {
                k[0] = k[0] % 8;
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
    map_index: &ProofMapIndex<&mut Fork, [u8; 32], i32>,
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
        (num::u8::ANY, num::i32::ANY).prop_map(|(i, v)| {
            let mut key = [0u8; 32];
            key[0] = i;
            MapAction::Put(key, v)
        }),
        num::u8::ANY.prop_map(|i| {
            let mut key = [0u8; 32];
            key[0] = i;
            MapAction::Remove(key)
        }),
        strategy::Just(MapAction::Clear),
        strategy::Just(MapAction::MergeFork),
    ]
}

proptest! {
    #[test]
    fn proptest_proof_map_index_to_rust_map(ref actions in vec(generate_action(),
                                                               1..ACTIONS_MAX_LEN) ) {
        let db = MemoryDB::new();

        let mut fork = db.fork();
        let mut ref_map: HashMap<[u8; 32], i32> = HashMap::new();

        for action in actions {
            match action {
                MapAction::MergeFork => {
                    db.merge(fork.into_patch()).unwrap();
                    fork = db.fork();
                }
                _ => {
                    let mut map_index = ProofMapIndex::new("test", &mut fork);
                    action.clone().modify(&mut map_index);
                    action.clone().modify(&mut ref_map);
                    compare_collections(&map_index, &ref_map)?;
                }
            }
        }
        db.merge(fork.into_patch()).unwrap();

        let mut fork = db.fork();
        let map_index = ProofMapIndex::new("test", &mut fork);
        compare_collections(&map_index, &ref_map)?;
    }
}
