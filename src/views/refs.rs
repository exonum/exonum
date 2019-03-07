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
use crate::views::IndexType;
use crate::{views::IndexAddress, BinaryValue, IndexAccess, IndexBuilder, ListIndex, Snapshot, TemporaryDB, Fork};
use std::ops::{Deref, DerefMut};

trait ObjectAccess<V: BinaryValue>: IndexAccess {

    fn create_list<I: Into<IndexAddress>>(&self, address: I) -> Ref<ListIndex<Self, V>>;

    fn get_object<K: ObjectGetter<K, Self, V>>(&self, address: IndexAddress) -> Ref<K>;
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

impl<'a, V: BinaryValue> ObjectAccess<V> for &'a Box<dyn Snapshot> {

    fn create_list<I: Into<IndexAddress>>(&self, address: I) -> Ref<ListIndex<Self, V>> {

        //TODO: remove unwrap
        Ref {
            view: ListIndex::create_from_address(address, *self).unwrap()
        }

    }

    fn get_object<K: ObjectGetter<K, Self, V>>(&self, address: IndexAddress) -> Ref<K> {
        Ref {
            //TODO: remove unwrap
            view: K::get(*self, address).unwrap(),
        }
    }
}

impl<V: BinaryValue> ObjectAccess<V> for &Fork {

    fn create_list<I: Into<IndexAddress>>(&self, address: I) -> Ref<ListIndex<Self, V>> {

        //TODO: remove unwrap
        Ref {
            view: ListIndex::create_from_address(address, *self).unwrap()
        }
    }

    fn get_object<K: ObjectGetter<K, Self, V>>(&self, address: IndexAddress) -> Ref<K> {
        Ref {
            //TODO: remove unwrap
            view: K::get(*self, address).unwrap(),
        }
    }
}


#[test]
fn get_reference_from_snapshot() {
    let db = TemporaryDB::new();
    let snapshot = &db.snapshot();

    {
        let list_index: Ref<ListIndex<_, u32>> = snapshot.get_object("index".into());

        assert_eq!(list_index.get(1), None);
    }
}

#[test]
fn create_index_already_existed() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let address = IndexAddress::new().append_name("index");
        let mut index: Ref<ListIndex<_, u32>> = (&fork).create_list(address);
    }

    db.merge(fork.into_patch());

    let fork = &db.fork();

    let index_with_same_address: Ref<ListIndex<_, u8>> = fork.get_object("index".into());
}