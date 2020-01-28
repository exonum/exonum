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

//! High-level access to database.
//!
//! # Overview
//!
//! The core type in this module is the [`Access`] trait, which provides ability to access
//! [indexes] from the database. The `Access` trait has several implementations:
//!
//! - `Access` is implemented for [`RawAccess`]es, that is, types that provide access to the
//!   entire database. [`Snapshot`], [`Fork`] and [`ReadonlyFork`] fall into this category.
//! - [`Prefixed`] restricts an access to a single *namespace*.
//! - [`Migration`]s are used for data created during [migrations]. Similar to `Prefixed`, migrations
//!   are separated by namespaces.
//! - [`Scratchpad`]s can be used for temporary data. They are distinguished by namespaces as well.
//!
//! [`CopyAccessExt`] extends [`Access`] and provides helper methods to instantiate indexes. This
//! is useful in quick-and-dirty testing. For more complex applications, consider deriving
//! data schema via [`FromAccess`].
//!
//! # Guarantees
//!
//! - Namespaced accesses (`Prefixed`, `Migration`s and `Scratchpad`s) do not intersect (i.e.,
//!   do not have common indexes) for different namespaces. They also do not intersect for
//!   different access types, even for the same namespace; for example, a `Prefixed` access
//!   can never access an index from a `Migration` or a `Scratchpad` and vice versa.
//! - For all listed `Access` implementations, different addresses *within* an `Access` correspond
//!   to different indexes.
//! - However, if we consider multiple accesses, indexes can alias. For example, an index
//!   with address `bar` from a `Prefixed<&Fork>` in namespace `foo` can also be accessed via
//!   address `foo.bar` from the underlying `Fork`.
//!
//! [`Access`]: trait.Access.html
//! [indexes]: ../index.html#indexes
//! [`RawAccess`]: trait.RawAccess.html
//! [`Snapshot`]: ../trait.Snapshot.html
//! [`Fork`]: ../struct.Fork.html
//! [`ReadonlyFork`]: ../struct.ReadonlyFork.html
//! [`Prefixed`]: struct.Prefixed.html
//! [`Migration`]: ../migration/struct.Migration.html
//! [migrations]: ../migration/index.html
//! [`Scratchpad`]: ../migration/struct.Scratchpad.html
//! [`CopyAccessExt`]: trait.CopyAccessExt.html
//! [`FromAccess`]: trait.FromAccess.html

use failure::{Error, Fail};

use std::fmt;

pub use self::extensions::{AccessExt, CopyAccessExt};
pub use crate::views::{AsReadonly, RawAccess, RawAccessMut};

use crate::{
    validation::assert_valid_name_component,
    views::{GroupKeys, IndexAddress, IndexMetadata, IndexType, ViewWithMetadata},
    BinaryKey,
};

mod extensions;

/// High-level access to database data.
///
/// This trait is not intended to be implemented by the types outside the crate; indeed,
/// it instantiates several crate-private types. Correspondingly, `Access` methods
/// rarely need to be used directly; use [its extension trait][`CopyAccessExt`] instead.
///
/// [`CopyAccessExt`]: trait.CopyAccessExt.html
///
/// # Examples
///
/// `Access` can be used as a bound on structured database objects and their
/// readonly methods:
///
/// ```
/// use exonum_merkledb::{access::Access, ListIndex, ProofMapIndex};
///
/// struct Schema<T: Access> {
///     list: ListIndex<T::Base, u64>,
///     map: ProofMapIndex<T::Base, str, u64>,
/// }
///
/// impl<T: Access> Schema<T> {
///     fn get_some_data(&self) -> Option<u64> {
///         Some(self.list.get(0)? + self.map.get("foo")?)
///     }
/// }
/// ```
pub trait Access: Clone {
    /// Raw access serving as the basis for created indexes.
    type Base: RawAccess;

    /// Gets index metadata at the specified address, or `None` if there is no index.
    fn get_index_metadata(self, addr: IndexAddress) -> Result<Option<IndexMetadata>, AccessError>;

    /// Gets or creates a generic view with the specified address.
    fn get_or_create_view(
        self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError>;

    /// Returns an iterator over keys in a group with the specified address.
    ///
    /// The iterator buffers keys in memory and may become inconsistent for accesses
    /// based on [`ReadonlyFork`].
    ///
    /// [`ReadonlyFork`]: ../struct.ReadonlyFork.html
    fn group_keys<K>(self, base_addr: IndexAddress) -> GroupKeys<Self::Base, K>
    where
        K: BinaryKey + ?Sized,
        Self::Base: AsReadonly<Readonly = Self::Base>;
}

impl<T: RawAccess> Access for T {
    type Base = Self;

    fn get_index_metadata(self, addr: IndexAddress) -> Result<Option<IndexMetadata>, AccessError> {
        ViewWithMetadata::get_metadata(self, &addr)
    }

    fn get_or_create_view(
        self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
        ViewWithMetadata::get_or_create(self, &addr, index_type)
    }

    fn group_keys<K>(self, base_addr: IndexAddress) -> GroupKeys<Self::Base, K>
    where
        K: BinaryKey + ?Sized,
        Self::Base: AsReadonly<Readonly = Self::Base>,
    {
        GroupKeys::new(self, &base_addr)
    }
}

/// Access that prepends the specified prefix to each created view. The prefix is separated
/// from user-provided names with a dot char `'.'`.
///
/// Since the prefix itself cannot contain a dot, `Prefixed` accesses provide namespace
/// separation. A set of indexes to which `Prefixed` provides access does not intersect
/// with a set of indexes accessed by a `Prefixed` instance with another prefix. Additionally,
/// index in `Prefixed` accesses do not intersect with indexes in special-purpose `Access`
/// implementations ([`Migration`]s and [`Scratchpad`]s).
///
/// [`Migration`]: ../migration/struct.Migration.html
/// [`Scratchpad`]: ../migration/struct.Scratchpad.html
///
/// # Examples
///
/// ```
/// use exonum_merkledb::{access::{AccessExt, CopyAccessExt, Prefixed}, Database, TemporaryDB};
///
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// let prefixed = Prefixed::new("prefixed", &fork);
/// prefixed.get_list("list").extend(vec![1_u32, 2, 3]);
/// let same_list = fork.get_list::<_, u32>("prefixed.list");
/// assert_eq!(same_list.len(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct Prefixed<T> {
    access: T,
    prefix: String,
}

// **NB.** Must not be made public! This would allow the caller to violate access restrictions
// imposed by `Prefixed`.
impl<T> Prefixed<T> {
    pub(crate) fn access(&self) -> &T {
        &self.access
    }

    pub(crate) fn into_parts(self) -> (String, T) {
        (self.prefix, self.access)
    }
}

impl<T: RawAccess> Prefixed<T> {
    /// Creates a new prefixed access.
    ///
    /// # Panics
    ///
    /// - Will panic if the prefix is not a [valid prefix name].
    ///
    /// [valid prefix name]: ../validation/fn.is_valid_index_name_component.html
    pub fn new(prefix: impl Into<String>, access: T) -> Self {
        let prefix = prefix.into();
        assert_valid_name_component(prefix.as_ref());
        Self { access, prefix }
    }
}

impl<T: RawAccess> Access for Prefixed<T> {
    type Base = T;

    fn get_index_metadata(self, addr: IndexAddress) -> Result<Option<IndexMetadata>, AccessError> {
        let prefixed_addr = addr.prepend_name(self.prefix.as_ref());
        self.access.get_index_metadata(prefixed_addr)
    }

    fn get_or_create_view(
        self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
        let prefixed_addr = addr.prepend_name(self.prefix.as_ref());
        self.access.get_or_create_view(prefixed_addr, index_type)
    }

    fn group_keys<K>(self, base_addr: IndexAddress) -> GroupKeys<Self::Base, K>
    where
        K: BinaryKey + ?Sized,
        Self::Base: AsReadonly<Readonly = Self::Base>,
    {
        let prefixed_addr = base_addr.prepend_name(self.prefix.as_ref());
        self.access.group_keys(prefixed_addr)
    }
}

/// Access error together with the location information.
#[derive(Debug, Fail)]
pub struct AccessError {
    /// Address of the index where the error has occurred.
    pub addr: IndexAddress,
    /// Error kind.
    #[fail(cause)]
    pub kind: AccessErrorKind,
}

impl fmt::Display for AccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: implement `Display` for `IndexAddress` for human-readable errors
        write!(formatter, "Error accessing {:?}: {}", self.addr, self.kind)
    }
}

/// Error that can be emitted during accessing an object from the database.
///
/// This type is not intended to be exhaustively matched. It can be extended in the future
/// without breaking the semver compatibility.
#[derive(Debug, Fail)]
pub enum AccessErrorKind {
    /// Index has wrong type.
    #[fail(
        display = "Wrong index type: expected {:?}, but got {:?}",
        expected, actual
    )]
    WrongIndexType {
        /// Expected index type.
        expected: IndexType,
        /// Actual index type.
        actual: IndexType,
    },

    /// Index name is reserved. It's forbidden for user to create indexes with names
    /// starting with `__` and not containing a dot `.`.
    #[fail(display = "Index name is reserved")]
    ReservedName,

    /// Index name is empty.
    #[fail(display = "Index name must not be empty")]
    EmptyName,

    /// Index contains invalid characters.
    #[fail(
        display = "Invalid characters used in name ({}). Use {}",
        name, allowed_chars
    )]
    InvalidCharsInName {
        /// Name that contains invalid chars.
        name: String,
        /// Characters allowed in name.
        allowed_chars: &'static str,
    },

    /// Invalid tombstone location.
    #[fail(display = "Invalid tombstone location. Tombstones can only be created in migrations")]
    InvalidTombstone,

    /// Custom error.
    #[fail(display = "{}", _0)]
    Custom(#[fail(cause)] Error),

    #[doc(hidden)]
    #[fail(display = "")] // Never actually generated.
    __NonExhaustive,
}

/// Constructs an object atop the database. The constructed object provides access to data
/// in the DB, akin to an object-relational mapping.
///
/// The access to DB can be readonly or read-write, depending on the `T: Access` type param.
/// Most object should implement `FromAccess<T>` for all `T: Access`.
///
/// Simplest `FromAccess` implementors are indexes; it is also implemented for [`Lazy`] and [`Group`].
/// `FromAccess` can be implemented for more complex *components*. Thus, `FromAccess` can
/// be used to compose storage objects from simpler ones.
///
/// [`Lazy`]: ../struct.Lazy.html
/// [`Group`]: ../indexes/group/struct.Group.html
///
/// # Examples
///
/// Component with two inner indexes. `FromAccess` is automatically derived using
/// the `exonum_derive` crate.
///
/// ```
/// use exonum_derive::FromAccess;
/// # use exonum_merkledb::{
/// #     access::{Access, CopyAccessExt, AccessError, FromAccess, RawAccessMut},
/// #     Database, Entry, Group, Lazy, MapIndex, IndexAddress, TemporaryDB,
/// # };
///
/// #[derive(FromAccess)]
/// struct InsertOnlyMap<T: Access> {
///     map: MapIndex<T::Base, str, String>,
///     len: Entry<T::Base, u64>,
/// }
///
/// impl<T: Access> InsertOnlyMap<T>
/// where
///     T::Base: RawAccessMut,
/// {
///     fn insert(&mut self, key: &str, value: String) -> bool {
///         if self.map.contains(key) { return false; }
///         self.map.put(&key.to_owned(), value);
///         self.len.set(self.len.get().unwrap_or_default() + 1);
///         true
///     }
/// }
///
/// # fn main() -> Result<(), AccessError> {
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// # {
/// let mut map = InsertOnlyMap::from_access(&fork, "test".into())?;
/// map.insert("foo", "FOO".to_owned());
/// map.insert("bar", "BAR".to_owned());
/// assert_eq!(map.len.get(), Some(2));
/// # }
///
/// // Components could be used with `Group` / `Lazy` out of the box:
/// let lazy_map: Lazy<_, InsertOnlyMap<_>> =
///     Lazy::from_access(&fork, "test".into())?;
/// assert_eq!(lazy_map.get().map.get("foo").unwrap(), "FOO");
///
/// let group_of_maps: Group<_, u16, InsertOnlyMap<_>> =
///     fork.get_group("test_group");
/// group_of_maps.get(&1).insert("baz", "BAZ".to_owned());
/// group_of_maps.get(&2).insert("baz", "BUZZ".to_owned());
/// # assert_eq!(group_of_maps.get(&1).len.get(), Some(1));
/// # assert_eq!(
/// #     fork.get_map::<_, str, String>(("test_group.map", &2_u16)).get("baz").unwrap(),
/// #     "BUZZ"
/// # );
/// # Ok(())
/// # }
/// ```
pub trait FromAccess<T: Access>: Sized {
    /// Constructs the object at the given address.
    ///
    /// # Return value
    ///
    /// Returns the constructed object. An error should be returned if the object cannot be
    /// constructed.
    fn from_access(access: T, addr: IndexAddress) -> Result<Self, AccessError>;

    /// Constructs the object from the root of the `access`.
    ///
    /// The default implementation uses `Self::from_access()` with an empty address.
    fn from_root(access: T) -> Result<Self, AccessError> {
        Self::from_access(access, IndexAddress::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, ListIndex, TemporaryDB};

    #[test]
    fn prefixed_works() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let prefixed = Prefixed::new("test", &fork);
            let mut list = prefixed.get_list::<_, i32>("foo");
            list.extend(vec![1, 2, 3]);
        }
        {
            let list = fork.get_list::<_, i32>("test.foo");
            assert_eq!(list.len(), 3);
            assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
        }
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let list = snapshot.get_list::<_, i32>("test.foo");
        assert_eq!(list.len(), 3);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);

        let prefixed = Prefixed::new("test", &snapshot);
        let list = prefixed.get_list::<_, i32>("foo");
        assert_eq!(list.len(), 3);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn prefixed_views_do_not_collide() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let foo_space = Prefixed::new("foo", &fork);
        let bar_space = Prefixed::new("bar", &fork);
        {
            let mut list = foo_space.get_list("test");
            list.push("Test".to_owned());
            let mut other_list = bar_space.get_list("test");
            other_list.extend(vec![1_u64, 2, 3]);

            assert_eq!(list.len(), 1);
            assert_eq!(other_list.len(), 3);
        }
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let foo_space = Prefixed::new("foo", &snapshot);
        let list = foo_space.get_list::<_, String>("test");
        assert_eq!(list.get(0), Some("Test".to_owned()));
        let bar_space = Prefixed::new("bar", &snapshot);
        let list = bar_space.get_list::<_, u64>("test");
        assert_eq!(list.get(0), Some(1_u64));

        // It is possible to create indexes of the different types at the same (relative) address
        // in the different `Prefixed` instances.
        let fork = db.fork();
        let foo_space = Prefixed::new("foo", &fork);
        foo_space.get_list::<_, u32>(("fam", &1_u32));
        let bar_space = Prefixed::new("bar", &fork);
        bar_space.get_proof_map::<_, u32, u32>(("fam", &1_u32));
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let view = snapshot
            .as_ref()
            .get_or_create_view(("foo.fam", &1_u32).into(), IndexType::List)
            .unwrap();
        assert!(!view.is_phantom());
        let view = snapshot
            .as_ref()
            .get_or_create_view(("bar.fam", &1_u32).into(), IndexType::ProofMap)
            .unwrap();
        assert!(!view.is_phantom());
    }

    #[test]
    fn from_root_method() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let prefixed = Prefixed::new("foo", &fork);
        {
            let mut list: ListIndex<_, u64> = ListIndex::from_root(prefixed).unwrap();
            list.extend(vec![1, 2, 3]);
        }
        assert_eq!(fork.get_list::<_, u64>("foo").len(), 3);
    }
}
