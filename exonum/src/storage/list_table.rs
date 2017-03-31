use num::{Integer, range, ToPrimitive};

use std::marker::PhantomData;
use std::cell::Cell;

use super::{Map, Error, StorageValue, List};

pub struct ListTable<T: Map<[u8], Vec<u8>>, K, V> {
    map: T,
    count: Cell<Option<K>>,
    _v: PhantomData<V>,
}

impl<'a, T, K, V> ListTable<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Integer + Copy + Clone + ToPrimitive + StorageValue,
          V: StorageValue
{
    pub fn new(map: T) -> Self {
        ListTable {
            map: map,
            count: Cell::new(None),
            _v: PhantomData,
        }
    }

    // TODO: implement iterator for List
    pub fn values(&self) -> Result<Vec<V>, Error> {
        Ok(if self.is_empty()? {
            Vec::new()
        } else {
            range(K::zero(), self.len()?).map(|i| self.get(i).unwrap().unwrap()).collect()
        })
    }
}

impl<T, K: ?Sized, V> List<K, V> for ListTable<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Integer + Copy + Clone + ToPrimitive + StorageValue,
          V: StorageValue
{
    fn append(&self, value: V) -> Result<(), Error> {
        let len = self.len()?;
        self.map.put(&len.serialize(), value.serialize())?;
        self.map.put(&[], (len + K::one()).serialize())?;
        self.count.set(Some(len + K::one()));
        Ok(())
    }

    fn extend<I>(&self, iter: I) -> Result<(), Error>
        where I: IntoIterator<Item = V>
    {
        let mut len = self.len()?;
        for value in iter {
            self.map.put(&len.serialize(), value.serialize())?;
            len = len + K::one();
        }
        self.map.put(&[], len.serialize())?;
        self.count.set(Some(len));
        Ok(())
    }

    fn get(&self, index: K) -> Result<Option<V>, Error> {
        let value = self.map.get(&index.serialize())?;
        Ok(value.map(StorageValue::deserialize))
    }

    fn set(&self, index: K, value: V) -> Result<(), Error> {
        if index >= self.len()? {
            return Err(Error::new("Wrong index!"));
        }
        self.map.put(&index.serialize(), value.serialize())
    }

    fn last(&self) -> Result<Option<V>, Error> {
        let len = self.len()?;
        if len == K::zero() {
            Ok(None)
        } else {
            self.get(len - K::one())
        }
    }

    fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == K::zero())
    }

    fn len(&self) -> Result<K, Error> {
        if let Some(count) = self.count.get() {
            return Ok(count);
        }

        let v = self.map.get(&[])?;
        let c = v.map_or_else(K::zero, K::deserialize);
        self.count.set(Some(c));
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use ::storage::{MemoryDB, List, ListTable, MapTable};

    #[test]
    fn test_list_table_methods() {
        let storage = MemoryDB::new();
        let list_table = ListTable::new(MapTable::new(vec![255], &storage));

        assert!(list_table.is_empty().unwrap());
        assert_eq!(0, list_table.len().unwrap());
        assert!(list_table.last().unwrap().is_none());

        let extended_by = vec![45u64, 3422u64, 234u64];
        list_table.extend(extended_by.into_iter()).unwrap();
        assert!(!list_table.is_empty().unwrap());
        assert_eq!(Some(45u64), list_table.get(0u32).unwrap());
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
        let _ = list_table.swap(3, 4).unwrap();
        assert_eq!(Some(999u64), list_table.get(3).unwrap());
        assert_eq!(Some(666u64), list_table.get(4).unwrap());
        let res = list_table.swap(5, 10);
        assert!(res.is_err());
    }

}
