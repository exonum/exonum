use std::marker::PhantomData;

use crate::{
    access::{Access, AccessError, AccessErrorKind, Ensure, RawAccessMut, Restore},
    views::IndexAddress,
    BinaryKey,
};

/// Group of indexes distinguished by a prefix.
#[derive(Debug)]
pub struct Group<T, K: ?Sized, I> {
    access: T,
    prefix: IndexAddress,
    _key: PhantomData<K>,
    _index: PhantomData<I>,
}

impl<T, K, I> Restore<T> for Group<T, K, I>
where
    T: Access,
    K: BinaryKey + ?Sized,
    I: Restore<T>,
{
    fn restore(access: &T, addr: IndexAddress) -> Result<Self, AccessError> {
        Ok(Self {
            access: access.to_owned(),
            prefix: addr,
            _key: PhantomData,
            _index: PhantomData,
        })
    }
}

impl<T, K, I> Ensure<T> for Group<T, K, I>
where
    T: Access,
    T::Base: RawAccessMut,
    K: BinaryKey + ?Sized,
    I: Ensure<T>,
{
    fn ensure(access: &T, addr: IndexAddress) -> Result<Self, AccessError> {
        // Unlike indexes, groups don't require initialization.
        Ok(Self {
            access: access.to_owned(),
            prefix: addr,
            _key: PhantomData,
            _index: PhantomData,
        })
    }
}

impl<T, K, I> Group<T, K, I>
where
    T: Access,
    K: BinaryKey + ?Sized,
    I: Restore<T>,
{
    /// Gets an index corresponding to the specified key. If the index is not present in
    /// the storage, returns `None`.
    ///
    /// # Panics
    ///
    /// If the index is present, but has the wrong type.
    pub fn get(&self, key: &K) -> Option<I> {
        let addr = self.prefix.clone().append_bytes(key);
        match I::restore(&self.access, addr) {
            Ok(value) => Some(value),
            Err(AccessError {
                kind: AccessErrorKind::DoesNotExist,
                ..
            }) => None,
            Err(e) => panic!("{}", e),
        }
    }
}

impl<T, K, I> Group<T, K, I>
where
    T: Access,
    T::Base: RawAccessMut,
    K: BinaryKey + ?Sized,
    I: Ensure<T>,
{
    /// Gets or creates an index corresponding to the specified key.
    ///
    /// # Panics
    ///
    /// The method will panic if the retrieved index has wrong type.
    pub fn ensure(&self, key: &K) -> I {
        let addr = self.prefix.clone().append_bytes(key);
        I::ensure(&self.access, addr).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{access::AccessExt, Database, ProofListIndex, TemporaryDB};

    #[test]
    fn group() {
        let db = TemporaryDB::new();
        let fork = db.fork();

        {
            let group: Group<_, u32, ProofListIndex<_, String>> = fork.as_ref().group("group");
            let mut list = group.ensure(&1);
            list.push("foo".to_owned());
            list.push("bar".to_owned());
            group.ensure(&2).push("baz".to_owned());
        }

        {
            let list = fork
                .as_ref()
                .proof_list::<_, String>(("group", &1_u32))
                .unwrap();
            assert_eq!(list.len(), 2);
            assert_eq!(list.get(1), Some("bar".to_owned()));
            let other_list = fork
                .as_ref()
                .proof_list::<_, String>(("group", &2_u32))
                .unwrap();
            assert_eq!(other_list.len(), 1);
        }

        db.merge_sync(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        let group: Group<_, u32, ProofListIndex<_, String>> = snapshot.as_ref().group("group");
        assert_eq!(group.get(&1).unwrap().len(), 2);
        assert_eq!(group.get(&2).unwrap().len(), 1);
        assert!(group.get(&0).is_none());
    }
}
