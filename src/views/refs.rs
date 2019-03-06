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
use crate::{
    views::IndexAddress, BinaryValue, IndexAccess, IndexBuilder, ListIndex, Snapshot, TemporaryDB,
};
use std::ops::{Deref, DerefMut};

trait ObjectAccess<V: BinaryValue>: IndexAccess {

    fn create_list<I: Into<IndexAddress>>(&self, address: I) -> ListIndex<Self, V>;

    fn get_object<K: ObjectGetter<K, Self, V>>(&self, address: IndexAddress) -> Ref<K>;
}

#[derive(Debug)]
struct Ref<T> {
    view: T,
}

trait ObjectGetter<K, T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn get<I: Into<IndexAddress>>(index_access: T, address: I) -> K;
}

impl<T, V> ObjectGetter<Self, T, V> for ListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn get<I: Into<IndexAddress>>(index_access: T, address: I) -> Self {
        ListIndex::new("index", index_access)
    }
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

impl<'a, V: BinaryValue> ObjectAccess<V> for &'a Box<dyn Snapshot> {

    fn create_list<I: Into<IndexAddress>>(&self, address: I) -> ListIndex<Self, V> {
        let address = address.into();
        ListIndex::new(address.name, *self)
    }

    fn get_object<K: ObjectGetter<K, Self, V>>(&self, address: IndexAddress) -> Ref<K> {
        Ref {
            view: K::get(*self, address),
        }
    }
}

#[test]
fn get_reference_from_snapshot() {
    let db = TemporaryDB::new();

    let snapshot = &db.snapshot();

    {
        let address = IndexAddress::new().append_name("index");

        let list_index: Ref<ListIndex<_, u32>> = snapshot.get_object(address);

        assert_eq!(list_index.get(1), None);
    }
}
