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

//! Property testing for list index as a rust collection.

use exonum::storage::{Database, Fork, ListIndex, MemoryDB, StorageValue};
use modifier::Modifier;
use proptest::{collection::vec, num, strategy, strategy::Strategy, test_runner::TestCaseResult};

use super::{ListAction, ACTIONS_MAX_LEN};

impl<'a, V> Modifier<ListIndex<&'a mut Fork, V>> for ListAction<V>
where
    V: StorageValue,
{
    fn modify(self, list: &mut ListIndex<&mut Fork, V>) {
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
    list_index: &ListIndex<&mut Fork, i32>,
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

proptest! {
    #[test]
    fn proptest_list_index_to_rust_vec(ref actions in vec(generate_action(),
                                                          1..ACTIONS_MAX_LEN)) {
        let db = MemoryDB::new();

        let mut fork = db.fork();
        let mut ref_list: Vec<i32> = Vec::new();

        for action in actions {
            match action {
                ListAction::MergeFork => {
                    db.merge(fork.into_patch()).unwrap();
                    fork = db.fork();
                }
                _ => {
                    let mut list_index = ListIndex::new("test", &mut fork);
                    action.clone().modify(&mut list_index);
                    action.clone().modify(&mut ref_list);
                    compare_collections(&list_index, &ref_list)?;
                }
            }
        }
        db.merge(fork.into_patch()).unwrap();

        let mut fork = db.fork();
        let list_index = ListIndex::new("test", &mut fork);
        compare_collections(&list_index, &ref_list)?;
    }
}
