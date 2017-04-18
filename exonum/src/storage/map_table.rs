use std::marker::PhantomData;

use profiler;

use super::{Map, Error, StorageValue};

pub struct MapTable<'a, T: Map<[u8], Vec<u8>> + 'a, K: ?Sized, V> {
    prefix: Vec<u8>,
    storage: &'a T,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<'a, T: Map<[u8], Vec<u8>> + 'a, K: ?Sized, V> MapTable<'a, T, K, V> {
    pub fn new(prefix: Vec<u8>, storage: &'a T) -> Self {
        MapTable {
            prefix: prefix,
            storage: storage,
            _k: PhantomData,
            _v: PhantomData,
        }
    }
}

impl<'a, T, K: ?Sized, V> Map<K, V> for MapTable<'a, T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: AsRef<[u8]>,
          V: StorageValue
{
    fn get(&self, key: &K) -> Result<Option<V>, Error> {
        let _profiler = profiler::ProfilerSpan::new("MapTable::get");
        let v = self.storage.get(&[&self.prefix, key.as_ref()].concat())?;
        Ok(v.map(StorageValue::deserialize))
    }

    fn put(&self, key: &K, value: V) -> Result<(), Error> {
        self.storage.put(&[&self.prefix, key.as_ref()].concat(), value.serialize())
    }

    fn delete(&self, key: &K) -> Result<(), Error> {
        self.storage.delete(&[&self.prefix, key.as_ref()].concat())
    }
    fn find_key(&self, origin_key: &K) -> Result<Option<Vec<u8>>, Error> {
        let key = [&self.prefix, origin_key.as_ref()].concat();
        let result = match self.storage.find_key(&key)? {
            Some(x) => {
                if !x.starts_with(&key) {
                    None
                } else {
                    Some(x[self.prefix.len()..].to_vec())
                }
            }
            None => None,
        };
        Ok(result)
    }
}

// pub struct MapTableIterator<'a, T: Iterator<Item = (Vec<u8>, Vec<u8>)>> {
//     iter: T,
//     prefix: &'a [u8],
// }

// impl<'a, T> Iterator for MapTableIterator<'a, T>
//     where T: Iterator<Item = (Vec<u8>, Vec<u8>)>
// {
//     type Item = (Vec<u8>, Vec<u8>);

//     fn next(&mut self) -> Option<(Vec<u8>, Vec<u8>)> {
//         match self.iter.next() {
//             Some(item) => {
//                 if item.0.starts_with(self.prefix) {
//                     let key = item.0[self.prefix.len()..].to_vec();
//                     return Some((key, item.1));
//                 }
//                 None
//             }
//             None => None,
//         }
//     }
// }

// impl<'a, T, K, V> IntoIterator for &'a MapTable<'a, T, K, V>
//     where T: Map<[u8], Vec<u8>> + 'a,
//           K: AsRef<[u8]>,
//           V: StorageValue,
//           &'a T: Iterable,
//           <&'a T as Iterable>::Iter: Iterator<Item = (Vec<u8>, Vec<u8>)> + Seekable<'a, Key = Vec<u8>>
// {
//     type Item = (Vec<u8>, Vec<u8>);
//     type IntoIter = MapTableIterator<'a, <&'a T as Iterable>::Iter>;

//     fn into_iter(self) -> Self::IntoIter {
//         let mut iter = self.storage.iter();
//         iter.seek(&self.prefix);
//         MapTableIterator {
//             iter: iter,
//             prefix: &self.prefix,
//         }
//     }
// }

// impl<'a, T, K, V> Iterable for &'a MapTable<'a, T, K, V>
//     where T: Map<[u8], Vec<u8>> + 'a,
//           K: AsRef<[u8]>,
//           V: StorageValue,
//           &'a T: Iterable,
//           <&'a T as Iterable>::Iter: Iterator<Item = (Vec<u8>, Vec<u8>)> + Seekable<'a, Key = Vec<u8>>,

// {
//     type Iter = MapTableIterator<'a, <&'a T as Iterable>::Iter>;

//     fn iter(self) -> Self::Iter {
//         let mut iter = self.storage.iter();
//         iter.seek(&self.prefix);
//         MapTableIterator {
//             iter: iter,
//             prefix: &self.prefix,
//         }
//     }
// }

// impl<'a, T> Seekable<'a> for MapTableIterator<'a, T>
//     where T: Iterator<Item = (Vec<u8>, Vec<u8>)> + Seekable<'a, Key=Vec<u8>, Item=(Vec<u8>, Vec<u8>)>
// {
//     type Key = Vec<u8>;
//     type Item = (Vec<u8>, Vec<u8>);

//     fn seek(&mut self, key: &Self::Key) -> Option<(Vec<u8>, Vec<u8>)> {
//         let db_key = &[self.prefix, &key].concat();
//         self.iter.seek(db_key).map(|x| (x.0[self.prefix.len()..].to_vec(), x.1))
//     }
// }
