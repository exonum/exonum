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

//! Tests that compare exonum collections and corresponding rust types using proptest.

use modifier::Modifier;
use proptest::test_runner::TestCaseResult;

use std::rc::Rc;

use exonum_merkledb::{Database, Fork, TemporaryDB};

// Max size of the generated sequence of actions.
pub const ACTIONS_MAX_LEN: usize = 100;

pub trait FromFork {
    fn from_fork(fork: Rc<Fork>) -> Self;
    fn clear(&mut self);
}

pub enum ForkAction {
    Flush,
    Merge,
}

pub trait AsForkAction {
    fn as_fork_action(&self) -> Option<ForkAction>;
}

pub fn compare_collections<A, R, T>(
    db: &TemporaryDB,
    actions: &[A],
    compare: impl Fn(&T, &R) -> TestCaseResult,
) -> TestCaseResult
where
    A: Clone + AsForkAction + Modifier<R> + Modifier<T> + std::fmt::Debug,
    R: Default,
    T: FromFork,
{
    let mut fork = Rc::new(db.fork());
    {
        let mut collection = T::from_fork(fork.clone());
        collection.clear();
    }
    let mut reference = R::default();

    for action in actions {
        match action.as_fork_action() {
            Some(ForkAction::Merge) => {
                let patch = Rc::try_unwrap(fork).expect("fork ref leaked").into_patch();
                db.merge(patch).unwrap();
                fork = Rc::new(db.fork());
            }
            Some(ForkAction::Flush) => {
                Rc::get_mut(&mut fork).expect("fork ref leaked").flush();
            }
            None => {
                let mut collection = T::from_fork(fork.clone());
                action.clone().modify(&mut collection);
                action.clone().modify(&mut reference);
                compare(&collection, &reference)?;
            }
        }
    }
    let collection = T::from_fork(fork);
    compare(&collection, &reference)
}
