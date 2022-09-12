// Copyright 2022 The Exonum Team
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

use std::sync::Arc;

use crate::Database;

#[cfg(not(feature = "persisted_tempdb"))]
pub use memory::TemporaryDB;
#[cfg(feature = "persisted_tempdb")]
pub use persisted::TemporaryDB;

#[cfg(not(feature = "persisted_tempdb"))]
mod memory;
#[cfg(feature = "persisted_tempdb")]
mod persisted;

impl Default for TemporaryDB {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::use_self)] // false positive
impl From<TemporaryDB> for Arc<dyn Database> {
    fn from(db: TemporaryDB) -> Self {
        Arc::new(db)
    }
}

#[test]
fn clearing_database() {
    use crate::access::CopyAccessExt;

    let db = TemporaryDB::new();
    let fork = db.fork();

    fork.get_list("foo").extend(vec![1_u32, 2, 3]);
    fork.get_proof_entry(("bar", &0_u8)).set("!".to_owned());
    fork.get_proof_entry(("bar", &1_u8)).set("?".to_owned());
    db.merge(fork.into_patch()).unwrap();
    db.clear().unwrap();

    let fork = db.fork();

    assert!(fork.index_type("foo").is_none());
    assert!(fork.index_type(("bar", &0_u8)).is_none());
    assert!(fork.index_type(("bar", &1_u8)).is_none());
    fork.get_proof_list("foo").extend(vec![4_u32, 5, 6]);
    db.merge(fork.into_patch()).unwrap();

    let snapshot = db.snapshot();
    let list = snapshot.get_proof_list::<_, u32>("foo");

    assert_eq!(list.len(), 3);
    assert_eq!(list.iter().collect::<Vec<_>>(), vec![4, 5, 6]);
}
