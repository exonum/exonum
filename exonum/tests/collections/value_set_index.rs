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

//! Property testing for value set index.

use exonum::storage::{Database, Fork, MemoryDB, ValueSetIndex};
use modifier::Modifier;
use proptest::{collection::vec, num, strategy, strategy::Strategy, test_runner::TestCaseResult};

use std::collections::HashSet;

use super::{SetAction, ACTIONS_MAX_LEN};
use SetAction::MergeFork;

impl<'a> Modifier<ValueSetIndex<&'a mut Fork, u8>> for SetAction<u8> where {
    fn modify(self, set: &mut ValueSetIndex<&'a mut Fork, u8>) {
        match self {
            SetAction::Put(mut k) => {
                k = k % 8;
                set.insert(k);
            }
            SetAction::Remove(mut k) => {
                k = k % 8;
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
    value_set_index: &ValueSetIndex<&mut Fork, u8>,
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

fn generate_action() -> impl Strategy<Value = SetAction<u8>> {
    prop_oneof![
        num::u8::ANY.prop_map(SetAction::Put),
        num::u8::ANY.prop_map(SetAction::Remove),
        strategy::Just(SetAction::Clear),
        strategy::Just(SetAction::MergeFork),
    ]
}

proptest_compare_collections!(proptest_compare_to_rust_set, ValueSetIndex, HashSet);
