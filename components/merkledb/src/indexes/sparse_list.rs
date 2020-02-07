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

//! An implementation of an array list of items with spaces.
//!
//! The given section contains methods related to `SparseListIndex` and iterators
//! over the items of this index.

use std::{io::Error, marker::PhantomData};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::{
    access::{Access, AccessError, FromAccess},
    indexes::iter::{Entries, IndexIterator, Keys, Values},
    views::{
        BinaryAttribute, IndexAddress, IndexState, IndexType, RawAccess, RawAccessMut, View,
        ViewWithMetadata,
    },
    BinaryValue,
};

#[derive(Debug, Default, Clone, Copy)]
struct SparseListSize {
    /// Total list's length including spaces. In fact points to the next index for a new element.
    capacity: u64,
    /// Amount of non-empty elements.
    length: u64,
}

impl BinaryAttribute for SparseListSize {
    fn size(&self) -> usize {
        16
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        buffer.write_u64::<LittleEndian>(self.capacity).unwrap();
        buffer.write_u64::<LittleEndian>(self.length).unwrap();
    }

    fn read(mut buffer: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            capacity: buffer.read_u64::<LittleEndian>()?,
            length: buffer.read_u64::<LittleEndian>()?,
        })
    }
}

/// A list of items similar to `ListIndex`; however, it may contain "spaces". For instance,
/// a list might contain six elements with indexes: `1, 2, 3, 5, 7, 8` (missing 4 and 6). And if you
/// try to get the element for index 4 or 6, you will get `None`.
///
/// Later, elements can be added to the
/// spaces, if required. Elements in this list are added to the end of the list and are
/// removed either from the end of the list or from certain indexes.
///
/// `SparseListIndex` has length and capacity. Length is the number of non-empty
/// elements in the list. Capacity is the number of all elements in the list, both
/// empty and non-empty.
///
/// `SparseListIndex` implements an array list, storing an element as a value and using `u64`
/// as an index.
/// `SparseListIndex` requires that elements should implement the [`BinaryValue`] trait.
///
/// [`BinaryValue`]: ../trait.BinaryValue.html
#[derive(Debug)]
pub struct SparseListIndex<T: RawAccess, V> {
    base: View<T>,
    state: IndexState<T, SparseListSize>,
    _v: PhantomData<V>,
}

impl<T, V> FromAccess<T> for SparseListIndex<T::Base, V>
where
    T: Access,
    V: BinaryValue,
{
    fn from_access(access: T, addr: IndexAddress) -> Result<Self, AccessError> {
        let view = access.get_or_create_view(addr, IndexType::SparseList)?;
        Ok(Self::new(view))
    }
}

impl<T, V> SparseListIndex<T, V>
where
    T: RawAccess,
    V: BinaryValue,
{
    fn new(view: ViewWithMetadata<T>) -> Self {
        let (base, state) = view.into_parts();
        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    fn size(&self) -> SparseListSize {
        self.state.get().unwrap_or_default()
    }

    /// Returns an element at the indicated position or `None` if the indicated
    /// position is out of bounds or if it does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    /// assert_eq!(None, index.get(0));
    ///
    /// index.push(42);
    /// assert_eq!(Some(42), index.get(0));
    /// index.push(1);
    /// index.remove(0);
    /// assert_eq!(None, index.get(0));
    /// assert_eq!(Some(1), index.get(1));
    /// ```
    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&index)
    }

    /// Returns `true` if the list contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    /// assert!(index.is_empty());
    ///
    /// index.push(42);
    /// assert!(!index.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the total amount of elements, including empty elements, in the list. The value of
    /// capacity is determined by the maximum index of an element ever inserted into the given index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    /// assert_eq!(0, index.capacity());
    ///
    /// index.push(10);
    /// index.push(12);
    /// assert_eq!(2, index.capacity());
    ///
    /// index.remove(0);
    ///
    /// index.push(100);
    /// assert_eq!(3, index.capacity());
    /// ```
    pub fn capacity(&self) -> u64 {
        self.size().capacity
    }

    /// Returns the total amount of non-empty elements in the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    /// assert_eq!(0, index.len());
    ///
    /// index.push(10);
    /// assert_eq!(1, index.len());
    ///
    /// index.remove(0);
    ///
    /// index.push(100);
    /// assert_eq!(1, index.len());
    /// ```
    pub fn len(&self) -> u64 {
        self.size().length
    }

    /// Returns an iterator over the list elements with corresponding indexes.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// for val in index.iter() {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn iter(&self) -> Entries<'_, u64, V> {
        self.index_iter(None)
    }

    /// Returns an iterator over the indexes of the `SparseListIndex`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// for val in index.indexes() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn indexes(&self) -> Keys<'_, u64> {
        self.iter().skip_values()
    }

    /// Returns an iterator over list elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// for val in index.values() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values(&self) -> Values<'_, V> {
        self.iter().skip_keys()
    }

    /// Returns an iterator over the list elements starting from the specified position. Elements
    /// are yielded with the corresponding index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    /// index.remove(3);
    ///
    /// for val in index.iter_from(3) {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: u64) -> Entries<'_, u64, V> {
        self.index_iter(Some(&from))
    }
}

impl<T, V> SparseListIndex<T, V>
where
    T: RawAccessMut,
    V: BinaryValue,
{
    /// Appends an element to the back of the `SparseListIndex`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    /// ```
    pub fn push(&mut self, value: V) {
        let mut size = self.size();
        self.base.put(&size.capacity, value);
        size.capacity += 1;
        size.length += 1;
        self.set_size(size);
    }

    /// Removes the element with the given index from the list and returns it,
    /// or returns `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    /// assert_eq!(0, index.capacity());
    ///
    /// index.push(10);
    /// index.push(12);
    ///
    /// assert_eq!(Some(10), index.remove(0));
    /// assert_eq!(None, index.remove(0));
    /// assert_eq!(2, index.capacity());
    /// assert_eq!(1, index.len());
    /// assert_eq!(Some(12), index.remove(1));
    /// assert_eq!(2, index.capacity());
    /// ```
    pub fn remove(&mut self, index: u64) -> Option<V> {
        let mut size = self.size();
        if index >= size.capacity {
            return None;
        }
        let v = self.base.get(&index);
        if v.is_some() {
            self.base.remove(&index);
            size.length -= 1;
            self.set_size(size);
        }
        v
    }

    /// Extends the list with the contents of an iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    /// assert!(index.is_empty());
    ///
    /// index.extend([1, 2, 3].iter().cloned());
    /// assert_eq!(3, index.capacity());
    /// ```
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = V>,
    {
        let mut size = self.size();
        for value in iter {
            self.base.put(&size.capacity, value);
            size.capacity += 1;
            size.length += 1;
        }
        self.set_size(size);
    }

    /// Changes a value at a specified position. If the position contains an empty value, it
    /// also increments the elements count. If the index value of the new element is greater than
    /// the current capacity, the capacity of the list is considered index + 1 and all further elements
    /// without specific index values will be appended after this index.
    ///
    /// Returns the value of a previous element at the indicated position or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    ///
    /// index.push(1);
    /// assert_eq!(Some(1), index.get(0));
    ///
    /// index.set(0, 10);
    /// assert_eq!(Some(10), index.get(0));
    /// ```
    pub fn set(&mut self, index: u64, value: V) -> Option<V> {
        let mut size = self.size();
        // Update items count
        let old_value = self.base.get::<u64, V>(&index);
        if old_value.is_none() {
            size.length += 1;
            if index >= size.capacity {
                size.capacity = index + 1;
            }
            self.set_size(size);
        }
        self.base.put(&index, value);
        old_value
    }

    /// Clears the list, removing all values.
    ///
    /// # Notes
    ///
    /// Currently, this method is not optimized to delete a large set of data.
    /// During the execution of this method, the amount of allocated memory
    /// is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    ///
    /// index.clear();
    /// assert!(index.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.base.clear();
        self.state.unset();
    }

    /// Removes the first element from the `SparseListIndex` and returns it, or
    /// returns `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, SparseListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = fork.get_sparse_list("name");
    /// assert_eq!(None, index.pop());
    ///
    /// index.push(1);
    /// assert_eq!(Some(1), index.pop());
    /// ```
    pub fn pop(&mut self) -> Option<V> {
        let first_item = { self.iter().next() };

        if let Some((first_index, first_elem)) = first_item {
            let mut size = self.size();
            self.base.remove(&first_index);
            size.length -= 1;
            self.set_size(size);
            return Some(first_elem);
        }
        None
    }

    fn set_size(&mut self, size: SparseListSize) {
        self.state.set(size);
    }
}

impl<'a, T, V> IntoIterator for &'a SparseListIndex<T, V>
where
    T: RawAccess,
    V: BinaryValue,
{
    type Item = (u64, V);
    type IntoIter = Entries<'a, u64, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T, V> IndexIterator for SparseListIndex<T, V>
where
    T: RawAccess,
    V: BinaryValue,
{
    type Key = u64;
    type Value = V;

    fn index_iter(&self, from: Option<&u64>) -> Entries<'_, u64, V> {
        Entries::new(&self.base, from)
    }
}

#[cfg(test)]
mod tests {
    use crate::{access::CopyAccessExt, db::Database, TemporaryDB};

    const IDX_NAME: &str = "idx_name";

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_list_index_methods() {
        let db = TemporaryDB::default();
        let fork = db.fork();
        let mut list_index = fork.get_sparse_list(IDX_NAME);

        assert!(list_index.is_empty());
        assert_eq!(0, list_index.capacity());
        assert!(list_index.get(0).is_none());
        assert_eq!(None, list_index.pop());

        let extended_by = vec![45, 3422, 234];
        list_index.extend(extended_by);
        assert!(!list_index.is_empty());
        assert_eq!(Some(45), list_index.get(0));
        assert_eq!(Some(3422), list_index.get(1));
        assert_eq!(Some(234), list_index.get(2));
        assert_eq!(3, list_index.capacity());
        assert_eq!(3, list_index.len());

        assert_eq!(Some(234), list_index.set(2, 777));
        assert_eq!(Some(777), list_index.get(2));
        assert_eq!(3, list_index.capacity());
        assert_eq!(3, list_index.len());

        let extended_by_again = vec![666, 999];
        for el in &extended_by_again {
            list_index.push(*el);
        }
        assert_eq!(Some(666), list_index.get(3));
        assert_eq!(Some(999), list_index.get(4));
        assert_eq!(5, list_index.capacity());
        assert_eq!(5, list_index.len());

        assert_eq!(Some(3422), list_index.remove(1));
        assert_eq!(None, list_index.remove(1));
        assert_eq!(5, list_index.capacity());
        assert_eq!(4, list_index.len());

        assert_eq!(Some(777), list_index.remove(2));
        assert_eq!(5, list_index.capacity());
        assert_eq!(3, list_index.len());

        assert_eq!(Some(45), list_index.pop());
        assert_eq!(5, list_index.capacity());
        assert_eq!(2, list_index.len());
        assert_eq!(Some(666), list_index.pop());
        assert_eq!(5, list_index.capacity());
        assert_eq!(1, list_index.len());

        list_index.push(42);
        assert_eq!(6, list_index.capacity());
        assert_eq!(2, list_index.len());

        assert_eq!(Some(999), list_index.pop());
        assert_eq!(6, list_index.capacity());
        assert_eq!(1, list_index.len());
        assert_eq!(Some(42), list_index.pop());
        assert_eq!(6, list_index.capacity());
        assert_eq!(0, list_index.len());
        assert_eq!(None, list_index.pop());

        // check that capacity gets overwritten by bigger index correctly
        assert_eq!(None, list_index.set(42, 1024));
        assert_eq!(43, list_index.capacity());

        list_index.clear();
        assert_eq!(0, list_index.len());
    }

    #[test]
    fn test_list_index_iter() {
        let db = TemporaryDB::default();
        let fork = db.fork();
        let mut list_index = fork.get_sparse_list(IDX_NAME);

        list_index.extend(vec![1_u8, 15, 25, 2, 3]);
        assert_eq!(
            list_index.indexes().collect::<Vec<u64>>(),
            vec![0_u64, 1, 2, 3, 4]
        );
        assert_eq!(
            list_index.values().collect::<Vec<u8>>(),
            vec![1_u8, 15, 25, 2, 3]
        );

        list_index.remove(1);
        list_index.remove(2);

        assert_eq!(
            list_index.iter().collect::<Vec<_>>(),
            vec![(0_u64, 1_u8), (3_u64, 2_u8), (4_u64, 3_u8)]
        );

        assert_eq!(
            list_index.iter_from(0).collect::<Vec<_>>(),
            vec![(0_u64, 1_u8), (3_u64, 2_u8), (4_u64, 3_u8)]
        );
        assert_eq!(
            list_index.iter_from(1).collect::<Vec<_>>(),
            vec![(3_u64, 2_u8), (4_u64, 3_u8)]
        );
        assert_eq!(list_index.iter_from(5).count(), 0);

        assert_eq!(list_index.indexes().collect::<Vec<_>>(), vec![0_u64, 3, 4]);
        assert_eq!(list_index.values().collect::<Vec<_>>(), vec![1_u8, 2, 3]);
    }

    #[test]
    fn restore_after_no_op_initialization() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_sparse_list::<_, u32>(IDX_NAME);
        let list = fork.readonly().get_sparse_list::<_, u32>(IDX_NAME);
        assert!(list.is_empty());
    }
}
