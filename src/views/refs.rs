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

use std::ops::{Deref, DerefMut};

use crate::{
    views::{IndexAddress, View},
    BinaryKey, BinaryValue, Entry, Fork, IndexAccess, KeySetIndex, ListIndex, MapIndex, ObjectHash,
    ProofListIndex, ProofMapIndex, Snapshot,
};

pub trait FromView<T: IndexAccess>
where
    Self: Sized,
{
    ///TODO: add documentation [ECR-2820]
    fn get(view: View<T>) -> Result<Self, failure::Error>;

    ///TODO: add documentation [ECR-2820]
    fn create(view: View<T>) -> Self;
}

impl<T, V> FromView<T> for ListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn get(view: View<T>) -> Result<Self, failure::Error> {
        Self::get_from_view(view)
    }

    fn create(view: View<T>) -> Self {
        Self::create_from_view(view)
    }
}

impl<T, V> FromView<T> for Entry<T, V>
where
    T: IndexAccess,
    V: BinaryValue + ObjectHash,
{
    fn get(view: View<T>) -> Result<Self, failure::Error> {
        Self::get_from_view(view)
    }

    fn create(view: View<T>) -> Self {
        Self::create_from_view(view)
    }
}

impl<T, K> FromView<T> for KeySetIndex<T, K>
where
    T: IndexAccess,
    K: BinaryKey,
{
    fn get(view: View<T>) -> Result<Self, failure::Error> {
        Self::get_from_view(view)
    }

    fn create(view: View<T>) -> Self {
        Self::create_from_view(view)
    }
}

impl<T, V> FromView<T> for ProofListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue + ObjectHash,
{
    fn get(view: View<T>) -> Result<Self, failure::Error> {
        Self::get_from_view(view)
    }

    fn create(view: View<T>) -> Self {
        Self::create_from_view(view)
    }
}

impl<T, K, V> FromView<T> for ProofMapIndex<T, K, V>
where
    T: IndexAccess,
    K: BinaryKey + ObjectHash,
    V: BinaryValue + ObjectHash,
{
    fn get(view: View<T>) -> Result<Self, failure::Error> {
        Self::get_from_view(view)
    }

    fn create(view: View<T>) -> Self {
        Self::create_from_view(view)
    }
}

impl<T, K, V> FromView<T> for MapIndex<T, K, V>
where
    T: IndexAccess,
    K: BinaryKey,
    V: BinaryValue,
{
    fn get(view: View<T>) -> Result<Self, failure::Error> {
        Self::get_from_view(view)
    }

    fn create(view: View<T>) -> Self {
        Self::create_from_view(view)
    }
}

pub trait ObjectAccess: IndexAccess {
    fn create_view<I: Into<IndexAddress>>(&self, address: I) -> View<Self>;

    fn get<I, T>(&self, address: I) -> Result<Ref<T>, failure::Error>
    where
        I: Into<IndexAddress>,
        T: FromView<Self>,
    {
        let object = T::get(self.create_view(address))?;
        Ok(Ref { value: object })
    }
}

impl ObjectAccess for &Box<dyn Snapshot> {
    //TODO: check if view creation here is redundant
    fn create_view<I: Into<IndexAddress>>(&self, address: I) -> View<Self> {
        let address = address.into();
        View::new(self, address)
    }
}

impl Fork {
    ///TODO: add documentation [ECR-2820]
    pub fn create_object<'a, T>(&'a self) -> T
    where
        T: FromView<&'a Self>,
    {
        let view = View::new(self, IndexAddress::default());
        T::create(view)
    }

    ///TODO: add documentation [ECR-2820]
    pub fn create_object_with_root<'a, T, I>(&'a self, address: I) -> T
    where
        T: FromView<&'a Self>,
        I: Into<IndexAddress>,
    {
        let view = View::new(self, address);
        T::create(view)
    }

    ///TODO: add documentation [ECR-2820]
    pub fn get<'a, T, I>(&'a self, address: I) -> Option<Ref<T>>
    where
        T: FromView<&'a Self>,
        I: Into<IndexAddress>,
    {
        let view = View::new(self, address);
        //TODO: remove unwrap
        let object = T::get(view).unwrap();

        Some(Ref { value: object })
    }

    ///TODO: add documentation [ECR-2820]
    pub fn get_mut<'a, T, I>(&'a self, address: I) -> Option<RefMut<T>>
    where
        T: FromView<&'a Self>,
        I: Into<IndexAddress>,
    {
        let view = View::new(self, address);
        //TODO: remove unwrap
        let object = T::get(view).unwrap();

        Some(RefMut { value: object })
    }
}

#[derive(Debug)]
#[allow(dead_code)]
//TODO: remove dead_code
///TODO: add documentation [ECR-2820]
pub struct Ref<T> {
    value: T,
}

#[derive(Debug)]
///TODO: add documentation [ECR-2820]
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

#[cfg(test)]
mod tests {
    use crate::{
        db::Database,
        views::{
            refs::{ObjectAccess, Ref, RefMut},
            IndexAddress,
        },
        ListIndex, TemporaryDB,
    };

    #[test]
    fn basic_object_refs() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut index: ListIndex<_, u32> = fork.create_object_with_root("index");
            index.push(1);
        }

        db.merge(fork.into_patch()).unwrap();

        let snapshot = &db.snapshot();
        let index: Ref<ListIndex<_, u32>> = snapshot
            //TODO: fix `From` implementation for `IndexAddress`
            .get(IndexAddress::with_root("index"))
            .unwrap();

        assert_eq!(index.get(0), Some(1));
    }

    #[test]
    #[should_panic(expected = "already exist")]
    fn create_index_already_existed() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut _index: ListIndex<_, u32> = fork.create_object_with_root(("index", &3));
        }
        {
            let mut _index: ListIndex<_, u32> = fork.create_object_with_root(("index", &3));
        }
    }

    #[test]
    #[should_panic(expected = "is not found")]
    fn get_non_existent_index() {
        let db = TemporaryDB::new();
        let snapshot = &db.snapshot();
        let _index: Ref<ListIndex<_, u32>> =
            snapshot.get(IndexAddress::with_root("index")).unwrap();
    }

    #[test]
    fn create_list() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let address = {
            let mut index: ListIndex<_, u32> = fork.create_object();
            index.push(1);
            index.address().clone()
        };
        db.merge(fork.into_patch()).unwrap();

        let snapshot = &db.snapshot();
        let _index: Ref<ListIndex<_, u32>> = snapshot.get(address).unwrap();
    }

    #[test]
    fn fork_get_mut() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let _list: ListIndex<_, u32> = fork.create_object_with_root("index");
        }

        db.merge(fork.into_patch()).unwrap();

        let fork = db.fork();
        let mut list: RefMut<ListIndex<_, u32>> = fork.get_mut("index").unwrap();

        list.push(1);
    }
}
