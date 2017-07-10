//! An implementation of array list of items.
use std::cell::Cell;
use std::marker::PhantomData;

use super::{BaseIndex, BaseIndexIter, Snapshot, Fork, StorageValue};

/// A list of items that implemets `StorageValue` trait.
///
/// `ListIndex` implements an array list, storing the element as values and using `u64` as an index.
/// `ListIndex` requires that the elements implement the [`StorageValue`] trait.
/// [`StorageValue`]: ../trait.StorageValue.html
#[derive(Debug)]
pub struct ListIndex<T, V> {
    base: BaseIndex<T>,
    length: Cell<Option<u64>>,
    _v: PhantomData<V>,
}

/// An iterator over the items of a `ListIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] methods on [`ListIndex`]. See its documentation for more.
///
/// [`iter`]: struct.ListIndex.html#method.iter
/// [`iter_from`]: struct.ListIndex.html#method.iter_from
/// [`ListIndex`]: struct.ListIndex.html
#[derive(Debug)]
pub struct ListIndexIter<'a, V> {
    base_iter: BaseIndexIter<'a, u64, V>,
}

impl<T, V> ListIndex<T, V> {
    /// Creates a new index representation based on the common prefix of its keys and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    pub fn new(prefix: Vec<u8>, view: T) -> Self {
        ListIndex {
            base: BaseIndex::new(prefix, view),
            length: Cell::new(None),
            _v: PhantomData,
        }
    }
}

impl<T, V> ListIndex<T, V>
    where T: AsRef<Snapshot>,
          V: StorageValue
{
    /// Returns an element at that position or `None` if out of bounds.
    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&index)
    }

    /// Returns the last element of the list, or `None` if it is empty.
    pub fn last(&self) -> Option<V> {
        match self.len() {
            0 => None,
            l => self.get(l - 1),
        }
    }

    /// Returns `true` if the list contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of elements in the list.
    pub fn len(&self) -> u64 {
        if let Some(len) = self.length.get() {
            return len;
        }
        let len = self.base.get(&()).unwrap_or(0);
        self.length.set(Some(len));
        len
    }

    /// Returns an iterator over the list. The iterator element type is V.
    pub fn iter(&self) -> ListIndexIter<V> {
        ListIndexIter { base_iter: self.base.iter_from(&(), &0u64) }
    }

    /// Returns an iterator over the list starting from the specified position. The iterator
    /// element type is V.
    pub fn iter_from(&self, from: u64) -> ListIndexIter<V> {
        ListIndexIter { base_iter: self.base.iter_from(&(), &from) }
    }
}

impl<'a, V> ListIndex<&'a mut Fork, V>
    where V: StorageValue
{
    fn set_len(&mut self, len: u64) {
        self.base.put(&(), len);
        self.length.set(Some(len));
    }

    /// Appends an element to the back of the list.
    pub fn push(&mut self, value: V) {
        let len = self.len();
        self.base.put(&len, value);
        self.set_len(len + 1)
    }

    /// Removes the last element from the list and returns it, or None if it is empty.
    pub fn pop(&mut self) -> Option<V> {
        // TODO: shoud we get and return dropped value?
        match self.len() {
            0 => None,
            l => {
                let v = self.base.get(&(l - 1));
                self.base.remove(&(l - 1));
                self.set_len(l - 1);
                v
            }
        }
    }

    /// Extends the list with the contents of an iterator.
    pub fn extend<I>(&mut self, iter: I)
        where I: IntoIterator<Item = V>
    {
        let mut len = self.len();
        for value in iter {
            self.base.put(&len, value);
            len += 1;
        }
        self.base.put(&(), len);
        self.set_len(len);
    }

    /// Shortens the list, keeping the first `len` elements and dropping the rest.
    ///
    /// If `len` is greater than the list's current length, this has no effect.
    pub fn truncate(&mut self, len: u64) {
        // TODO: optimize this
        while self.len() > len {
            self.pop();
        }
    }

    /// Changes a value at specified position.
    ///
    /// # Panics
    /// Panics if `index` is equal or greater than the list's current length.
    pub fn set(&mut self, index: u64, value: V) {
        if index >= self.len() {
            panic!("index out of bounds: \
                    the len is {} but the index is {}",
                   self.len(),
                   index);
        }
        self.base.put(&index, value)
    }

    /// Clears the list, removing all values.
    ///
    /// # Notes
    /// Currently this method is not optimized to delete large set of data. During the execution of
    /// this method the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    pub fn clear(&mut self) {
        self.length.set(Some(0));
        self.base.clear()
    }
}

impl<'a, T, V> ::std::iter::IntoIterator for &'a ListIndex<T, V>
    where T: AsRef<Snapshot>,
          V: StorageValue
{
    type Item = V;
    type IntoIter = ListIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for ListIndexIter<'a, V>
    where V: StorageValue
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(.., v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::ListIndex;
    use super::super::{MemoryDB, Database};

    #[test]
    fn test_list_index_methods() {
        let mut fork = MemoryDB::new().fork();
        let mut list_index = ListIndex::new(vec![255], &mut fork);

        assert!(list_index.is_empty());
        assert_eq!(0, list_index.len());
        assert!(list_index.last().is_none());

        let extended_by = vec![45, 3422, 234];
        list_index.extend(extended_by);
        assert!(!list_index.is_empty());
        assert_eq!(Some(45), list_index.get(0));
        assert_eq!(Some(3422), list_index.get(1));
        assert_eq!(Some(234), list_index.get(2));
        assert_eq!(3, list_index.len());

        list_index.set(2, 777);
        assert_eq!(Some(777), list_index.get(2));
        assert_eq!(Some(777), list_index.last());
        assert_eq!(3, list_index.len());

        let mut extended_by_again = vec![666, 999];
        for el in &extended_by_again {
            list_index.push(*el);
        }
        assert_eq!(Some(666), list_index.get(3));
        assert_eq!(Some(999), list_index.get(4));
        assert_eq!(5, list_index.len());
        extended_by_again[1] = 1001;
        list_index.extend(extended_by_again);
        assert_eq!(7, list_index.len());
        assert_eq!(Some(1001), list_index.last());

        assert_eq!(Some(1001), list_index.pop());
        assert_eq!(6, list_index.len());

        list_index.truncate(3);

        assert_eq!(3, list_index.len());
        assert_eq!(Some(777), list_index.last());
    }

    #[test]
    fn test_list_index_iter() {
        let mut fork = MemoryDB::new().fork();
        let mut list_index = ListIndex::new(vec![255], &mut fork);

        list_index.extend(vec![1u8, 2, 3]);

        assert_eq!(list_index.iter().collect::<Vec<u8>>(), vec![1, 2, 3]);

        assert_eq!(list_index.iter_from(0).collect::<Vec<u8>>(), vec![1, 2, 3]);
        assert_eq!(list_index.iter_from(1).collect::<Vec<u8>>(), vec![2, 3]);
        assert_eq!(list_index.iter_from(3).collect::<Vec<u8>>(),
                   Vec::<u8>::new());
    }
}
