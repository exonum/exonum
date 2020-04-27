// Copyright 2020 The Exonum Team
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

//! Tests related to components and `FromAccess` derivation.

use exonum_derive::FromAccess;

use exonum_merkledb::{
    access::{Access, CopyAccessExt, FromAccess, RawAccessMut},
    BinaryKey, Database, Entry, Group, Lazy, ListIndex, ObjectHash, ProofEntry, ProofMapIndex,
    TemporaryDB,
};

#[derive(FromAccess)]
struct Simple<Acc: Access> {
    map: ProofMapIndex<Acc::Base, u64, String>,
    list: ListIndex<Acc::Base, String>,
}

#[derive(FromAccess)]
struct Generic<Acc: Access, V>
where
    V: ObjectHash + BinaryKey,
{
    inner: ProofMapIndex<Acc::Base, V, u64>,
}

impl<T: Access> Simple<T>
where
    T::Base: RawAccessMut,
{
    fn modify(&mut self, key: u64, value: String) {
        self.list.push(value.clone());
        self.map.put(&key, value);
    }
}

#[derive(FromAccess)]
struct ComplexSchema<T: Access> {
    count: Entry<T::Base, u64>,
    generic: Generic<T, String>,
    lazy: Lazy<T, Simple<T>>,
    group: Group<T, str, Simple<T>>,
}

impl<T: Access> ComplexSchema<T>
where
    T::Base: RawAccessMut,
{
    fn modify(&mut self, key: u64, value: String) {
        self.generic.inner.put(&value, key);
        self.count.set(self.count.get().unwrap_or_default() + 1);
        self.lazy.get().modify(key, value.clone());
        self.group.get(&value).modify(key, value);
    }
}

#[test]
fn embedded_components() {
    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut complex = ComplexSchema::from_root(&fork).unwrap();
        assert!(!complex.count.exists());
        complex.modify(1, "!".to_owned());
        complex.modify(2, "!!".to_owned());
        complex.modify(1, "?".to_owned());
        complex.modify(42, "!".to_owned());

        assert_eq!(complex.count.get(), Some(4));
        assert_eq!(complex.generic.inner.get(&"!".to_owned()), Some(42));

        let lazy = complex.lazy.get();
        assert_eq!(lazy.map.keys().collect::<Vec<_>>(), vec![1, 2, 42]);
        assert_eq!(
            lazy.list.iter().collect::<Vec<_>>(),
            vec![
                "!".to_owned(),
                "!!".to_owned(),
                "?".to_owned(),
                "!".to_owned()
            ]
        );
        let grouped = complex.group.get("!");
        assert_eq!(
            grouped.map.iter().collect::<Vec<_>>(),
            vec![(1, "!".to_owned()), (42, "!".to_owned())]
        );
        assert_eq!(grouped.list.len(), 2);
    }

    // Check that the indexes are laid out as expected.
    assert_eq!(fork.get_entry("count").get(), Some(4_u64));
    assert_eq!(
        fork.get_proof_map::<_, _, u64>("generic.inner")
            .get("!")
            .unwrap(),
        42
    );

    let lazy_map = fork.get_proof_map::<_, u64, String>("lazy.map");
    assert_eq!(lazy_map.keys().collect::<Vec<_>>(), vec![1, 2, 42]);
    let lazy_list = fork.get_list::<_, String>("lazy.list");
    assert_eq!(
        lazy_list.iter().collect::<Vec<_>>(),
        vec![
            "!".to_owned(),
            "!!".to_owned(),
            "?".to_owned(),
            "!".to_owned()
        ]
    );

    let grouped_map = fork.get_proof_map::<_, u64, String>(("group.map", "!"));
    assert_eq!(
        grouped_map.iter().collect::<Vec<_>>(),
        vec![(1, "!".to_owned()), (42, "!".to_owned())]
    );
    let grouped_list = fork.get_list::<_, String>(("group.list", "!"));
    assert_eq!(grouped_list.len(), 2);
}

#[test]
fn tuple_component() {
    #[derive(FromAccess)]
    struct Tuple<T: Access>(
        #[from_access(rename = "strings")] ListIndex<T::Base, String>,
        #[from_access(rename = "numbers")] ListIndex<T::Base, u64>,
    );

    let db = TemporaryDB::new();
    let fork = db.fork();
    fork.get_list("strings").push("!".to_owned());
    fork.get_list("numbers").push(42_u64);

    let Tuple(strings, numbers) = Tuple::from_root(&fork).unwrap();
    assert_eq!(strings.len(), 1);
    assert_eq!(strings.iter().collect::<Vec<_>>(), vec!["!".to_owned()]);
    assert_eq!(numbers.len(), 1);
    assert_eq!(numbers.get(0), Some(42));
}

#[test]
fn tuple_wrapper() {
    #[derive(FromAccess)]
    #[from_access(transparent)]
    struct Wrapper<T: Access>(ProofMapIndex<T::Base, u64, u64>);

    let db = TemporaryDB::new();
    let fork = db.fork();
    fork.get_proof_map("wrapper").put(&1_u64, 2_u64);
    let wrapper = Wrapper::from_access(&fork, "wrapper".into()).unwrap();
    assert_eq!(wrapper.0.get(&1_u64).unwrap(), 2);
}

#[test]
fn wrapper_with_named_field() {
    #[derive(FromAccess)]
    #[from_access(transparent)]
    struct Wrapper<T: Access> {
        inner: ProofMapIndex<T::Base, u64, u64>,
    }

    let db = TemporaryDB::new();
    let fork = db.fork();
    fork.get_proof_map("wrapper").put(&1_u64, 2_u64);
    let wrapper = Wrapper::from_access(&fork, "wrapper".into()).unwrap();
    assert_eq!(wrapper.inner.get(&1_u64).unwrap(), 2);
}

#[test]
fn component_with_implicit_type_param() {
    #[derive(FromAccess)]
    struct Schema<T>
    where
        T: Access,
        T::Base: RawAccessMut,
    {
        map: ProofMapIndex<T::Base, u64, u64>,
    }

    let db = TemporaryDB::new();
    let fork = db.fork();
    fork.get_proof_map("map").put(&1_u64, 2_u64);
    let schema = Schema::from_root(&fork).unwrap();
    assert_eq!(schema.map.get(&1_u64).unwrap(), 2);
}

#[test]
fn public_schema_pattern() {
    #[derive(Debug, FromAccess)]
    struct Schema<T: Access> {
        pub wallets: ProofMapIndex<T::Base, str, u64>,
        pub total_balance: ProofEntry<T::Base, u64>,
    }

    #[derive(Debug, FromAccess)]
    struct SchemaImpl<T: Access> {
        /// Flattened components are useful to split schemas into a public interface
        /// and implementation details.
        #[from_access(flatten)]
        public: Schema<T>,
        private_entry: Entry<T::Base, String>,
        private_list: ListIndex<T::Base, u64>,
    }

    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut schema: SchemaImpl<_> = SchemaImpl::from_root(&fork).unwrap();
        schema.public.wallets.put("Alice", 10);
        schema.public.wallets.put("Bob", 20);
        schema.public.total_balance.set(30);
        schema.private_entry.set("XNM".to_owned());
        schema.private_list.extend(vec![10, 20]);
    }

    let interface = Schema::from_root(fork.readonly()).unwrap();
    assert_eq!(interface.wallets.values().sum::<u64>(), 30);
    assert_eq!(interface.total_balance.get(), Some(30));
}

#[test]
fn flattened_unnamed_fields() {
    #[derive(FromAccess)]
    struct Flattened<T: Access> {
        entry: Entry<T::Base, String>,
        other_entry: Entry<T::Base, u64>,
    }

    #[derive(FromAccess)]
    struct Wrapper<T: Access>(
        #[from_access(flatten)] Flattened<T>,
        #[from_access(rename = "list")] ListIndex<T::Base, u8>,
    );

    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut wrapper = Wrapper::from_root(&fork).unwrap();
        wrapper.0.entry.set("!!".to_owned());
        wrapper.0.other_entry.set(42);
        wrapper.1.extend(vec![1, 2, 3]);
    }
    assert_eq!(fork.get_entry::<_, String>("entry").get().unwrap(), "!!");
    assert_eq!(fork.get_list::<_, u8>("list").get(1), Some(2));
}

#[test]
fn multiple_flattened_fields() {
    #[derive(FromAccess)]
    struct Flattened<T: Access> {
        entry: Entry<T::Base, String>,
        other_entry: Entry<T::Base, u64>,
    }

    #[derive(FromAccess)]
    struct OtherFlattened<T: Access> {
        list: ListIndex<T::Base, Vec<u8>>,
        maps: Group<T, u32, ProofMapIndex<T::Base, str, u64>>,
    }

    #[derive(FromAccess)]
    struct Schema<T: Access> {
        #[from_access(flatten)]
        first: Flattened<T>,
        #[from_access(flatten)]
        second: OtherFlattened<T>,
    }

    let db = TemporaryDB::new();
    let fork = db.fork();
    {
        let mut schema = Schema::from_root(&fork).unwrap();
        schema.first.entry.set("Some".to_owned());
        schema.first.other_entry.set(1);
        schema.second.list.push(vec![2, 3, 4]);
        schema.second.maps.get(&23).put("Alice", 1);
    }
    assert_eq!(fork.get_entry::<_, String>("entry").get().unwrap(), "Some");
    assert_eq!(fork.get_list::<_, Vec<u8>>("list").len(), 1);
    assert_eq!(
        fork.get_proof_map(("maps", &23_u32)).get("Alice"),
        Some(1_u64)
    );
}
