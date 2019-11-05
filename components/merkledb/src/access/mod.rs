//! High-level access to database.

use failure::{Error, Fail};

use std::{borrow::Cow, fmt};

pub use self::extensions::AccessExt;
pub use crate::views::{RawAccess, RawAccessMut, ToReadonly};

use crate::{
    validation::assert_valid_name,
    views::{IndexAddress, IndexType, ViewWithMetadata},
};

mod extensions;

/// High-level access to indexes.
pub trait Access: Clone {
    /// Raw access serving as the basis for created indices.
    type Base: RawAccess;

    /// Gets or creates a generic `View` with the specified address.
    fn get_or_create_view(
        &self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError>;
}

impl<T: RawAccess> Access for T {
    type Base = Self;

    fn get_or_create_view(
        &self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
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

    fn get_or_create_view(
        &self,
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
            let list = fork.as_ref().get_list::<_, i32>("test.foo");
            assert_eq!(list.len(), 3);
            assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
        }
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let list = snapshot.as_ref().get_list::<_, i32>("test.foo");
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

        // It is possible to create indexes of the different types at the same place.
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
