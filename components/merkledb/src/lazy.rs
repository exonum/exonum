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

use std::marker::PhantomData;

use crate::{
    access::{Access, AccessError, FromAccess},
    views::IndexAddress,
};

/// Lazily initialized object in the database.
///
/// Unlike eagerly initialized objects, lazy ones are not accessed until a [`get()`] method
/// is called; thus, construction of a lazy object is cheap.
/// This can be used to improve performance of a database object, some components of which
/// are rarely accessed.
///
/// Note that [`Group`]s are already lazy, so it does not make sense to wrap a one into `Lazy<_>`
/// (although this is technically possible).
///
/// # Examples
///
/// ```
/// # use exonum_merkledb::{access::{CopyAccessExt, FromAccess}, Database, Lazy, ListIndex, TemporaryDB};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// {
///     let lazy: Lazy<_, ListIndex<_, String>> =
///         Lazy::from_access(&fork, "lazy_list".into()).unwrap();
///     lazy.get().push("!".to_owned());
///     assert_eq!(lazy.get().len(), 1);
/// }
/// // List can then be accessed eagerly.
/// assert_eq!(
///     fork.get_list::<_, String>("lazy_list").get(0),
///     Some("!".to_owned())
/// );
/// ```
///
/// [`get()`]: #method.get
/// [`Group`]: indexes/group/struct.Group.html
#[derive(Debug)]
pub struct Lazy<T, I> {
    access: T,
    address: IndexAddress,
    _index: PhantomData<I>,
}

impl<T, I> FromAccess<T> for Lazy<T, I>
where
    T: Access,
    I: FromAccess<T>,
{
    fn from_access(access: T, addr: IndexAddress) -> Result<Self, AccessError> {
        Ok(Self {
            access,
            address: addr,
            _index: PhantomData,
        })
    }
}

impl<T, I> Lazy<T, I>
where
    T: Access,
    I: FromAccess<T>,
{
    /// Gets the object from the database.
    ///
    /// # Panics
    ///
    /// Panics if the object cannot be restored.
    pub fn get(&self) -> I {
        self.try_get()
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Tries to restore the object from the database.
    pub fn try_get(&self) -> Result<I, AccessError> {
        I::from_access(self.access.clone(), self.address.clone())
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;

    use super::*;
    use crate::{access::AccessErrorKind, Database, IndexType, ListIndex, MapIndex, TemporaryDB};

    #[test]
    fn lazy_initialization() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let lazy_index: Lazy<_, ListIndex<_, u64>> =
                Lazy::from_access(&fork, "lazy".into()).unwrap();
            lazy_index.get().extend(vec![1, 2, 3]);
            assert_eq!(lazy_index.get().len(), 3);
            lazy_index.get().push(4);
        }
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let lazy_index: Lazy<_, ListIndex<_, u64>> =
            Lazy::from_access(&snapshot, "lazy".into()).unwrap();
        assert_eq!(
            lazy_index.get().iter().collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );

        // Note that index type is not checked on `restore` / `ensure`, so the following is valid:
        let bogus: Lazy<_, MapIndex<_, u64, String>> =
            Lazy::from_access(&snapshot, "lazy".into()).unwrap();
        // ...but this errors:
        assert_matches!(
            bogus.try_get().unwrap_err().kind,
            AccessErrorKind::WrongIndexType { actual: IndexType::List, .. }
        )
    }
}
