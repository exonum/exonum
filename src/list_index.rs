// Copyright 2019 The Exonum Team
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

//! An implementation of an array list of items.
//!
//! The given section contains methods related to `ListIndex` and the iterator
//! over the items of this list.

use std::marker::PhantomData;

use crate::{
    views::{
        IndexAccess, IndexAddress, IndexBuilder, IndexState, IndexType, Iter as ViewIter, View,
    },
    BinaryKey, BinaryValue,
};

/// A list of items where elements are added to the end of the list and are
/// removed starting from the end of the list.
///
/// Access to the elements is obtained using the indices of the list items.
/// `ListIndex` implements an array list, storing the elements as values and
/// using `u64` as an index. `ListIndex` requires that elements implement the
/// [`BinaryValue`] trait.
///
/// [`BinaryValue`]: ../trait.BinaryValue.html
#[derive(Debug)]
pub struct ListIndex<T: IndexAccess, V> {
    base: View<T>,
    state: IndexState<T, u64>,
    _v: PhantomData<V>,
}

/// Returns an iterator over the items of a `ListIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] method on [`ListIndex`]. See its documentation for details.
///
/// [`iter`]: struct.ListIndex.html#method.iter
/// [`iter_from`]: struct.ListIndex.html#method.iter_from
/// [`ListIndex`]: struct.ListIndex.html
#[derive(Debug)]
pub struct ListIndexIter<'a, V> {
    base_iter: ViewIter<'a, u64, V>,
}

impl<T, V> ListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    /// Creates a new index representation based on the name and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case, only
    /// immutable methods are available. In the second case, both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ListIndex<_, u8> = ListIndex::new(name, &snapshot);
    /// ```
    pub fn new<S: Into<String>>(index_name: S, index_access: T) -> Self {
        let (base, state) = IndexBuilder::new(index_access)
            .index_type(IndexType::List)
            .index_name(index_name)
            .build();

        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    /// Creates a new index representation based on the name, index ID in family
    /// and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case, only
    /// immutable methods are available. In the second case, both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let index_id = vec![01];
    /// let snapshot = db.snapshot();
    /// let index: ListIndex<_, u8> = ListIndex::new_in_family(name, &index_id, &snapshot);
    /// ```
    pub fn new_in_family<S, I>(family_name: S, index_id: &I, index_access: T) -> Self
    where
        I: BinaryKey,
        I: ?Sized,
        S: Into<String>,
    {
        let (base, state) = IndexBuilder::new(index_access)
            .index_type(IndexType::List)
            .index_name(family_name)
            .family_id(index_id)
            .build();

        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    pub fn create_from_address<I: Into<IndexAddress>>(
        address: I,
        index_access: T,
    ) -> Result<Self, failure::Error> {
        let address = address.into();
        let (base, state) = IndexBuilder::from_address(address.clone(), index_access)
            .index_type(IndexType::List)
            .build_new()?;

        Ok(Self {
            base,
            state,
            _v: PhantomData,
        })
    }

    //TODO: allow to create indexes without address.
    pub fn create(index_access: T) -> Result<Self, failure::Error> {
        Self::create_from_address(IndexAddress::default(), index_access)
    }

    pub fn get_from_view(view: View<T>) -> Result<Self, failure::Error> {
        let (base, state) = IndexBuilder::from_view(view)
            .index_type(IndexType::List)
            .build_existed()?;

        Ok(Self {
            base,
            state,
            _v: PhantomData,
        })
    }

    pub fn address(&self) -> &IndexAddress {
        &self.base.address
    }

    /// Returns an element at the indicated position or `None` if the indicated
    /// position is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    /// assert_eq!(None, index.get(0));
    ///
    /// index.push(42);
    /// assert_eq!(Some(42), index.get(0));
    /// ```
    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&index)
    }

    /// Returns the last element of the list or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    /// assert_eq!(None, index.last());
    ///
    /// index.push(42);
    /// assert_eq!(Some(42), index.last());
    /// ```
    pub fn last(&self) -> Option<V> {
        match self.len() {
            0 => None,
            l => self.get(l - 1),
        }
    }

    /// Returns `true` if the list contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    /// assert!(index.is_empty());
    ///
    /// index.push(42);
    /// assert!(!index.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of elements in the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    /// assert_eq!(0, index.len());
    ///
    /// index.push(10);
    /// assert_eq!(1, index.len());
    ///
    /// index.push(100);
    /// assert_eq!(2, index.len());
    /// ```
    pub fn len(&self) -> u64 {
        self.state.get()
    }

    /// Returns an iterator over the list. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// for val in index.iter() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter(&self) -> ListIndexIter<V> {
        ListIndexIter {
            base_iter: self.base.iter_from(&(), &0_u64),
        }
    }

    /// Returns an iterator over the list starting from the specified position. The iterator
    /// element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// for val in index.iter_from(3) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: u64) -> ListIndexIter<V> {
        ListIndexIter {
            base_iter: self.base.iter_from(&(), &from),
        }
    }

    /// Appends an element to the back of the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    /// ```
    pub fn push(&mut self, value: V) {
        let len = self.len();
        self.base.put(&len, value);
        self.set_len(len + 1)
    }

    /// Removes the last element from the list and returns it, or returns `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    /// assert_eq!(None, index.pop());
    ///
    /// index.push(1);
    /// assert_eq!(Some(1), index.pop());
    /// ```
    pub fn pop(&mut self) -> Option<V> {
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
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    /// assert!(index.is_empty());
    ///
    /// index.extend([1, 2, 3].iter().cloned());
    /// assert_eq!(3, index.len());
    /// ```
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = V>,
    {
        let mut len = self.len();
        for value in iter {
            self.base.put(&len, value);
            len += 1;
        }
        self.base.put(&(), len);
        self.set_len(len);
    }

    /// Shortens the list, keeping the indicated number of first `len` elements
    /// and dropping the rest.
    ///
    /// If `len` is greater than the current state of the list, this has no effect.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    /// assert_eq!(5, index.len());
    ///
    /// index.truncate(3);
    /// assert_eq!(3, index.len());
    /// ```
    pub fn truncate(&mut self, len: u64) {
        // TODO: Optimize this. (ECR-175)
        while self.len() > len {
            self.pop();
        }
    }

    /// Changes a value at the specified position.
    ///
    /// # Panics
    ///
    /// Panics if the indicated position (`index`) is equal to or greater than
    /// the current state of the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    ///
    /// index.push(1);
    /// assert_eq!(Some(1), index.get(0));
    ///
    /// index.set(0, 10);
    /// assert_eq!(Some(10), index.get(0));
    /// ```
    pub fn set(&mut self, index: u64, value: V) {
        if index >= self.len() {
            panic!(
                "index out of bounds: \
                 the len is {} but the index is {}",
                self.len(),
                index
            );
        }
        self.base.put(&index, value)
    }

    /// Clears the list, removing all values.
    ///
    /// # Notes
    ///
    /// Currently, this method is not optimized to delete a large set of data. During the execution of
    /// this method, the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ListIndex::new(name, &fork);
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    ///
    /// index.clear();
    /// assert!(index.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.base.clear();
        self.state.clear();
    }

    fn set_len(&mut self, len: u64) {
        self.state.set(len)
    }
}

impl<'a, T, V> ::std::iter::IntoIterator for &'a ListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    type Item = V;
    type IntoIter = ListIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for ListIndexIter<'a, V>
where
    V: BinaryValue,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(.., v)| v)
    }
}

//TODO: revert tests
#[cfg(test_test)]
mod tests {
    use crate::{list_index::ListIndex, views::IndexAccess, Database, Fork, TemporaryDB};

    use std::string::String;

    fn list_index_methods(list_index: &mut ListIndex<&Fork, i32>) {
        assert!(list_index.is_empty());
        assert_eq!(0, list_index.len());
        assert!(list_index.last().is_none());
        assert_eq!(None, list_index.pop());

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

        list_index.clear();
        assert_eq!(0, list_index.len());
    }

    fn list_index_iter(list_index: &mut ListIndex<&Fork, u8>) {
        list_index.extend(vec![1u8, 2, 3]);

        assert_eq!(list_index.iter().collect::<Vec<u8>>(), vec![1, 2, 3]);

        assert_eq!(list_index.iter_from(0).collect::<Vec<u8>>(), vec![1, 2, 3]);
        assert_eq!(list_index.iter_from(1).collect::<Vec<u8>>(), vec![2, 3]);
        assert_eq!(
            list_index.iter_from(3).collect::<Vec<u8>>(),
            Vec::<u8>::new()
        );
    }

    fn list_index_clear_in_family(db: &dyn Database, x: u32, y: u32, merge_before_clear: bool) {
        assert_ne!(x, y);
        let mut fork = db.fork();

        fn list<T>(index: u32, view: T) -> ListIndex<T, String>
        where
            T: IndexAccess,
        {
            ListIndex::new_in_family("family", &index, view)
        }

        // Write data to both indexes.
        {
            let mut index = list(x, &fork);
            index.push("foo".to_owned());
            index.push("bar".to_owned());
        }
        {
            let mut index = list(y, &fork);
            index.push("baz".to_owned());
            index.push("qux".to_owned());
        }

        if merge_before_clear {
            db.merge_sync(fork.into_patch()).expect("merge");
            fork = db.fork();
        }

        // Clear the index with the lower family key.
        {
            let mut index = list(x, &fork);
            index.clear();
        }

        // The other index should be unaffected.
        {
            let index = list(x, &fork);
            assert!(index.is_empty());
            let index = list(y, &fork);
            assert_eq!(
                index.iter().collect::<Vec<_>>(),
                vec!["baz".to_owned(), "qux".to_owned()]
            );
        }

        // ...even after fork merge.
        db.merge_sync(fork.into_patch()).expect("merge");
        let snapshot = db.snapshot();
        let index = list(x, &snapshot);
        assert!(index.is_empty());
        let index = list(y, &snapshot);
        assert_eq!(
            index.iter().collect::<Vec<_>>(),
            vec!["baz".to_owned(), "qux".to_owned()]
        );
    }

    // Parameters for the `list_index_clear_in_family` test.
    const FAMILY_CLEAR_PARAMS: &[(u32, u32, bool)] =
        &[(0, 5, false), (5, 0, false), (1, 7, true), (7, 1, true)];

    const IDX_NAME: &'static str = "idx_name";

    #[test]
    fn test_list_index_methods() {
        let db = TemporaryDB::default();
        let fork = db.fork();
        let mut list_index = ListIndex::new(IDX_NAME, &fork);
        list_index_methods(&mut list_index);
    }

    #[test]
    fn test_list_index_in_family_methods() {
        let db = TemporaryDB::default();
        let fork = db.fork();
        let mut list_index = ListIndex::new_in_family(IDX_NAME, &vec![01], &fork);
        list_index_methods(&mut list_index);
    }

    #[test]
    fn test_list_index_iter() {
        let db = TemporaryDB::default();
        let fork = db.fork();
        let mut list_index = ListIndex::new(IDX_NAME, &fork);
        list_index_iter(&mut list_index);
    }

    #[test]
    fn test_list_index_in_family_iter() {
        let db = TemporaryDB::default();
        let fork = db.fork();
        let mut list_index = ListIndex::new_in_family(IDX_NAME, &vec![01], &fork);
        list_index_iter(&mut list_index);
    }

    #[test]
    fn test_list_index_clear_in_family() {
        for &(x, y, merge_before_clear) in FAMILY_CLEAR_PARAMS {
            let db = TemporaryDB::default();
            list_index_clear_in_family(&db, x, y, merge_before_clear);
        }
    }
}
