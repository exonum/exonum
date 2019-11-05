use std::marker::PhantomData;

use crate::{
    access::{Access, AccessError, Ensure, RawAccessMut, Restore},
    views::IndexAddress,
};

/// Lazily initialized object.
#[derive(Debug)]
pub struct Lazy<T, I> {
    access: T,
    address: IndexAddress,
    _index: PhantomData<I>,
}

impl<T, I> Restore<T> for Lazy<T, I>
where
    T: Access,
    I: Restore<T>,
{
    fn restore(access: &T, addr: IndexAddress) -> Result<Self, AccessError> {
        Ok(Self {
            access: access.to_owned(),
            address: addr,
            _index: PhantomData,
        })
    }
}

impl<T, I> Ensure<T> for Lazy<T, I>
where
    T: Access,
    T::Base: RawAccessMut,
    I: Ensure<T>,
{
    fn ensure(access: &T, addr: IndexAddress) -> Result<Self, AccessError> {
        Ok(Self {
            access: access.to_owned(),
            address: addr,
            _index: PhantomData,
        })
    }
}

impl<T, I> Lazy<T, I>
where
    T: Access,
    I: Restore<T>,
{
    /// Gets the object from the database.
    ///
    /// # Panics
    ///
    /// Panics if the object cannot be restored.
    pub fn get(&self) -> I {
        self.try_get().unwrap()
    }

    /// Tries to restore the object from the database.
    pub fn try_get(&self) -> Result<I, AccessError> {
        I::restore(&self.access, self.address.clone())
    }
}

impl<T, I> Lazy<T, I>
where
    T: Access,
    T::Base: RawAccessMut,
    I: Ensure<T>,
{
    /// Gets or creates an object in the database.
    pub fn get_or_create(&self) -> I {
        I::ensure(&self.access, self.address.clone()).unwrap()
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
                Lazy::ensure(&&fork, "lazy".into()).unwrap();
            lazy_index.get_or_create().extend(vec![1, 2, 3]);
            assert_eq!(lazy_index.get().len(), 3);
            lazy_index.get().push(4);
        }
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let lazy_index: Lazy<_, ListIndex<_, u64>> =
            Lazy::restore(&&snapshot, "lazy".into()).unwrap();
        assert_eq!(
            lazy_index.get().iter().collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );

        // Note that index type is not checked on `restore` / `ensure`, so the following is valid:
        let bogus: Lazy<_, MapIndex<_, u64, String>> =
            Lazy::restore(&&snapshot, "lazy".into()).unwrap();
        // ...but this errors:
        assert_matches!(
            bogus.try_get().unwrap_err().kind,
            AccessErrorKind::WrongIndexType { actual: IndexType::List, .. }
        )
    }
}
