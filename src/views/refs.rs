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
use crate::views::{IndexAddress, View};
use crate::{
    BinaryKey, BinaryValue, Entry, Fork, IndexAccess, KeySetIndex, ListIndex, MapIndex, ObjectHash,
    ProofListIndex, ProofMapIndex, Snapshot, TemporaryDB,
};
use std::fmt;
use std::ops::{Deref, DerefMut};

trait ObjectCreator<'a, T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn create_list_with_root<I: Into<IndexAddress>>(
        &'a self,
        address: I,
    ) -> RefMut<ListIndex<T, V>>;

    fn create_list(&'a self) -> RefMut<ListIndex<T, V>>;
}

impl<'a, V> ObjectCreator<'a, &'a Self,V> for Fork
where
    V: BinaryValue,
{
    fn create_list_with_root<I: Into<IndexAddress>>(
        &'a self,
        address: I,
    ) -> RefMut<ListIndex<&'a Self, V>> {
        RefMut {
            value: ListIndex::create_from_address(address, self).unwrap(),
        }
    }

    fn create_list(&'a self) -> RefMut<ListIndex<&Fork, V>> {
        RefMut {
            value: ListIndex::create(self).unwrap(),
        }
    }
}

pub trait FromView<T: IndexAccess>
where
    Self: Sized,
{
    fn from_view(view: View<T>) -> Result<Self, failure::Error>;
}

impl<T, V> FromView<T> for ListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn from_view(view: View<T>) -> Result<Self, failure::Error> {
        ListIndex::get_from_view(view)
    }
}

impl<T, V> FromView<T> for Entry<T, V>
where
    T: IndexAccess,
    V: BinaryValue + ObjectHash,
{
    fn from_view(view: View<T>) -> Result<Self, failure::Error> {
        Entry::get_from_view(view)
    }
}

impl<T, K> FromView<T> for KeySetIndex<T, K>
where
    T: IndexAccess,
    K: BinaryKey,
{
    fn from_view(view: View<T>) -> Result<Self, failure::Error> {
        KeySetIndex::get_from_view(view)
    }
}

impl<T, V> FromView<T> for ProofListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue + ObjectHash,
{
    fn from_view(view: View<T>) -> Result<Self, failure::Error> {
        ProofListIndex::get_from_view(view)
    }
}

impl<T, K, V> FromView<T> for ProofMapIndex<T, K, V>
where
    T: IndexAccess,
    K: BinaryKey + ObjectHash,
    V: BinaryValue + ObjectHash,
{
    fn from_view(view: View<T>) -> Result<Self, failure::Error> {
        ProofMapIndex::get_from_view(view)
    }
}

impl<T, K, V> FromView<T> for MapIndex<T, K, V>
where
    T: IndexAccess,
    K: BinaryKey,
    V: BinaryValue,
{
    fn from_view(view: View<T>) -> Result<Self, failure::Error> {
        MapIndex::get_from_view(view)
    }
}

pub trait ObjectAccess: IndexAccess {
    fn create_view<I: Into<IndexAddress>>(&self, address: I) -> View<Self>;

    fn get_object<'a, I, T>(&'a self, address: I) -> Result<Ref<T>, failure::Error>
    where
        I: Into<IndexAddress>,
        T: FromView<Self>,
    {
        let object = T::from_view(self.create_view(address))?;
        Ok(Ref { value: object })
    }
}

impl ObjectAccess for &Box<dyn Snapshot> {
    fn create_view<I: Into<IndexAddress>>(&self, address: I) -> View<Self> {
        let address = address.into();
        View::new(&self, address)
    }
}

#[derive(Debug)]
pub struct Ref<T> {
    value: T,
}

#[derive(Debug)]
pub struct RefMut<T> {
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
        let mut index: RefMut<ListIndex<_, u32>> = fork.create_list_with_root(("index"));
        index.push(1);
    }

    db.merge(fork.into_patch());

    let snapshot = &db.snapshot();
    let index: Ref<ListIndex<_, u32>> = snapshot
        .get_object(IndexAddress::with_root("index"))
        .unwrap();

    assert_eq!(index.get(0), Some(1));
}

#[test]
#[should_panic(expected = "already exist")]
fn create_index_already_existed() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut index: RefMut<ListIndex<_, u32>> = fork.create_list_with_root(("index", &3));
    }
    {
        let mut index: RefMut<ListIndex<_, u32>> = fork.create_list_with_root(("index", &3));
    }
}

#[test]
#[should_panic(expected = "is not found")]
fn get_non_existent_index() {
    let db = TemporaryDB::new();
    let snapshot = &db.snapshot();
    let index: Ref<ListIndex<_, u32>> = snapshot
        .get_object(IndexAddress::with_root("index"))
        .unwrap();
}

#[test]
fn create_list() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    let address = {
        let mut index: RefMut<ListIndex<_, u32>> = fork.create_list();
        index.push(1);
        index.address().clone()
    };
    db.merge(fork.into_patch());

    let snapshot = &db.snapshot();
    let index: Ref<ListIndex<_, u32>> = snapshot.get_object(address).unwrap();
}
