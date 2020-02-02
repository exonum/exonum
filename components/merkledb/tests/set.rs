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

// cspell:ignore oneof

//! Property testing for key set index and value set index as a rust collection.

use modifier::Modifier;
use proptest::{
    collection::vec, prop_assert, prop_oneof, proptest, strategy, strategy::Strategy,
    test_runner::TestCaseResult,
};

use std::{collections::HashSet, hash::Hash, rc::Rc};

use exonum_merkledb::{access::AccessExt, Fork, KeySetIndex, TemporaryDB, ValueSetIndex};

mod common;
use crate::common::{compare_collections, AsForkAction, ForkAction, FromFork, ACTIONS_MAX_LEN};

#[derive(Debug, Clone)]
enum SetAction<V> {
    // Should be applied to a small subset of values (like modulo 8 for int).
    Put(V),
    // Should be applied to a small subset of values (like modulo 8 for int).
    Remove(V),
    Clear,
    FlushFork,
    MergeFork,
}

impl<V> AsForkAction for SetAction<V> {
    fn as_fork_action(&self) -> Option<ForkAction> {
        match self {
            SetAction::FlushFork => Some(ForkAction::Flush),
            SetAction::MergeFork => Some(ForkAction::Merge),
            _ => None,
        }
    }
}

fn generate_action() -> impl Strategy<Value = SetAction<u8>> {
    prop_oneof![
        (0..8u8).prop_map(SetAction::Put),
        (0..8u8).prop_map(SetAction::Remove),
        strategy::Just(SetAction::Clear),
        strategy::Just(SetAction::FlushFork),
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

impl Modifier<KeySetIndex<Rc<Fork>, u8>> for SetAction<u8> {
    fn modify(self, set: &mut KeySetIndex<Rc<Fork>, u8>) {
        match self {
            SetAction::Put(k) => {
                set.insert(&k);
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

impl Modifier<ValueSetIndex<Rc<Fork>, u8>> for SetAction<u8> {
    fn modify(self, set: &mut ValueSetIndex<Rc<Fork>, u8>) {
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

impl FromFork for KeySetIndex<Rc<Fork>, u8> {
    fn from_fork(fork: Rc<Fork>) -> Self {
        fork.get_key_set("test")
    }

    fn clear(&mut self) {
        self.clear();
    }
}

impl FromFork for ValueSetIndex<Rc<Fork>, u8> {
    fn from_fork(fork: Rc<Fork>) -> Self {
        fork.get_value_set("test")
    }

    fn clear(&mut self) {
        self.clear();
    }
}

fn compare_key_set(set: &KeySetIndex<Rc<Fork>, u8>, ref_set: &HashSet<u8>) -> TestCaseResult {
    for k in ref_set {
        prop_assert!(set.contains(k));
    }
    for k in set.iter() {
        prop_assert!(ref_set.contains(&k));
    }
    Ok(())
}

fn compare_value_set(set: &ValueSetIndex<Rc<Fork>, u8>, ref_set: &HashSet<u8>) -> TestCaseResult {
    for k in ref_set {
        prop_assert!(set.contains(k));
    }
    for (_, k) in set.iter() {
        prop_assert!(ref_set.contains(&k));
    }
    Ok(())
}

#[test]
fn compare_key_set_to_hash_set() {
    let db = TemporaryDB::new();
    proptest!(|(ref actions in vec(generate_action(), 1..ACTIONS_MAX_LEN))| {
        compare_collections(&db, actions, compare_key_set)?;
    });
}

#[test]
fn compare_value_set_to_hash_set() {
    let db = TemporaryDB::new();
    proptest!(|(ref actions in vec(generate_action(), 1..ACTIONS_MAX_LEN))| {
        compare_collections(&db, actions, compare_value_set)?;
    });
}
