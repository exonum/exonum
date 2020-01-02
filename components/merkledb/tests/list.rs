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

#![allow(clippy::ptr_arg)] // Usage of `&Vec<_>` is needed for type inference

// cspell:ignore oneof

//! Property testing for list index and proof list index as a rust collection.

use modifier::Modifier;
use proptest::{
    collection::vec, num, prop_assert, prop_assert_eq, prop_oneof, proptest, strategy,
    strategy::Strategy, test_runner::TestCaseResult,
};

use std::rc::Rc;

use exonum_merkledb::{
    access::AccessExt, BinaryValue, Fork, HashTag, ListIndex, ObjectHash, ProofListIndex,
    TemporaryDB,
};

mod common;
use crate::common::{compare_collections, AsForkAction, ForkAction, FromFork, ACTIONS_MAX_LEN};

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
    FlushFork,
    MergeFork,
}

impl<V> AsForkAction for ListAction<V> {
    fn as_fork_action(&self) -> Option<ForkAction> {
        match self {
            ListAction::FlushFork => Some(ForkAction::Flush),
            ListAction::MergeFork => Some(ForkAction::Merge),
            _ => None,
        }
    }
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

impl<V: BinaryValue> Modifier<ListIndex<Rc<Fork>, V>> for ListAction<V> {
    fn modify(self, list: &mut ListIndex<Rc<Fork>, V>) {
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

impl<V> Modifier<ProofListIndex<Rc<Fork>, V>> for ListAction<V>
where
    V: BinaryValue,
{
    fn modify(self, list: &mut ProofListIndex<Rc<Fork>, V>) {
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
            ListAction::Set(idx, val) => {
                let len = list.len();
                if len > 0 {
                    list.set(idx % len, val);
                }
            }
            ListAction::Clear => {
                list.clear();
            }
            ListAction::Truncate(size) => {
                let len = list.len();
                if len > 0 {
                    list.truncate(size % len);
                }
            }
            _ => unreachable!(),
        }
    }
}

impl<V: BinaryValue> FromFork for ListIndex<Rc<Fork>, V> {
    fn from_fork(fork: Rc<Fork>) -> Self {
        fork.get_list("test")
    }

    fn clear(&mut self) {
        self.clear();
    }
}

impl<V: BinaryValue> FromFork for ProofListIndex<Rc<Fork>, V> {
    fn from_fork(fork: Rc<Fork>) -> Self {
        fork.get_proof_list("test")
    }

    fn clear(&mut self) {
        self.clear();
    }
}

fn generate_action() -> impl Strategy<Value = ListAction<i32>> {
    prop_oneof![
        num::i32::ANY.prop_map(ListAction::Push),
        strategy::Just(ListAction::Pop),
        vec(num::i32::ANY, 1..5).prop_map(ListAction::Extend),
        num::u64::ANY.prop_map(ListAction::Truncate),
        (num::u64::ANY, num::i32::ANY).prop_map(|(i, v)| ListAction::Set(i, v)),
        strategy::Just(ListAction::Clear),
        strategy::Just(ListAction::FlushFork),
        strategy::Just(ListAction::MergeFork),
    ]
}

fn compare_list(list: &ListIndex<Rc<Fork>, i32>, ref_list: &Vec<i32>) -> TestCaseResult {
    prop_assert!(ref_list.iter().copied().eq(list));
    Ok(())
}

fn compare_proof_list(list: &ProofListIndex<Rc<Fork>, i32>, ref_list: &Vec<i32>) -> TestCaseResult {
    prop_assert!(ref_list.iter().copied().eq(list));
    Ok(())
}

/// Checking equality of the list hash together with validity of all proofs allows to be reasonably
/// sure that all branches in the Merkle tree are updated properly.
fn check_list_proofs(list: &ProofListIndex<Rc<Fork>, i32>, ref_list: &Vec<i32>) -> TestCaseResult {
    let expected_hash = HashTag::hash_list(ref_list);
    prop_assert_eq!(list.object_hash(), expected_hash);
    for i in 0..list.len() {
        let proof = list.get_proof(i);
        prop_assert!(proof.check_against_hash(list.object_hash()).is_ok());
    }
    Ok(())
}

#[test]
fn compare_list_to_vec() {
    let db = TemporaryDB::new();
    proptest!(|(ref actions in vec(generate_action(), 1..ACTIONS_MAX_LEN))| {
        compare_collections(&db, actions, compare_list)?;
    });
}

#[test]
fn compare_proof_list_to_vec() {
    let db = TemporaryDB::new();
    proptest!(|(ref actions in vec(generate_action(), 1..ACTIONS_MAX_LEN))| {
        compare_collections(&db, actions, compare_proof_list)?;
    });
}

#[test]
fn check_proofs_for_proof_list() {
    let db = TemporaryDB::new();
    proptest!(|(ref actions in vec(generate_action(), 1..ACTIONS_MAX_LEN))| {
        compare_collections(&db, actions, check_list_proofs)?;
    });
}
