use std::marker::PhantomData;
use std::cell::Cell;
use std::num::{Zero, One};
use std::ops::{Add, Sub};

use super::{Map, Error, StorageValue};

pub struct ListTable<T: Map<[u8], Vec<u8>>, K, V> {
    map: T,
    count: Cell<Option<K>>,
    _v: PhantomData<V>,
}

impl<'a, T, K, V> ListTable<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Zero + One + Add<Output = K> + Sub<Output = K> + PartialEq + Copy + StorageValue,
          ::std::ops::Range<K>: ::std::iter::Iterator<Item = K>,
          V: StorageValue
{
    pub fn new(map: T) -> Self {
        ListTable {
            map: map,
            count: Cell::new(None),
            _v: PhantomData,
        }
    }

    pub fn append(&mut self, value: V) -> Result<(), Error> {
        let len = self.len()?;
        self.map.put(&len.serialize(), value.serialize())?;
        self.map.put(&[], (len + One::one()).serialize())?;
        self.count.set(Some(len + One::one()));
        Ok(())
    }

    pub fn extend<I>(&mut self, iter: I) -> Result<(), Error>
        where I: IntoIterator<Item = V>
    {
        let mut len = self.len()?;
        for value in iter {
            self.map.put(&len.serialize(), value.serialize())?;
            len = len + One::one();
        }
        self.map.put(&[], (len + One::one()).serialize())?;
        self.count.set(Some(len + One::one()));
        Ok(())
    }

    pub fn get(&self, index: K) -> Result<Option<V>, Error> {
        let value = self.map.get(&index.serialize())?;
        Ok(value.map(StorageValue::deserialize))
    }

    pub fn last(&self) -> Result<Option<V>, Error> {
        let len = self.len()?;
        if len == Zero::zero() {
            Ok(None)
        } else {
            self.get(len - One::one())
        }
    }

    // TODO: implement iterator for List
    pub fn iter(&self) -> Result<Option<Vec<V>>, Error> {
        Ok(if self.is_empty()? {
            None
        } else {
            Some((Zero::zero()..self.len()?).map(|i| self.get(i).unwrap().unwrap()).collect())
        })
    }

    pub fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == Zero::zero())
    }

    pub fn len(&self) -> Result<K, Error> {
        if let Some(count) = self.count.get() {
            return Ok(count);
        }

        let v = self.map.get(&[])?;
        let c = v.map(K::deserialize).unwrap_or(Zero::zero());
        self.count.set(Some(c));
        Ok(c)
    }
}
