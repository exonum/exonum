use num::{range};

use std::marker::PhantomData;
use std::cell::Cell;

use super::base_table::BaseTable;

use super::{View, Error, StorageValue, VoidKey, List};

pub struct ListTable<'a, V> {
    base: BaseTable<'a>,
    count: Cell<Option<u64>>,
    _v: PhantomData<V>,
}

impl<'a, V: StorageValue> ListTable<'a, V> {
    pub fn new(prefix: Vec<u8>, view: &'a View) -> Self {
        ListTable {
            base: BaseTable::new(prefix, view),
            count: Cell::new(None),
            _v: PhantomData,
        }
    }

    // TODO: implement iterator for List
    pub fn values(&self) -> Result<Vec<V>, Error> {
        Ok(if self.is_empty()? {
            Vec::new()
        } else {
            range(0, self.len()?).map(|i| self.get(i).unwrap().unwrap()).collect()
        })
    }
}

impl<'a, V: StorageValue> List<V> for ListTable<'a, V> {
    fn append(&self, value: V) -> Result<(), Error> {
        let len = self.len()?;
        self.base.put(&len, value.serialize())?;
        self.base.put(&VoidKey, (len + 1).serialize())?;
        self.count.set(Some(len + 1));
        Ok(())
    }

    fn extend<I>(&self, iter: I) -> Result<(), Error>
        where I: IntoIterator<Item = V>
    {
        let mut len = self.len()?;
        for value in iter {
            self.base.put(&len, value.serialize())?;
            len = len + 1;
        }
        self.base.put(&VoidKey, len.serialize())?;
        self.count.set(Some(len));
        Ok(())
    }

    fn get(&self, index: u64) -> Result<Option<V>, Error> {
        let value = self.base.get(&index)?;
        Ok(value.map(StorageValue::deserialize))
    }

    fn set(&self, index: u64, value: V) -> Result<(), Error> {
        if index >= self.len()? {
            return Err(Error::new("Wrong index!"));
        }
        self.base.put(&index, value.serialize())
    }

    fn last(&self) -> Result<Option<V>, Error> {
        let len = self.len()?;
        if len == 0 {
            Ok(None)
        } else {
            self.get(len - 1)
        }
    }

    fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == 0)
    }

    fn len(&self) -> Result<u64, Error> {
        if let Some(count) = self.count.get() {
            return Ok(count);
        }

        let c = self.base.get(&VoidKey)?.map(StorageValue::deserialize).unwrap_or(0);
        self.count.set(Some(c));
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use ::storage::{MemoryDB, Database, List, ListTable};

    #[test]
    fn test_list_table_methods() {
        let storage = MemoryDB::new().fork();
        let list_table = ListTable::new(vec![255], &storage);

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
