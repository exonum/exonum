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

//! Property testing for key set index and value set index as a rust collection.

mod common;

use modifier::Modifier;
use proptest::{
    collection::vec, prop_assert, prop_oneof, strategy, strategy::Strategy,
    test_runner::TestCaseResult,
};

use std::collections::HashSet;
use std::hash::Hash;

use crate::common::ACTIONS_MAX_LEN;
use exonum_merkledb::{Fork, KeySetIndex, ValueSetIndex};

#[derive(Debug, Clone)]
enum SetAction<V> {
    // Should be applied to a small subset of values (like modulo 8 for int).
    Put(V),
    // Should be applied to a small subset of values (like modulo 8 for int).
    Remove(V),
    Clear,
    MergeFork,
}

fn generate_action() -> impl Strategy<Value = SetAction<u8>> {
    prop_oneof![
        (0..8u8).prop_map(SetAction::Put),
        (0..8u8).prop_map(SetAction::Remove),
        strategy::Just(SetAction::Clear),
        strategy::Just(SetAction::MergeFork),
    ]
}

impl<V> Modifier<HashSet<V>> for SetAction<V>
where
    V: Eq + Hash,
{
    fn modify(self, set: &mut HashSet<V>) {
        match self {
            SetAction::Put(v) => {
                set.insert(v);
            }
            SetAction::Remove(v) => {
                set.remove(&v);
            }
            SetAction::Clear => set.clear(),
            _ => unreachable!(),
        }
    }
}

mod key_set_index {
    use super::*;

    impl<'a> Modifier<KeySetIndex<&'a Fork, u8>> for SetAction<u8> {
        fn modify(self, set: &mut KeySetIndex<&'a Fork, u8>) {
            match self {
                SetAction::Put(k) => {
                    set.insert(k);
                }
                SetAction::Remove(k) => {
                    set.remove(&k);
                }
                SetAction::Clear => {
                    set.clear();
                }
                _ => unreachable!(),
            }
        }
    }

    fn compare_collections(
        key_set_index: &KeySetIndex<&Fork, u8>,
        ref_set: &HashSet<u8>,
    ) -> TestCaseResult {
        for k in ref_set {
            prop_assert!(key_set_index.contains(k));
        }
        for k in key_set_index.iter() {
            prop_assert!(ref_set.contains(&k));
        }
        Ok(())
    }

    proptest_compare_collections!(
        proptest_compare_to_rust_set,
        KeySetIndex,
        HashSet,
        SetAction
    );
}

mod value_set_index {
    use super::*;

    impl<'a> Modifier<ValueSetIndex<&'a Fork, u8>> for SetAction<u8> {
        fn modify(self, set: &mut ValueSetIndex<&'a Fork, u8>) {
            match self {
                SetAction::Put(k) => {
                    set.insert(k);
                }
                SetAction::Remove(k) => {
                    set.remove(&k);
                }
                SetAction::Clear => {
                    set.clear();
                }
                _ => unreachable!(),
            }
        }
    }

    fn compare_collections(
        value_set_index: &ValueSetIndex<&Fork, u8>,
        ref_set: &HashSet<u8>,
    ) -> TestCaseResult {
        for k in ref_set {
            prop_assert!(value_set_index.contains(k));
        }
        for (_, k) in value_set_index.iter() {
            prop_assert!(ref_set.contains(&k));
        }
        Ok(())
    }

    proptest_compare_collections!(
        proptest_compare_to_rust_set,
        ValueSetIndex,
        HashSet,
        SetAction
    );
}
