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

use crate::views::IndexAddress;
use crate::{BinaryValue, Fork, IndexAccess, ListIndex, TemporaryDB};
use std::ops::{Deref, DerefMut};
use crate::db::Database;

trait ObjectAccess<'a, T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn create_list<I: Into<IndexAddress>>(&'a self, address: I) -> RefMut<ListIndex<T, V>>;
}

impl<'a, V> ObjectAccess<'a, &'a Self, V> for Fork
where
    V: BinaryValue,
{
    fn create_list<I: Into<IndexAddress>>(&'a self, address: I) -> RefMut<ListIndex<&'a Self, V>> {
        RefMut {
            view_mut: ListIndex::create_from_address(address, self).unwrap(),
        }
    }
}

#[derive(Debug)]
struct Ref<T> {
    view: T,
}

#[derive(Debug)]
struct RefMut<T> {
    view_mut: T,
}

impl<T> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl<T> Deref for RefMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.view_mut
    }
}

impl<T> DerefMut for RefMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.view_mut
    }
}

#[test]
fn basic_ref() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let address = IndexAddress::new().append_name("index");
        let mut index: RefMut<ListIndex<_, u32>> = fork.create_list(address);

        index.push(10);
    }

    db.merge(fork.into_patch());

    let snapshot = db.snapshot();

    let index: ListIndex<_, u32> = ListIndex::new("index", &snapshot);

    assert_eq!(index.get(0), Some(1));
}
