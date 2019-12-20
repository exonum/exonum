//! Tests related to components and `FromAccess` derivation.

use exonum_derive::FromAccess;

use exonum_merkledb::{
    access::{Access, AccessExt, FromAccess, RawAccessMut},
    BinaryKey, Database, Entry, Group, Lazy, ListIndex, ObjectHash, ProofMapIndex, TemporaryDB,
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
// Since the name ends with `Schema`, the `new` constructor is derived automatically
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
        let mut complex = ComplexSchema::new(&fork);
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
    let schema = Schema::new(&fork);
    assert_eq!(schema.map.get(&1_u64).unwrap(), 2);
}

#[test]
fn schema_with_non_standard_naming() {
    #[derive(FromAccess)]
    #[from_access(schema)]
    struct NonStandard<T: Access> {
        map: ProofMapIndex<T::Base, u64, u64>,
    }

    let db = TemporaryDB::new();
    let fork = db.fork();
    fork.get_proof_map("map").put(&1_u64, 2_u64);
    let schema = NonStandard::new(&fork);
    assert_eq!(schema.map.get(&1_u64).unwrap(), 2);
}

#[test]
fn opt_out_from_schema() {
    #[derive(FromAccess)]
    #[from_access(schema = false)]
    struct NotSchema<T: Access> {
        map: ProofMapIndex<T::Base, u64, u64>,
    }

    impl<T: Access> NotSchema<T> {
        fn new(access: T, msg: &str) -> Self {
            Self::from_root(access).expect(msg)
        }
    }

    let db = TemporaryDB::new();
    let fork = db.fork();
    fork.get_proof_map("map").put(&1_u64, 2_u64);
    let schema = NotSchema::new(&fork, "huh?");
    assert_eq!(schema.map.get(&1_u64).unwrap(), 2);
}
