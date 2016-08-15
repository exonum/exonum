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
    pub fn iter(&self) -> Result<Option<Vec<V>>, Error> {
        Ok(if self.is_empty()? {
            None
        } else {
            Some(range(K::zero(), self.len()?).map(|i| self.get(i).unwrap().unwrap()).collect())
        })
    }
}

impl<T, K: ?Sized, V> List<K, V> for ListTable<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Integer + Copy + Clone + ToPrimitive + StorageValue,
          V: StorageValue
{
    fn append(&mut self, value: V) -> Result<(), Error> {
        let len = self.len()?;
        self.map.put(&len.serialize(), value.serialize())?;
        self.map.put(&[], (len + K::one()).serialize())?;
        self.count.set(Some(len + K::one()));
        Ok(())
    }

    fn extend<I>(&mut self, iter: I) -> Result<(), Error>
        where I: IntoIterator<Item = V>
    {
        let mut len = self.len()?;
        for value in iter {
            self.map.put(&len.serialize(), value.serialize())?;
            len = len + K::one();
        }
        self.map.put(&[], (len + K::one()).serialize())?;
        self.count.set(Some(len + K::one()));
        Ok(())
    }

    fn get(&self, index: K) -> Result<Option<V>, Error> {
        let value = self.map.get(&index.serialize())?;
        Ok(value.map(StorageValue::deserialize))
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
