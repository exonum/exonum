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

use crate::db::Database;
use crate::views::IndexAddress;
use crate::{BinaryValue, Fork, IndexAccess, ListIndex, Snapshot, TemporaryDB};
use std::ops::{Deref, DerefMut};
use std::fmt;

trait ObjectCreator<'a, T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn create_list<I: Into<IndexAddress> + fmt::Debug>(&'a self, address: I) -> RefMut<ListIndex<T, V>>;
}

impl<'a, V> ObjectCreator<'a, &'a Self, V> for Fork
where
    V: BinaryValue,
{
    fn create_list<I: Into<IndexAddress> + fmt::Debug>(&'a self, address: I) -> RefMut<ListIndex<&'a Self, V>> {
        RefMut {
            value: ListIndex::create_from_address(address, self).unwrap(),
        }
    }
}

trait ObjectGetter<K, T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn get<I: Into<IndexAddress>>(index_access: T, address: I) -> Result<K, failure::Error>;
}

impl<T, V> ObjectGetter<Self, T, V> for ListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn get<I: Into<IndexAddress>>(index_access: T, address: I) -> Result<Self, failure::Error> {
        ListIndex::get_from_address(address, index_access)
    }
}

trait ObjectAccess<V: BinaryValue>: IndexAccess {
    fn get_object<K: ObjectGetter<K, Self, V>>(&self, address: IndexAddress) -> Ref<K>;
}

impl<'a, V: BinaryValue> ObjectAccess<V> for &'a Box<dyn Snapshot> {
    fn get_object<K: ObjectGetter<K, Self, V>>(&self, address: IndexAddress) -> Ref<K> {
        Ref {
            // TODO: remove unwrap
            value: K::get(*self, address).unwrap(),
        }
    }
}

#[derive(Debug)]
struct Ref<T> {
    value: T,
}

#[derive(Debug)]
struct RefMut<T> {
    value: T,
}

impl<T> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> Deref for RefMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for RefMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

#[test]
fn basic_object_refs() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut index: RefMut<ListIndex<_, u32>> = fork.create_list("index");

        index.push(1);
    }

    db.merge(fork.into_patch());

    let snapshot = &db.snapshot();
    let index: Ref<ListIndex<_, u32>> = snapshot.get_object(IndexAddress::with_root("index"));

    assert_eq!(index.get(0), Some(1));
}
