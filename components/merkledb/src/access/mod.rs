//! High-level access to database.

use failure::{Error, Fail};

use std::{borrow::Cow, fmt};

pub use self::extensions::AccessExt;
pub use crate::views::{AsReadonly, RawAccess, RawAccessMut};

use crate::validation::assert_valid_name_component;
use crate::views::{IndexAddress, IndexType, ViewWithMetadata};

mod extensions;

/// High-level access to database data.
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
///     map: ProofMapIndex<T::Base, String, u64>,
/// }
///
/// impl<T: Access> Schema<T> {
///     fn get_some_data(&self) -> Option<u64> {
///         Some(self.list.get(0)? + self.map.get(&"foo".to_owned())?)
///     }
/// }
/// ```
pub trait Access: Clone {
    /// Raw access serving as the basis for created indices.
    type Base: RawAccess;

    /// Gets or creates a generic `View` with the specified address.
    fn get_or_create_view(
        self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError>;
}

impl<T: RawAccess> Access for T {
    type Base = Self;

    fn get_or_create_view(
        self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
        ViewWithMetadata::get_or_create(self, &addr, index_type)
    }
}

/// Access that prepends the specified prefix to each created view.
///
/// # Examples
///
/// ```
/// use exonum_merkledb::{access::{AccessExt, Prefixed}, Database, TemporaryDB};
///
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// let prefixed = Prefixed::new("prefixed", &fork);
/// prefixed.get_list("list").extend(vec![1_u32, 2, 3]);
/// let same_list = fork.get_list::<_, u32>("prefixed.list");
/// assert_eq!(same_list.len(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct Prefixed<'a, T> {
    access: T,
    prefix: Cow<'a, str>,
}

impl<'a, T: Access> Prefixed<'a, T> {
    /// Creates new prefixed access.
    ///
    /// # Panics
    ///
    /// Will panic if the prefix does not conform to valid names for indexes.
    pub fn new(prefix: impl Into<Cow<'a, str>>, access: T) -> Self {
        let prefix = prefix.into();
        assert_valid_name_component(prefix.as_ref());
        Self { access, prefix }
    }
}

impl<T: Access> Access for Prefixed<'_, T> {
    type Base = T::Base;

    fn get_or_create_view(
        self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
        let prefixed_addr = addr.prepend_name(self.prefix.as_ref());
        self.access.get_or_create_view(prefixed_addr, index_type)
    }
}

/// Error together with location information.
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

    /// Index has invalid name.
    #[fail(display = "{}", _0)]
    InvalidIndexName(String),

    /// System index has invalid name.
    #[fail(display = "{}", _0)]
    InvalidSystemIndexName(String),

    /// Custom error.
    #[fail(display = "{}", _0)]
    Custom(#[fail(cause)] Error),
}

/// Constructs an object atop the database. The constructed object provides access to data
/// in the DB, akin to an object-relational mapping.
///
/// The access to DB can be readonly or read-write, depending on the `T: Access` type param.
/// Most object should implement `FromAccess<T>` for all `T: Access`, unless there are compelling
/// reasons not to.
///
/// Simplest `FromAccess` implementors are indexes; it is implemented for [`Lazy`] and [`Group`].
/// `FromAccess` can be implemented for more complex *components*. Thus, `FromAccess` can
/// be used to compose storage objects from simpler ones.
///
/// [`Lazy`]: ../struct.Lazy.html
/// [`Group`]: ../struct.Group.html
///
/// # Examples
///
/// Component with two inner indexes.
///
/// ```
/// # use exonum_merkledb::{
/// #     access::{Access, AccessExt, AccessError, FromAccess, RawAccessMut},
/// #     Database, Entry, Group, Lazy, MapIndex, IndexAddress, TemporaryDB,
/// # };
/// struct InsertOnlyMap<T: Access> {
///     map: MapIndex<T::Base, String, String>,
///     len: Entry<T::Base, u64>,
/// }
///
/// impl<T: Access> FromAccess<T> for InsertOnlyMap<T> {
///     fn from_access(access: T, addr: IndexAddress) -> Result<Self, AccessError> {
///         Ok(Self {
///             map: FromAccess::from_access(access.clone(), addr.clone().append_name("map"))?,
///             len: FromAccess::from_access(access, addr.append_name("len"))?,
///         })
///     }
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
/// #     fork.get_map::<_, String, String>(("test_group.map", &2_u16)).get("baz").unwrap(),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, TemporaryDB};

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
        foo_space
            .touch_index(("fam", &1_u32), IndexType::List)
            .unwrap();
        let bar_space = Prefixed::new("bar", &fork);
        bar_space
            .touch_index(("fam", &1_u32), IndexType::ProofMap)
            .unwrap();
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
}
