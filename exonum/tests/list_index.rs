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

extern crate exonum;
#[macro_use]
extern crate proptest;

use exonum::storage::{Database, Fork, ListIndex, MemoryDB};
use proptest::{collection::vec, num, prelude::*, strategy};

#[derive(Debug, Clone)]
enum Action {
    Push(i32),
    Pop,
    Extend(Vec<i32>),
    //Applied with argument modulo collection.len()
    Truncate(u64),
    //Applied to index modulo collection.len()
    Set(u64, i32),
    Clear,
    MergeFork,
}

impl Action {
    fn apply(&self, list: &mut ListIndex<&mut Fork, i32>, ref_list: &mut Vec<i32>) {
        match *self {
            Action::Push(val) => {
                list.push(val);
                ref_list.push(val);
            }
            Action::Pop => {
                list.pop();
                ref_list.pop();
            }
            Action::Extend(ref vec) => {
                list.extend(vec.clone());
                ref_list.extend(vec.clone());
            }
            Action::Truncate(size) => {
                let len = list.len();
                if len > 0 {
                    list.truncate(size % len);
                }
                let len = ref_list.len();
                if len > 0 {
                    ref_list.truncate(size as usize % len);
                }
            }
            Action::Set(idx, val) => {
                let len = list.len();
                if len > 0 {
                    list.set(idx % len, val);
                }
                let len = ref_list.len();
                if len > 0 {
                    ref_list[idx as usize % len] = val;
                }
            }
            Action::Clear => {
                list.clear();
                ref_list.clear();
            }
            _ => unreachable!(),
        }
    }
}

proptest!{
    #[test]
    fn proptest_list_index_to_rust_vec(ref actions in
                     vec( prop_oneof![
                         num::i32::ANY.prop_map(Action::Push),
                         strategy::Just(Action::Pop),
                         vec( num::i32::ANY, 1..5 ).prop_map(Action::Extend),
                         num::u64::ANY.prop_map(Action::Truncate),
                         (num::u64::ANY, num::i32::ANY).prop_map(|(i, v)| Action::Set(i,v)),
                         strategy::Just(Action::Clear),
                         strategy::Just(Action::MergeFork),
                     ] , 1..10) ) {
        let db = MemoryDB::new();

        let mut fork = db.fork();
        let mut ref_list : Vec<i32> = Vec::new();

        for action in actions {
            match action {
                Action::MergeFork => {
                    db.merge(fork.into_patch()).unwrap();
                    fork = db.fork();
                },
                _ => {
                    let mut list = ListIndex::<_, i32>::new("test", &mut fork);
                    action.apply(&mut list, &mut ref_list);
                }
            }
        }
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let list_index = ListIndex::<_, i32>::new("test", &snapshot);

        prop_assert_eq!(list_index.len() as usize, ref_list.len());

        for (&l,r) in ref_list.iter().zip(list_index.iter()) {
            prop_assert_eq!(l, r);
        }
    }
}
