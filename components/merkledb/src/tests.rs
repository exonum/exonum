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

use crate::{
    Entry, Fork, KeySetIndex, ListIndex, MapIndex, ProofListIndex, ProofMapIndex, SparseListIndex,
    ValueSetIndex,
};

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
