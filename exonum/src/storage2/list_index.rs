use std::cell::Cell;
use std::marker::PhantomData;

use super::{BaseIndex, BaseIndexIter, Snapshot, Fork, StorageValue};

pub struct ListIndex<T, V> {
    base: BaseIndex<T>,
    length: Cell<Option<u64>>,
    _v: PhantomData<V>,
}

pub struct ListIndexIter<'a, V> {
    base_iter: BaseIndexIter<'a, u64, V>
}

impl<T, V> ListIndex<T, V> {
    pub fn new(prefix: Vec<u8>, base: T) -> Self {
        ListIndex {
            base: BaseIndex::new(prefix, base),
            length: Cell::new(None),
            _v: PhantomData
        }
    }
}

impl<T, V> ListIndex<T, V> where T: AsRef<Snapshot>,
                                 V: StorageValue {
    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&index)
    }

    pub fn last(&self) -> Option<V> {
        match self.len() {
            0 => None,
            l => self.get(l - 1)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> u64 {
        if let Some(len) = self.length.get() {
            return len
        }
        let len = self.base.get(&()).unwrap_or(0);
        self.length.set(Some(len));
        len
    }

    pub fn iter(&self) -> ListIndexIter<V> {
        ListIndexIter { base_iter: self.base.iter_from(&(), &0u64) }
    }

    pub fn iter_from(&self, from: u64) -> ListIndexIter<V> {
        ListIndexIter { base_iter: self.base.iter_from(&(), &from) }
    }
}

impl<'a, V> ListIndex<&'a mut Fork, V> where V: StorageValue {
    fn set_len(&mut self, len: u64) {
        self.base.put(&(), len);
        self.length.set(Some(len));
    }

    pub fn push(&mut self, value: V) {
        let len = self.len();
        self.base.put(&len, value);
        self.set_len(len + 1)
    }

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

    pub fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=V> {
        let mut len = self.len();
        for value in iter {
            self.base.put(&len, value);
            len = len + 1;
        }
        self.base.put(&(), len);
        self.set_len(len);
    }

    pub fn truncate(&mut self, len: u64) {
        // TODO: optimize this
        while self.len() > len {
            self.pop();
        }
    }

    pub fn set(&mut self, index: u64, value: V) {
        if index >= self.len() {
            panic!("index out of bounds: \
                    the len is {} but the index is {}", self.len(), index);
        }
        self.base.put(&index, value)
    }

    pub fn clear(&mut self) {
        self.length.set(Some(0));
        self.base.clear()
    }
}

impl<'a, T, V> ::std::iter::IntoIterator for &'a ListIndex<T, V> where T: AsRef<Snapshot>,
                                                                       V: StorageValue {
    type Item = V;
    type IntoIter = ListIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for ListIndexIter<'a, V> where V: StorageValue {
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
        list_index.extend(extended_by.into_iter());
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
    }

}
