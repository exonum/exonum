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

use modifier::Modifier;
use proptest::test_runner::TestCaseResult;

use std::rc::Rc;

use exonum_crypto::{hash, Hash, HASH_SIZE};
use exonum_merkledb::{BinaryKey, Database, Fork, ObjectHash, TemporaryDB};

// Max size of the generated sequence of actions.
//
// Due external tests running mechanism this file is linked
// separately with other modules in tests directory. This
// constant is used in all of then except `proof_map_index`, that's
// why it marked with `dead_code`.
#[allow(dead_code)]
pub const ACTIONS_MAX_LEN: usize = 100;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Key(pub [u8; HASH_SIZE]);

impl ObjectHash for Key {
    fn object_hash(&self) -> Hash {
        hash(&self.0)
    }
}

impl From<[u8; HASH_SIZE]> for Key {
    fn from(key: [u8; HASH_SIZE]) -> Self {
        Self(key)
    }
}

impl BinaryKey for Key {
    fn size(&self) -> usize {
        HASH_SIZE
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer.copy_from_slice(&self.0);
        self.0.len()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        let mut buf = [0; 32];
        buf.copy_from_slice(&buffer);
        Self(buf)
    }
}

pub trait FromFork {
    fn from_fork(fork: Rc<Fork>) -> Self;
    fn clear(&mut self);
}

// See `ACTIONS_MAX_LEN` comment above.
#[allow(dead_code)]
pub struct MergeFork;

// See `ACTIONS_MAX_LEN` comment above.
#[allow(dead_code)]
pub fn compare_collections<A, R, T>(
    db: &TemporaryDB,
    actions: &[A],
    compare: impl Fn(&T, &R) -> TestCaseResult,
) -> TestCaseResult
where
    A: Clone + PartialEq<MergeFork> + Modifier<R> + Modifier<T>,
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
        if *action == MergeFork {
            let patch = Rc::try_unwrap(fork).expect("fork ref leaked").into_patch();
            db.merge(patch).unwrap();
            fork = Rc::new(db.fork());
        } else {
            let mut collection = T::from_fork(fork.clone());
            action.clone().modify(&mut collection);
            action.clone().modify(&mut reference);
            compare(&collection, &reference)?;
        }
    }
    let collection = T::from_fork(fork);
    compare(&collection, &reference)
}
