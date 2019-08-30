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

use exonum_crypto::Hash;

use crate::{Entry, Fork, KeySetIndex, ListIndex, MapIndex, ProofListIndex, ProofMapIndex, SparseListIndex, ValueSetIndex, TemporaryDB, Database, View, IndexAddress};

// This should compile to ensure ?Sized bound on `new_in_family` (see #1024).
#[allow(dead_code, unreachable_code, unused_variables)]
fn should_compile() {
    let fork: Fork = unimplemented!();
    let _: Entry<_, ()> = Entry::new_in_family("", "", &fork);
    let _: KeySetIndex<_, Hash> = KeySetIndex::new_in_family("", "", &fork);
    let _: ListIndex<_, ()> = ListIndex::new_in_family("", "", &fork);
    let _: MapIndex<_, Hash, ()> = MapIndex::new_in_family("", "", &fork);
    let _: ProofListIndex<_, ()> = ProofListIndex::new_in_family("", "", &fork);
    let _: ProofMapIndex<_, Hash, ()> = ProofMapIndex::new_in_family("", "", &fork);
    let _: SparseListIndex<_, ()> = SparseListIndex::new_in_family("", "", &fork);
    let _: ValueSetIndex<_, ()> = ValueSetIndex::new_in_family("", "", &fork);
}

#[test]
fn data_interference() {
    let db = TemporaryDB::new();
    let fork = db.fork();

    {
        let mut index: ListIndex<_, i32> = ListIndex::new("index", &fork);
        index.push(1);
    }

    db.merge(fork.into_patch());

    let fork = db.fork();
    {
        let address = IndexAddress::new().append_bytes(&vec![0_u8; 8]);
        let mut view = View::new(&fork, address.clone());

        view.clear();
    }
    db.merge(fork.into_patch());

    let snapshot = db.snapshot();
    let index: ListIndex<_, i32> = ListIndex::new("index", &snapshot);

    assert_eq!(index.get(0), None);
}
