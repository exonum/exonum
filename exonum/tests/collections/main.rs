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

//! Tests that compare exonum collections and corresponding rust types using proptest.

#[macro_use]
extern crate proptest;

macro_rules! proptest_compare_collections {
    ($name:ident, $collection:ident, $reference:ident, $action:ident) => {
        proptest! {
            #[test]
            fn $name(ref actions in vec(generate_action(), 1..ACTIONS_MAX_LEN)) {
                use exonum::storage::{Database, MemoryDB};

                let db = MemoryDB::new();

                let mut fork = db.fork();
                let mut reference = $reference::new();

                for action in actions {
                    match action {
                        $action::MergeFork => {
                            db.merge(fork.into_patch()).unwrap();
                            fork = db.fork();
                        }
                        _ => {
                            let mut collection = $collection::new("test", &mut fork);
                            action.clone().modify(&mut collection);
                            action.clone().modify(&mut reference);
                            compare_collections(&collection, &reference)?;
                        }
                    }
                }
                db.merge(fork.into_patch()).unwrap();

                let mut fork = db.fork();
                let collection = $collection::new("test", &mut fork);
                compare_collections(&collection, &reference)?;
            }
        }
    };
}

mod list;
mod map;
mod set;

// Max size of the generated sequence of actions.
const ACTIONS_MAX_LEN: usize = 100;
