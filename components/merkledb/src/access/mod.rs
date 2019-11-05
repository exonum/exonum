//! High-level access to MerkleDB data.

use failure::{Error, Fail};

use std::{borrow::Cow, fmt};

pub use self::extensions::AccessExt;
pub use crate::views::{RawAccess, RawAccessMut, ToReadonly};

use crate::{
    validation::assert_valid_name,
    views::{IndexAddress, IndexType, ViewWithMetadata},
};

mod extensions;

/// Extension trait allowing for easy access to indices from any type implementing
/// `Access`.
pub trait Access: Clone {
    /// Index access serving as the basis for created indices.
    type Base: RawAccess;

    /// Gets a generic `View` with the specified address.
    fn get_view(&self, addr: IndexAddress) -> Result<ViewWithMetadata<Self::Base>, AccessError>;

    /// Gets or creates a generic `View` with the specified address.
    fn get_or_create_view(
        &self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError>
    where
        Self::Base: RawAccessMut;
}

impl<T: RawAccess> Access for T {
    type Base = Self;

    fn get_view(&self, addr: IndexAddress) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
        ViewWithMetadata::get(self.clone(), &addr).ok_or_else(|| AccessError {
            addr,
            kind: AccessErrorKind::DoesNotExist,
        })
    }

    fn get_or_create_view(
        &self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError>
    where
        Self: RawAccessMut,
    {
        ViewWithMetadata::get_or_create(self.clone(), &addr, index_type).map_err(|e| AccessError {
            addr,
            kind: AccessErrorKind::WrongIndexType {
                expected: index_type,
                actual: e.index_type(),
            },
        })
    }
}

/// Access that prepends the specified prefix to each created view.
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
        assert_valid_name(prefix.as_ref());
        Self { access, prefix }
    }
}

impl<T: Access> Access for Prefixed<'_, T> {
    type Base = T::Base;

    fn get_view(&self, addr: IndexAddress) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
        let prefixed_addr = addr.prepend_name(self.prefix.as_ref());
        self.access.get_view(prefixed_addr)
    }

    fn get_or_create_view(
        &self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError>
    where
        T::Base: RawAccessMut,
    {
        let prefixed_addr = addr.prepend_name(self.prefix.as_ref());
        self.access.get_or_create_view(prefixed_addr, index_type)
    }
}

/// Error together with location information.
#[derive(Debug, Fail)]
pub struct AccessError {
    /// Address of the index where the error has occurred relative to the `root`.
    pub addr: IndexAddress,
    /// Error kind.
    #[fail(cause)]
    pub kind: AccessErrorKind,
}

impl fmt::Display for AccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Error accessing {:?}: {}", self.addr, self.kind)
    }
}

/// Error that can be emitted during accessing an object from the database.
#[derive(Debug, Fail)]
pub enum AccessErrorKind {
    /// Index does not exist.
    #[fail(display = "Index does not exist")]
    DoesNotExist,

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

    /// Custom error.
    #[fail(display = "{}", _0)]
    Custom(#[fail(cause)] Error),
}

/// Restores an object from the database.
pub trait Restore<T: Access>: Sized {
    /// Restores the object at the given address.
    ///
    /// # Return value
    ///
    /// An error should be returned if the object cannot be restored.
    fn restore(access: &T, addr: IndexAddress) -> Result<Self, AccessError>;
}

/// Ensures that the object is in the database, creating it if necessary.
pub trait Ensure<T: Access>: Sized {
    /// Ensures that the object is in the database. If the object is not in the database,
    /// it should be created by this method.
    fn ensure(access: &T, addr: IndexAddress) -> Result<Self, AccessError>;
}

pub(crate) fn restore_view<T: Access>(
    access: &T,
    addr: IndexAddress,
    expected_type: IndexType,
) -> Result<ViewWithMetadata<T::Base>, AccessError> {
    let view = access.get_view(addr.clone())?;
    if view.index_type() != expected_type {
        return Err(AccessError {
            addr,
            kind: AccessErrorKind::WrongIndexType {
                expected: expected_type,
                actual: view.index_type(),
            },
        });
    }
    Ok(view)
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
            let mut list = prefixed.ensure_list::<_, i32>("foo");
            list.extend(vec![1, 2, 3]);
        }
        {
            let list = fork.as_ref().list::<_, i32>("test.foo").unwrap();
            assert_eq!(list.len(), 3);
            assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
        }
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let list = snapshot.as_ref().list::<_, i32>("test.foo").unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);

        let prefixed = Prefixed::new("test", &snapshot);
        let list = prefixed.list::<_, i32>("foo").unwrap();
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
            let mut list = foo_space.ensure_list("test");
            list.push("Test".to_owned());
            let mut other_list = bar_space.ensure_list("test");
            other_list.extend(vec![1_u64, 2, 3]);

            assert_eq!(list.len(), 1);
            assert_eq!(other_list.len(), 3);
        }
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let foo_space = Prefixed::new("foo", &snapshot);
        let list = foo_space.list::<_, String>("test").unwrap();
        assert_eq!(list.get(0), Some("Test".to_owned()));
        let bar_space = Prefixed::new("bar", &snapshot);
        let list = bar_space.list::<_, u64>("test").unwrap();
        assert_eq!(list.get(0), Some(1_u64));

        // It is possible to create indexes of the different types at the same place.
        let fork = db.fork();
        let foo_space = Prefixed::new("foo", &fork);
        foo_space.ensure_type(("fam", &1_u32), IndexType::List);
        let bar_space = Prefixed::new("bar", &fork);
        bar_space.ensure_type(("fam", &1_u32), IndexType::ProofMap);
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let view = snapshot
            .as_ref()
            .get_view(("foo.fam", &1_u32).into())
            .unwrap();
        assert_eq!(view.index_type(), IndexType::List);
        let view = snapshot
            .as_ref()
            .get_view(("bar.fam", &1_u32).into())
            .unwrap();
        assert_eq!(view.index_type(), IndexType::ProofMap);
    }
}
