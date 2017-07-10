//! An implementation of index that may only contain one element.
use std::marker::PhantomData;

use crypto::Hash;

use super::{BaseIndex, Snapshot, Fork, StorageValue};

/// An index that may only contain one element.

/// A value should implement [`StorageValue`] trait.
/// [`StorageValue`]: ../trait.StorageValue.html
#[derive(Debug)]
pub struct Entry<T, V> {
    base: BaseIndex<T>,
    _v: PhantomData<V>,
}

impl<T, V> Entry<T, V> {
    /// Creates a new index representation based on the prefix and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    pub fn new(prefix: Vec<u8>, view: T) -> Self {
        Entry {
            base: BaseIndex::new(prefix, view),
            _v: PhantomData,
        }
    }
}

impl<T, V> Entry<T, V>
    where T: AsRef<Snapshot>,
          V: StorageValue
{
    /// Returns a value of the entry or `None` if not exists.
    pub fn get(&self) -> Option<V> {
        self.base.get(&())
    }

    /// Returns `true` if a value of the entry exists.
    pub fn exists(&self) -> bool {
        self.base.contains(&())
    }

    /// Returns hash of the entry or default hash value if not exists.
    pub fn hash(&self) -> Hash {
        self.base
            .get::<(), V>(&())
            .map(|v| v.hash())
            .unwrap_or_default()
    }
}

impl<'a, V> Entry<&'a mut Fork, V>
    where V: StorageValue
{
    /// Changes a value of the entry.
    pub fn set(&mut self, value: V) {
        self.base.put(&(), value)
    }

    /// Removes a value of the entry.
    pub fn remove(&mut self) {
        self.base.remove(&())
    }
}
