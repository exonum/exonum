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
    db::Change::Put,
    views::{
        metadata::{index_metadata, INDEXES_POOL_NAME},
        ChangeSet, IndexAddress, IndexType, View,
    },
    BinaryKey, BinaryValue, Entry, Fork, IndexAccess, KeySetIndex, ListIndex, MapIndex, ObjectHash,
    ProofListIndex, ProofMapIndex, Snapshot,
};
use rand::Rng;
use uuid::Uuid;

pub trait AnyObject<T: IndexAccess> {
    fn view(self) -> View<T>;
    fn object_type(&self) -> IndexType;
    fn metadata(&self) -> Vec<u8>;
}

pub trait FromView<T: IndexAccess>
where
    Self: Sized,
{
    ///TODO: add documentation [ECR-2820]
    fn get(view: View<T>) -> Option<Self>;

    ///TODO: add documentation [ECR-2820]
    fn create(view: View<T>) -> Self;
}

impl<T, V> FromView<T> for ListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn get(view: View<T>) -> Option<Self> {
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
    fn get(view: View<T>) -> Option<Self> {
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
    fn get(view: View<T>) -> Option<Self> {
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
    fn get(view: View<T>) -> Option<Self> {
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
    fn get(view: View<T>) -> Option<Self> {
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
    fn get(view: View<T>) -> Option<Self> {
        Self::get_from_view(view)
    }

    fn create(view: View<T>) -> Self {
        Self::create_from_view(view)
    }
}

pub trait ObjectAccess: IndexAccess {
    fn create_view<I: Into<IndexAddress>>(&self, address: I) -> View<Self>;

    fn get_object<I, T>(&self, address: I) -> Option<Ref<T>>
    where
        I: Into<IndexAddress>,
        T: FromView<Self>,
    {
        T::get(self.create_view(address)).map(|value| Ref { value })
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
        let mut rng = rand::thread_rng();

        let my_uuid = Uuid::new_v4();

        let mut pool_length = rng.gen::<u64>();
        let address = IndexAddress::with_root("temp").append_bytes(&my_uuid.into_bytes());
        let view = View::new(self, address);
        //TODO: don't create redundant metadata
        T::create(view)
    }

    ///TODO: add documentation [ECR-2820]
    pub fn get_object<'a, T, I>(&'a self, address: I) -> Option<Ref<T>>
    where
        T: FromView<&'a Self>,
        I: Into<IndexAddress>,
    {
        let view = View::new(self, address);
        T::get(view).map(|value| Ref { value })
    }

    ///TODO: add documentation [ECR-2820]
    pub fn get_object_mut<'a, T, I>(&'a self, address: I) -> Option<RefMut<T>>
    where
        T: FromView<&'a Self>,
        I: Into<IndexAddress>,
    {
        let view = View::new(self, address);
        T::get(view).map(|value| RefMut { value })
    }

    ///TODO: add documentation [ECR-2820]
    pub fn insert<I, T, A>(&self, address: I, object: A)
    where
        I: Into<IndexAddress>,
        T: IndexAccess,
        A: AnyObject<T>,
    {
        let index_type = object.object_type();
        let metadata = object.metadata();
        let view = object.view();
        let address = address.into().clone();

        let (new_address, _state) = index_metadata::<_, ()>(self, &address, index_type);
        self.working_patch().clear(&view.address);

        let mut metadata_changes = self
            .working_patch()
            .changes_mut(&IndexAddress::with_root(INDEXES_POOL_NAME));

        metadata_changes
            .data
            .insert(address.fully_qualified_name(), Put(metadata));

        let mut changes = self.working_patch().changes_mut(&new_address);
        let view_changes = view.changes.as_ref().unwrap().clone();

        changes.data.extend(view_changes.data);
    }
}

#[derive(Debug)]
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
        views::refs::{ObjectAccess, Ref, RefMut},
        KeySetIndex, ListIndex, ProofListIndex, TemporaryDB,
    };
    use exonum_crypto::{Hash, PublicKey};

    #[test]
    fn basic_object_refs() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut index: ListIndex<_, u32> = fork.create_object();
            index.push(1);
            fork.insert("index", index);

            let index: Option<Ref<ListIndex<_, u32>>> = fork.get_object("index");
            assert!(index.is_some());
        }
        {
            let mut index: RefMut<ListIndex<_, u32>> = fork.get_object_mut("index").unwrap();
            index.push(2);
        }

        db.merge(fork.into_patch()).unwrap();

        let snapshot = &db.snapshot();
        let index: Ref<ListIndex<_, u32>> = snapshot.get_object("index").unwrap();

        assert_eq!(index.get(0), Some(1));
        assert_eq!(index.get(1), Some(2));
    }

    #[test]
    fn get_non_existent_index() {
        let db = TemporaryDB::new();
        let snapshot = &db.snapshot();
        let index: Option<Ref<ListIndex<_, u32>>> = snapshot.get_object("index");

        assert!(index.is_none());
    }

    #[test]
    fn fork_get_mut() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let list: ListIndex<_, u32> = fork.create_object();
            fork.insert("index", list);
        }

        db.merge(fork.into_patch()).unwrap();

        let fork = db.fork();
        {
            let mut list: RefMut<ListIndex<_, u32>> = fork.get_object_mut("index").unwrap();
            list.push(1);
        }

        db.merge(fork.into_patch()).unwrap();

        let snapshot = &db.snapshot();
        let list: Ref<ListIndex<_, u32>> = snapshot.get_object("index").unwrap();

        assert_eq!(list.get(0), Some(1));
    }

    #[test]
    fn fork_multiple_insert() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut index1: KeySetIndex<_, u32> = fork.create_object();
            let mut index2: KeySetIndex<_, u32> = fork.create_object();
            index1.insert(1);
            index2.insert(2);
            fork.insert("index1", index1);
            fork.insert("index2", index2);
        }
        db.merge(fork.into_patch()).unwrap();

        let snapshot = &db.snapshot();
        let index1: Ref<KeySetIndex<_, u32>> = snapshot.get_object("index1").unwrap();
        let index2: Ref<KeySetIndex<_, u32>> = snapshot.get_object("index2").unwrap();

        assert!(index1.contains(&1));
        assert!(index2.contains(&2));
    }

    #[test]
    fn ref_list_length() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut index: ListIndex<_, u32> = fork.create_object();
            index.push(1);

            let mut index2: ListIndex<_, u32> = fork.create_object();
            assert_eq!(index2.len(), 0);
            fork.insert("index", index);
        }

        db.merge(fork.into_patch()).unwrap();

        let snapshot = &db.snapshot();
        let index1: Ref<ListIndex<_, u32>> = snapshot.get_object("index").unwrap();

        assert_eq!(index1.len(), 1);
    }

    #[test]
    fn ref_proof_list() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let owner = PublicKey::zero();
        {
            let wallets_history: ProofListIndex<_, Hash> = fork.create_object();
            let address = ("wallets.history", &owner);
            fork.insert(address, wallets_history);
            let mut history: RefMut<ProofListIndex<_, Hash>> =
                fork.get_object_mut(address).unwrap();

            history.push(Hash::zero());
        }

        db.merge(fork.into_patch()).unwrap();

        let snapshot = &db.snapshot();
        let address = ("wallets.history", &owner);

        let history: Ref<ProofListIndex<_, Hash>> = snapshot.get_object(address).unwrap();

        dbg!(history.get(0));
    }

}
