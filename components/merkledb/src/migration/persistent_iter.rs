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

//! Persistent iterators.

use exonum_crypto::Hash;
use failure::{bail, ensure};

use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    fmt,
    iter::{Peekable, Zip},
    ops::RangeFrom,
};

use crate::{
    access::{Access, AccessExt, RawAccess, RawAccessMut},
    indexes::{self, proof_map::ToProofPath},
    BinaryKey, BinaryValue, Entry, KeySetIndex, ListIndex, MapIndex, ObjectHash, ProofListIndex,
    ProofMapIndex, SparseListIndex, ValueSetIndex,
};

/// Database object that supports iteration and continuing iteration from an intermediate position.
pub trait ContinueIterator {
    /// Type encompassing iteration position.
    type Key: BinaryKey + ?Sized;
    /// Iterator returned by the object.
    type Iter: Iterator;

    /// Continues iteration from the specified position. If `from` is `None`, starts the iteration
    /// from scratch.
    fn continue_iter(self, from: Option<&Self::Key>) -> Self::Iter;

    /// Extracts the iteration position from the item returned by the iterator.
    fn extract_key(item: &<Self::Iter as Iterator>::Item) -> <Self::Key as ToOwned>::Owned;
}

impl<'a, T, V> ContinueIterator for &'a ListIndex<T, V>
where
    T: RawAccess,
    V: BinaryValue,
{
    type Key = u64;
    type Iter = Zip<RangeFrom<u64>, indexes::list::Iter<'a, V>>;

    fn continue_iter(self, from: Option<&u64>) -> Self::Iter {
        if let Some(&from) = from {
            (from..).zip(self.iter_from(from))
        } else {
            (0..).zip(self.iter())
        }
    }

    fn extract_key(item: &(u64, V)) -> u64 {
        item.0
    }
}

impl<'a, T, V> ContinueIterator for &'a ProofListIndex<T, V>
where
    T: RawAccess,
    V: BinaryValue,
{
    type Key = u64;
    type Iter = Zip<RangeFrom<u64>, indexes::proof_list::Iter<'a, V>>;

    fn continue_iter(self, from: Option<&u64>) -> Self::Iter {
        if let Some(&from) = from {
            (from..).zip(self.iter_from(from))
        } else {
            (0..).zip(self.iter())
        }
    }

    fn extract_key(item: &(u64, V)) -> u64 {
        item.0
    }
}

impl<'a, T, V> ContinueIterator for &'a SparseListIndex<T, V>
where
    T: RawAccess,
    V: BinaryValue,
{
    type Key = u64;
    type Iter = indexes::sparse_list::Iter<'a, V>;

    fn continue_iter(self, from: Option<&u64>) -> Self::Iter {
        if let Some(&from) = from {
            self.iter_from(from)
        } else {
            self.iter()
        }
    }

    fn extract_key(item: &(u64, V)) -> u64 {
        item.0
    }
}

impl<'a, T, K, V> ContinueIterator for &'a MapIndex<T, K, V>
where
    T: RawAccess,
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    type Key = K;
    type Iter = indexes::map::Iter<'a, K, V>;

    fn continue_iter(self, from: Option<&K>) -> Self::Iter {
        if let Some(from) = from {
            self.iter_from(from)
        } else {
            self.iter()
        }
    }

    fn extract_key(item: &(K::Owned, V)) -> K::Owned {
        item.0.borrow().to_owned()
    }
}

impl<'a, T, K, V, KeyMode> ContinueIterator for &'a ProofMapIndex<T, K, V, KeyMode>
where
    T: RawAccess,
    K: BinaryKey + ?Sized,
    V: BinaryValue,
    KeyMode: ToProofPath<K>,
{
    type Key = K;
    type Iter = indexes::proof_map::Iter<'a, K, V>;

    fn continue_iter(self, from: Option<&K>) -> Self::Iter {
        if let Some(from) = from {
            self.iter_from(from)
        } else {
            self.iter()
        }
    }

    fn extract_key(item: &(K::Owned, V)) -> K::Owned {
        item.0.borrow().to_owned()
    }
}

impl<'a, T, K> ContinueIterator for &'a KeySetIndex<T, K>
where
    T: RawAccess,
    K: BinaryKey,
{
    type Key = K;
    type Iter = indexes::key_set::Iter<'a, K>;

    fn continue_iter(self, from: Option<&K>) -> Self::Iter {
        if let Some(from) = from {
            self.iter_from(from)
        } else {
            self.iter()
        }
    }

    fn extract_key(item: &K::Owned) -> K::Owned {
        item.borrow().to_owned()
    }
}

impl<'a, T, V> ContinueIterator for &'a ValueSetIndex<T, V>
where
    T: RawAccess,
    V: BinaryValue + ObjectHash,
{
    type Key = Hash;
    type Iter = indexes::value_set::Iter<'a, V>;

    fn continue_iter(self, from: Option<&Hash>) -> Self::Iter {
        if let Some(from) = from {
            self.iter_from(from)
        } else {
            self.iter()
        }
    }

    fn extract_key(item: &(Hash, V)) -> Hash {
        item.0
    }
}

/// Persistent iterator position.
#[derive(PartialEq)]
enum IteratorPosition<K: BinaryKey + ?Sized> {
    /// There is a next key to start iteration from.
    NextKey(K::Owned),
    /// The iterator has ended.
    Ended,
}

impl<K> fmt::Debug for IteratorPosition<K>
where
    K: BinaryKey + fmt::Debug + ?Sized,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IteratorPosition::NextKey(key) => {
                let key_ref: &K = key.borrow();
                formatter.debug_tuple("NextKey").field(&key_ref).finish()
            }
            IteratorPosition::Ended => formatter.debug_tuple("Ended").finish(),
        }
    }
}

impl<K> BinaryValue for IteratorPosition<K>
where
    K: BinaryKey + ?Sized,
{
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            IteratorPosition::NextKey(key) => {
                let key: &K = key.borrow();
                let mut buffer = vec![0; 1 + key.size()];
                key.write(&mut buffer[1..]);
                buffer
            }
            IteratorPosition::Ended => vec![1],
        }
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, failure::Error> {
        ensure!(
            !bytes.is_empty(),
            "`IteratorPosition` serialization cannot be empty"
        );
        Ok(match bytes[0] {
            0 => IteratorPosition::NextKey(K::read(&bytes[1..])),
            1 => IteratorPosition::Ended,
            _ => bail!("Invalid `IteratorPosition` discriminant"),
        })
    }
}

/// Persistent iterator that stores its position in the database.
///
/// Persistent iterators iterate over an index and automatically persist iteration
/// results in the DB. This allows to build fault-tolerant migration scripts that work correctly
/// after being restarted while merging the intermediate changes to the database.
///
/// Like indexes, persistent iterators are identified by an address. Likewise, they are subject
/// to the borrowing rules (e.g., attempting to create two instances of the same iterator will
/// result in a runtime error). When migrating data, it makes sense to store iterators
/// in the associated [`Scratchpad`]. In this way, iterators will be automatically removed
/// when the migration is over.
///
/// # Examples
///
/// [`MigrationHelper`] offers convenient iterator API via `iter_loop` method, which covers
/// basic use cases. When `iter_loop` is not enough, a persistent iterator can be instantiated
/// independently:
///
/// ```
/// # use exonum_merkledb::{access::AccessExt, Database, TemporaryDB};
/// # use exonum_merkledb::migration::{MigrationHelper, PersistentIter};
/// let db = TemporaryDB::new();
/// // Create data for migration.
/// let fork = db.fork();
/// fork.get_proof_list("migration.list").extend((0..123).map(|i| i.to_string()));
/// db.merge(fork.into_patch()).unwrap();
///
/// let helper = MigrationHelper::new(db, "migration");
/// // The old data is here.
/// let list = helper.old_data().get_proof_list::<_, String>("list");
/// // In the context of migration, persistent iterators should use
/// // the scratchpad data access.
/// let iter = PersistentIter::new(helper.scratchpad(), "list_iter", &list);
/// // Now, we can use `iter` as any other iterator. Persistence is most useful
/// // together with the `take` combinator; it allows to break migrated data
/// // into manageable chunks.
/// for (_, item) in iter.take(100) {
///     // Migrate `item`. The first component of a tuple is the index of the item
///     // in the list, which we ignore.
/// }
///
/// // If we recreate the iterator, it will resume iteration from the last
/// // known position (the element with 0-based index 100, in our case).
/// let mut iter = PersistentIter::new(helper.scratchpad(), "list_iter", &list);
/// let (i, item) = iter.next().unwrap();
/// assert_eq!(i, 100);
/// assert_eq!(item, "100");
/// assert_eq!(iter.count(), 22); // number of remaining items
/// ```
///
/// [`Scratchpad`]: struct.Scratchpad.html
/// [`MigrationHelper`]: struct.MigrationHelper.html
pub struct PersistentIter<T: RawAccess, I: ContinueIterator> {
    inner: Inner<T, I>,
}

impl<T, I> fmt::Debug for PersistentIter<T, I>
where
    T: RawAccess,
    I: ContinueIterator,
    I::Key: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PersistentIter")
            .field("inner", &self.inner)
            .finish()
    }
}

/// Internal details of a persistent iterator.
enum Inner<T: RawAccess, I: ContinueIterator> {
    /// The iterator is active: it has an underlying iterator over a database object,
    /// and an entry storing the iterator position.
    Active {
        iter: Peekable<I::Iter>,
        position_entry: Entry<T, IteratorPosition<I::Key>>,
    },
    /// The iterator has ended.
    Ended,
}

impl<T, I> fmt::Debug for Inner<T, I>
where
    T: RawAccess,
    I: ContinueIterator,
    I::Key: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Inner::Active { position_entry, .. } => formatter
                .debug_struct("Active")
                .field("position", &position_entry.get())
                .finish(),
            Inner::Ended => formatter.debug_tuple("Ended").finish(),
        }
    }
}

impl<T, I> PersistentIter<T, I>
where
    T: RawAccessMut,
    I: ContinueIterator,
{
    /// Creates a new persistent iterator.
    pub fn new<A>(access: A, name: &str, index: I) -> Self
    where
        A: Access<Base = T>,
    {
        let position_entry: Entry<_, IteratorPosition<I::Key>> = access.get_entry(name);
        let position = position_entry.get();

        let start_key = match position {
            None => None,
            Some(IteratorPosition::NextKey(key)) => Some(key),
            Some(IteratorPosition::Ended) => {
                return Self {
                    inner: Inner::Ended,
                };
            }
        };

        Self {
            inner: Inner::Active {
                iter: index
                    .continue_iter(start_key.as_ref().map(Borrow::borrow))
                    .peekable(),
                position_entry,
            },
        }
    }
}

impl<T, I> Iterator for PersistentIter<T, I>
where
    T: RawAccessMut,
    I: ContinueIterator,
{
    type Item = <I::Iter as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner {
            Inner::Ended => None,
            Inner::Active {
                ref mut iter,
                ref mut position_entry,
            } => {
                let next = iter.next();
                if next.is_some() {
                    position_entry.set(if let Some(ref item) = iter.peek() {
                        let key = I::extract_key(item);
                        IteratorPosition::NextKey(key)
                    } else {
                        IteratorPosition::Ended
                    });
                } else {
                    position_entry.set(IteratorPosition::Ended);
                    self.inner = Inner::Ended;
                }
                next
            }
        }
    }
}

/// Factory for persistent iterators.
#[derive(Debug)]
pub struct PersistentIters<T> {
    access: T,
    names: HashSet<String>,
}

impl<T> PersistentIters<T>
where
    T: Access,
    T::Base: RawAccessMut,
{
    /// Creates a new factory.
    pub fn new(access: T) -> Self {
        Self {
            access,
            names: HashSet::new(),
        }
    }

    /// Creates a persistent iterator identified by the `name`.
    pub fn create<I: ContinueIterator>(
        &mut self,
        name: &str,
        index: I,
    ) -> PersistentIter<T::Base, I> {
        self.names.insert(name.to_owned());
        PersistentIter::new(self.access.clone(), name, index)
    }

    /// Checks if all iterators instantiated via this instance have ended.
    ///
    /// This method will panic if any of iterators are borrowed and thus should only be called
    /// when this is a priori not the case.
    pub(super) fn all_ended(&self) -> bool {
        for name in &self.names {
            let pos = self
                .access
                .clone()
                .get_entry::<_, IteratorPosition<()>>(name.as_str())
                .get();
            if pos != Some(IteratorPosition::Ended) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migration::Scratchpad, Database, TemporaryDB};

    use std::{collections::HashSet, iter::FromIterator};

    #[test]
    fn persistent_iter_for_map() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut map = fork.get_map("map");
        for i in 0_u32..10 {
            map.put(&i, i.to_string());
        }

        let scratchpad = Scratchpad::new("iter", &fork);
        let iter = PersistentIter::new(scratchpad, "map", &map);
        let mut count = 0;
        for (i, (key, value)) in iter.take(5).enumerate() {
            assert_eq!(key, i as u32);
            assert_eq!(value, i.to_string());
            count += 1;
        }
        assert_eq!(count, 5);
        {
            let position_entry = scratchpad.get_entry::<_, IteratorPosition<u32>>("map");
            assert_eq!(position_entry.get(), Some(IteratorPosition::NextKey(5)));
        }

        // Resume the iterator.
        let iter = PersistentIter::new(scratchpad, "map", &map);
        count = 0;
        for (i, (key, value)) in (5..).zip(iter) {
            assert_eq!(key, i as u32);
            assert_eq!(value, i.to_string());
            count += 1;
        }
        assert_eq!(count, 5);
        {
            let position_entry = scratchpad.get_entry::<_, IteratorPosition<u32>>("map");
            assert_eq!(position_entry.get(), Some(IteratorPosition::Ended));
        }

        // The iterator is ended now.
        let iter = PersistentIter::new(scratchpad, "map", &map);
        assert_eq!(iter.count(), 0);
    }

    #[test]
    fn persistent_iter_with_unsized_keys() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut map: ProofMapIndex<_, str, u64> = fork.get_proof_map("map");
        let words = ["How", "many", "letters", "are", "in", "this", "word", "?"];
        for &word in &words {
            map.put(word, word.len() as u64);
        }

        let scratchpad = Scratchpad::new("iter", &fork);
        let iter = PersistentIter::new(scratchpad, "map", &map);
        for (word, size) in iter.take_while(|(word, _)| word.as_str() < "many") {
            assert!(words.contains(&word.as_str()));
            assert_eq!(word.len() as u64, size);
        }

        {
            let position_entry = scratchpad.get_entry::<_, IteratorPosition<str>>("map");
            // Note that `many` is not included into the values yielded by the iterator,
            // but the iterator is advanced past it.
            let expected_pos = IteratorPosition::NextKey("this".to_owned());
            assert_eq!(position_entry.get(), Some(expected_pos));
        }

        let iter = PersistentIter::new(scratchpad, "map", &map);
        assert_eq!(
            iter.collect::<Vec<_>>(),
            vec![("this".to_owned(), 4), ("word".to_owned(), 4)]
        );
    }

    #[test]
    fn persistent_iter_for_list() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut list = fork.get_list("list");
        list.extend((0_u32..10).map(|i| i.to_string()));

        let scratchpad = Scratchpad::new("iter", &fork);
        let iter = PersistentIter::new(scratchpad, "list", &list);
        // Test that iterators work with combinators as expected.
        let items: Vec<_> = iter.take(5).filter(|(i, _)| i % 2 == 1).collect();
        assert_eq!(items, vec![(1, "1".to_owned()), (3, "3".to_owned())]);

        {
            let position_entry = scratchpad.get_entry::<_, IteratorPosition<u64>>("list");
            assert_eq!(position_entry.get(), Some(IteratorPosition::NextKey(5)));
        }

        let iter = PersistentIter::new(scratchpad, "list", &list);
        for (i, value) in iter.take(3) {
            assert_eq!(i.to_string(), value);
        }

        {
            let position_entry = scratchpad.get_entry::<_, IteratorPosition<u64>>("list");
            assert_eq!(position_entry.get(), Some(IteratorPosition::NextKey(8)));
        }

        let iter = PersistentIter::new(scratchpad, "list", &list);
        assert_eq!(iter.count(), 2);
    }

    #[test]
    fn empty_persistent_iter() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let list = fork.get_list::<_, String>("list");

        let scratchpad = Scratchpad::new("iter", &fork);
        let iter = PersistentIter::new(scratchpad, "list", &list);
        assert_eq!(iter.count(), 0);
        let position_entry = scratchpad.get_entry::<_, IteratorPosition<u64>>("list");
        assert_eq!(position_entry.get(), Some(IteratorPosition::Ended));
    }

    #[test]
    fn persistent_iter_for_sparse_list() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut list = fork.get_sparse_list("list");
        for &i in &[0, 1, 2, 3, 5, 8, 13, 21] {
            list.set(i, i.to_string());
        }

        let scratchpad = Scratchpad::new("iter", &fork);
        let iter = PersistentIter::new(scratchpad, "list", &list);
        let mut count = 0;
        for (i, value) in iter.take(5) {
            assert_eq!(value, i.to_string());
            count += 1;
        }
        assert_eq!(count, 5);
        {
            let position_entry = scratchpad.get_entry::<_, IteratorPosition<u64>>("list");
            assert_eq!(position_entry.get(), Some(IteratorPosition::NextKey(8)));
        }

        let iter = PersistentIter::new(scratchpad, "list", &list);
        let indexes: Vec<_> = iter.map(|(i, _)| i).collect();
        assert_eq!(indexes, vec![8, 13, 21]);
    }

    #[test]
    fn persistent_iter_for_key_set() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut set = fork.get_key_set("set");
        for &i in &[0_u16, 1, 2, 3, 5, 8, 13, 21] {
            set.insert(i);
        }

        let scratchpad = Scratchpad::new("iter", &fork);
        let iter = PersistentIter::new(scratchpad, "set", &set);
        let head: Vec<_> = iter.take(3).collect();
        assert_eq!(head, vec![0, 1, 2]);

        {
            let mut iter = PersistentIter::new(scratchpad, "set", &set);
            assert_eq!(iter.nth(2), Some(8));
        }
        {
            let position_entry = scratchpad.get_entry::<_, IteratorPosition<u16>>("set");
            assert_eq!(position_entry.get(), Some(IteratorPosition::NextKey(13)));
        }

        let iter = PersistentIter::new(scratchpad, "set", &set);
        let tail: Vec<_> = iter.collect();
        assert_eq!(tail, vec![13, 21]);
    }

    #[test]
    fn persistent_iter_for_value_set() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut set = fork.get_value_set("set");
        let items = [0_u16, 1, 2, 3, 5, 8, 13, 21];
        for &i in &items {
            set.insert(i);
        }

        let scratchpad = Scratchpad::new("iter", &fork);
        let iter = PersistentIter::new(scratchpad, "set", &set);
        let head: Vec<_> = iter.take(3).map(|(_, val)| val).collect();
        let iter = PersistentIter::new(scratchpad, "set", &set);
        let middle: Vec<_> = iter.take(2).map(|(_, val)| val).collect();
        let iter = PersistentIter::new(scratchpad, "set", &set);
        let tail: Vec<_> = iter.map(|(_, val)| val).collect();

        let actual_set: HashSet<_> = HashSet::from_iter(head.into_iter().chain(middle).chain(tail));
        assert_eq!(actual_set, HashSet::from_iter(items.iter().copied()));
    }
}
