use std::cell::Cell;
use std::marker::PhantomData;

use super::{BaseIndex, BaseIndexIter, Result, Snapshot, Fork, StorageValue};

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
    pub fn get(&self, index: u64) -> Result<Option<V>> {
        self.base.get(&index)
    }

    pub fn last(&self) -> Result<Option<V>> {
        match self.len()? {
            0 => Ok(None),
            l => self.get(l - 1)
        }
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    pub fn len(&self) -> Result<u64> {
        if let Some(len) = self.length.get() {
            return Ok(len)
        }
        let len = self.base.get(&())?.unwrap_or(0);
        self.length.set(Some(len));
        Ok(len)
    }

    pub fn iter(&self) -> ListIndexIter<V> {
        ListIndexIter { base_iter: self.base.iter() }
    }

    pub fn iter_from(&self, from: u64) -> ListIndexIter<V> {
        ListIndexIter { base_iter: self.base.iter_from(&from) }
    }
}

impl<'a, V> ListIndex<&'a mut Fork, V> where V: StorageValue {
    pub fn push(&mut self, value: V) -> Result<()> {
        let len = self.len()?;
        self.base.put(&len, value);
        self.base.put(&(), len + 1);
        self.length.set(Some(len + 1));
        Ok(())
    }

    pub fn pop(&mut self) -> Result<Option<V>> {
        // TODO: shoud we get and return dropped value?
        match self.len()? {
            0 => Ok(None),
            l => {
                let v = self.base.get(&(l - 1))?;
                self.base.delete(&(l - 1));
                self.base.put(&(), l - 1);
                Ok(v)
            }
        }
    }

    pub fn extend<I>(&mut self, iter: I) -> Result<()> where I: IntoIterator<Item=V> {
        let mut len = self.len()?;
        for value in iter {
            self.base.put(&len, value);
            len = len + 1;
        }
        self.base.put(&(), len);
        self.length.set(Some(len));
        Ok(())
    }

    pub fn truncate(&mut self, len: u64) -> Result<()> {
        // TODO: optimize this
        while self.len()? > len {
            self.pop()?;
        }
        Ok(())
    }

    pub fn set(&mut self, index: u64, value: V) -> Result<()> {
        // TODO: shoud we panic here?
        if index < self.len()? {
            self.base.put(&index, value)
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.base.clear()
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
    use ::storage::{MemoryDB, Database, List, ListTable};

    #[test]
    fn test_list_table_methods() {
        let fork = MemoryDB::new().fork();
        let list_table = ListTable::new(vec![255], &mut fork);

        assert!(list_table.is_empty().unwrap());
        assert_eq!(0, list_table.len().unwrap());
        assert!(list_table.last().unwrap().is_none());

        let extended_by = vec![45u64, 3422u64, 234u64];
        list_table.extend(extended_by.into_iter()).unwrap();
        assert!(!list_table.is_empty().unwrap());
        assert_eq!(Some(45u64), list_table.get(0u64).unwrap());
        assert_eq!(Some(3422u64), list_table.get(1).unwrap());
        assert_eq!(Some(234u64), list_table.get(2).unwrap());
        assert_eq!(3, list_table.len().unwrap());

        list_table.set(2, 777u64).unwrap();
        assert_eq!(Some(777u64), list_table.get(2).unwrap());
        assert_eq!(Some(777u64), list_table.last().unwrap());
        assert_eq!(3, list_table.len().unwrap());

        let mut extended_by_again = vec![666u64, 999u64];
        for el in &extended_by_again {
            list_table.append(*el).unwrap();
        }
        assert_eq!(Some(666u64), list_table.get(3).unwrap());
        assert_eq!(Some(999u64), list_table.get(4).unwrap());
        assert_eq!(5, list_table.len().unwrap());
        extended_by_again[1] = 1001u64;
        list_table.extend(extended_by_again).unwrap();
        assert_eq!(7, list_table.len().unwrap());
        assert_eq!(Some(1001u64), list_table.last().unwrap());
    }

}
