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

//! Property testing for list index and proof list index as a rust collection.

use modifier::Modifier;
use proptest::{
    collection::vec, num, prop_assert, prop_oneof, strategy, strategy::Strategy,
    test_runner::TestCaseResult,
};

use exonum_merkledb::{BinaryValue, Fork, ListIndex, ObjectHash, ProofListIndex};

use crate::common::ACTIONS_MAX_LEN;

mod common;

#[derive(Debug, Clone)]
enum ListAction<V> {
    Push(V),
    Pop,
    Extend(Vec<V>),
    // Applied with argument modulo `collection.len()`.
    Truncate(u64),
    // Applied to index modulo `collection.len()`.
    Set(u64, V),
    Clear,
    MergeFork,
}

impl<V> Modifier<Vec<V>> for ListAction<V> {
    fn modify(self, list: &mut Vec<V>) {
        match self {
            ListAction::Push(val) => {
                list.push(val);
            }
            ListAction::Pop => {
                list.pop();
            }
            ListAction::Extend(vec) => {
                list.extend(vec);
            }
            ListAction::Truncate(size) => {
                let len = list.len();
                if len > 0 {
                    list.truncate(size as usize % len);
                }
            }
            ListAction::Set(idx, val) => {
                let len = list.len();
                if len > 0 {
                    list[idx as usize % len] = val;
                }
            }
            ListAction::Clear => {
                list.clear();
            }
            _ => unreachable!(),
        }
    }
}

mod list_index {
    use super::*;

    impl<'a, V> Modifier<ListIndex<&'a Fork, V>> for ListAction<V>
    where
        V: BinaryValue,
    {
        fn modify(self, list: &mut ListIndex<&Fork, V>) {
            match self {
                ListAction::Push(val) => {
                    list.push(val);
                }
                ListAction::Pop => {
                    list.pop();
                }
                ListAction::Extend(vec) => {
                    list.extend(vec);
                }
                ListAction::Truncate(size) => {
                    let len = list.len();
                    if len > 0 {
                        list.truncate(size % len);
                    }
                }
                ListAction::Set(idx, val) => {
                    let len = list.len();
                    if len > 0 {
                        list.set(idx % len, val);
                    }
                }
                ListAction::Clear => {
                    list.clear();
                }
                _ => unreachable!(),
            }
        }
    }

    fn compare_collections(
        list_index: &ListIndex<&Fork, i32>,
        ref_list: &Vec<i32>,
    ) -> TestCaseResult {
        prop_assert!(ref_list.iter().map(|v| *v).eq(list_index));
        Ok(())
    }

    fn generate_action() -> impl Strategy<Value = ListAction<i32>> {
        prop_oneof![
            num::i32::ANY.prop_map(ListAction::Push),
            strategy::Just(ListAction::Pop),
            vec(num::i32::ANY, 1..5).prop_map(ListAction::Extend),
            num::u64::ANY.prop_map(ListAction::Truncate),
            (num::u64::ANY, num::i32::ANY).prop_map(|(i, v)| ListAction::Set(i, v)),
            strategy::Just(ListAction::Clear),
            strategy::Just(ListAction::MergeFork),
        ]
    }

    proptest_compare_collections!(proptest_compare_to_rust_vec, ListIndex, Vec, ListAction);
}

mod proof_list_index {
    use super::*;

    impl<'a, V> Modifier<ProofListIndex<&'a Fork, V>> for ListAction<V>
    where
        V: BinaryValue + ObjectHash,
    {
        fn modify(self, list: &mut ProofListIndex<&Fork, V>) {
            match self {
                ListAction::Push(val) => {
                    list.push(val);
                }
                ListAction::Extend(vec) => {
                    list.extend(vec);
                }
                ListAction::Set(idx, val) => {
                    let len = list.len();
                    if len > 0 {
                        list.set(idx % len, val);
                    }
                }
                ListAction::Clear => {
                    list.clear();
                }
                _ => unreachable!(),
            }
        }
    }

    fn compare_collections(
        list_index: &ProofListIndex<&Fork, i32>,
        ref_list: &Vec<i32>,
    ) -> TestCaseResult {
        prop_assert!(ref_list.iter().map(|v| *v).eq(list_index));
        Ok(())
    }

    fn generate_action() -> impl Strategy<Value = ListAction<i32>> {
        prop_oneof![
            num::i32::ANY.prop_map(ListAction::Push),
            vec(num::i32::ANY, 1..5).prop_map(ListAction::Extend),
            (num::u64::ANY, num::i32::ANY).prop_map(|(i, v)| ListAction::Set(i, v)),
            strategy::Just(ListAction::Clear),
            strategy::Just(ListAction::MergeFork),
        ]
    }

    proptest_compare_collections!(
        proptest_compare_to_rust_vec,
        ProofListIndex,
        Vec,
        ListAction
    );
}
