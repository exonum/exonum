// Copyright 2017 The Exonum Team
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

//! A definition of `StorageKey` trait and implementations for common types.

use byteorder::{ByteOrder, BigEndian};

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crypto::{Hash, PublicKey, HASH_SIZE, PUBLIC_KEY_LENGTH};

/// A type that can be (de)serialized as a key in the blockchain storage.
///
/// Since keys are sorted in the serialized form, the big-endian encoding should be used
/// with unsigned integer types. Note however that the big-endian encoding
/// will not sort signed integer types in the natural order; therefore, they are
/// mapped to the corresponding unsigned type by adding a constant to the source value.
///
/// # Examples
///
/// ```
/// # extern crate exonum;
/// # extern crate byteorder;
/// use std::mem;
/// use exonum::storage::StorageKey;
///
/// struct Key {
///     a: i16,
///     b: u32,
/// }
///
/// impl StorageKey for Key {
///     fn size(&self) -> usize {
///         mem::size_of_val(&self.a) + mem::size_of_val(&self.b)
///     }
///
///     fn write(&self, buffer: &mut [u8]) {
///         self.a.write(&mut buffer[0..2]);
///         self.b.write(&mut buffer[2..6]);
///     }
///
///     fn read(buffer: &[u8]) -> Self {
///         let a = i16::read(&buffer[0..2]);
///         let b = u32::read(&buffer[2..6]);
///         Key { a, b }
///     }
/// }
/// # fn main() {
/// # // Check the natural ordering of keys
/// # let (mut x, mut y) = (vec![0u8; 6], vec![0u8; 6]);
/// # Key { a: -1, b: 2 }.write(&mut x);
/// # Key { a: 1, b: 513 }.write(&mut y);
/// # assert!(x < y);
/// # // Check the roundtrip
/// # let key = Key::read(&x);
/// # assert_eq!(key.a, -1);
/// # assert_eq!(key.b, 2);
/// # }
/// ```
pub trait StorageKey: ToOwned {
    /// Returns the size of the serialized key in bytes.
    fn size(&self) -> usize;

    /// Serializes the key into the specified buffer of bytes.
    ///
    /// The caller must guarantee that the size of the buffer is equal to the precalculated size
    /// of the serialized key.
    // TODO: should be unsafe (ECR-174)?
    fn write(&self, buffer: &mut [u8]);

    /// Deserializes the key from the specified buffer of bytes.
    // TODO: should be unsafe (ECR-174)?
    fn read(buffer: &[u8]) -> Self::Owned;
}

/// No-op implementation.
impl StorageKey for () {
    fn size(&self) -> usize {
        0
    }

    fn write(&self, _buffer: &mut [u8]) {
        // no-op
    }

    fn read(_buffer: &[u8]) -> Self::Owned {
        ()
    }
}

impl StorageKey for u8 {
    fn size(&self) -> usize {
        1
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer[0] = *self
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        buffer[0]
    }
}

/// Uses encoding with the values mapped to `u8`
/// by adding the corresponding constant (`128`) to the value.
impl StorageKey for i8 {
    fn size(&self) -> usize {
        1
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer[0] = self.wrapping_add(i8::min_value()) as u8;
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        buffer[0].wrapping_sub(i8::min_value() as u8) as i8
    }
}

// spell-checker:ignore utype, itype, vals, ints

macro_rules! storage_key_for_ints {
    ($utype:ident, $itype:ident, $size:expr, $read_method:ident, $write_method:ident) => {
        /// Uses big-endian encoding.
        impl StorageKey for $utype {
            fn size(&self) -> usize {
                $size
            }

            fn write(&self, buffer: &mut [u8]) {
                BigEndian::$write_method(buffer, *self);
            }

            fn read(buffer: &[u8]) -> Self {
                BigEndian::$read_method(buffer)
            }
        }

        /// Uses big-endian encoding with the values mapped to the unsigned format
        /// by adding the corresponding constant to the value.
        impl StorageKey for $itype {
            fn size(&self) -> usize {
                $size
            }

            fn write(&self, buffer: &mut [u8]) {
                BigEndian::$write_method(
                    buffer,
                    self.wrapping_add($itype::min_value()) as $utype,
                );
            }

            fn read(buffer: &[u8]) -> Self {
                BigEndian::$read_method(buffer)
                    .wrapping_sub($itype::min_value() as $utype) as $itype
            }
        }
    }
}

storage_key_for_ints!{u16, i16, 2, read_u16, write_u16}
storage_key_for_ints!{u32, i32, 4, read_u32, write_u32}
storage_key_for_ints!{u64, i64, 8, read_u64, write_u64}

impl StorageKey for Hash {
    fn size(&self) -> usize {
        HASH_SIZE
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_ref())
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        Hash::from_slice(buffer).unwrap()
    }
}

impl StorageKey for PublicKey {
    fn size(&self) -> usize {
        PUBLIC_KEY_LENGTH
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_ref())
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        PublicKey::from_slice(buffer).unwrap()
    }
}

impl StorageKey for Vec<u8> {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self)
    }

    fn read(buffer: &[u8]) -> Self {
        buffer.to_vec()
    }
}

impl StorageKey for [u8] {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self)
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        Vec::<u8>::read(buffer)
    }
}

/// Uses UTF-8 string serialization.
impl StorageKey for String {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_bytes())
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        unsafe { ::std::str::from_utf8_unchecked(buffer).to_string() }
    }
}

impl StorageKey for str {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_bytes())
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        String::read(buffer)
    }
}

/// The `SystemTime` (de)serialization is only possible for the values greater than
/// 1970-01-01 00:00:00 UTC. Since the `SystemTime` type saves time in the form
/// `(seconds: u64, nanoseconds: u32)`, the total occupied size of `SystemTime` in the storage
/// is 12 bytes. The seconds are stored in the first 8 bytes as per the `StorageKey` implementation
/// for u64, and nanoseconds in the remaining 4 bytes as per the `StorageKey`
/// implementation for u32.
impl StorageKey for SystemTime {
    fn size(&self) -> usize {
        12
    }

    fn write(&self, buffer: &mut [u8]) {
        let duration = self.duration_since(UNIX_EPOCH).expect(
            "time value is later than 1970-01-01 00:00:00 UTC.",
        );
        let secs = duration.as_secs();
        let nanos = duration.subsec_nanos();
        secs.write(&mut buffer[0..8]);
        nanos.write(&mut buffer[8..12]);
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        let secs = u64::read(&buffer[0..8]);
        let nanos = u32::read(&buffer[8..12]);
        // `Duration` performs internal normalization of time.
        // The value of nanoseconds can not be greater than or equal to 1,000,000,000.
        assert!(nanos < 1_000_000_000);
        UNIX_EPOCH + Duration::new(secs, nanos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Number of samples for fuzz testing
    const FUZZ_SAMPLES: usize = 100_000;

    macro_rules! test_storage_key_for_int_type {
        (full $type:ident, $size:expr => $test_name:ident) => {
            #[test]
            fn $test_name() {
                use std::iter::once;

                const MIN: $type = ::std::$type::MIN;
                const MAX: $type = ::std::$type::MAX;

                // Roundtrip
                let mut buffer = [0u8; $size];
                for x in (MIN..MAX).chain(once(MAX)) {
                    x.write(&mut buffer);
                    assert_eq!($type::read(&buffer), x);
                }

                // Ordering
                let (mut x_buffer, mut y_buffer) = ([0u8; $size], [0u8; $size]);
                for x in MIN..MAX {
                    let y = x + 1;
                    x.write(&mut x_buffer);
                    y.write(&mut y_buffer);
                    assert!(x_buffer < y_buffer);
                }
            }
        };
        (fuzz $type:ident, $size:expr => $test_name:ident) => {
            #[test]
            fn $test_name() {
                use rand::{Rng, thread_rng};
                let mut rng = thread_rng();

                // Fuzzed roundtrip
                let mut buffer = [0u8; $size];
                let handpicked_vals = vec![$type::min_value(), $type::max_value()];
                for x in rng.gen_iter::<$type>().take(FUZZ_SAMPLES).chain(handpicked_vals) {
                    x.write(&mut buffer);
                    assert_eq!($type::read(&buffer), x);
                }

                // Fuzzed ordering
                let (mut x_buffer, mut y_buffer) = ([0u8; $size], [0u8; $size]);
                let mut vals: Vec<$type> = rng.gen_iter().take(FUZZ_SAMPLES).collect();
                vals.sort();
                for w in vals.windows(2) {
                    let (x, y) = (w[0], w[1]);
                    if x == y { continue; }

                    x.write(&mut x_buffer);
                    y.write(&mut y_buffer);
                    assert!(x_buffer < y_buffer);
                }
            }
        }
    }

    test_storage_key_for_int_type!{full  u8, 1 => test_storage_key_for_u8}
    test_storage_key_for_int_type!{full  i8, 1 => test_storage_key_for_i8}
    test_storage_key_for_int_type!{full u16, 2 => test_storage_key_for_u16}
    test_storage_key_for_int_type!{full i16, 2 => test_storage_key_for_i16}
    test_storage_key_for_int_type!{fuzz u32, 4 => test_storage_key_for_u32}
    test_storage_key_for_int_type!{fuzz i32, 4 => test_storage_key_for_i32}
    test_storage_key_for_int_type!{fuzz u64, 8 => test_storage_key_for_u64}
    test_storage_key_for_int_type!{fuzz i64, 8 => test_storage_key_for_i64}

    #[test]
    fn test_signed_int_key_in_index() {
        use storage::{Database, MapIndex, MemoryDB};

        let db: Box<Database> = Box::new(MemoryDB::new());
        let mut fork = db.fork();
        {
            let mut index: MapIndex<_, i32, u64> = MapIndex::new("test_index", &mut fork);
            index.put(&5, 100);
            index.put(&-3, 200);
        }
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let index: MapIndex<_, i32, u64> = MapIndex::new("test_index", snapshot);
        assert_eq!(index.get(&5), Some(100));
        assert_eq!(index.get(&-3), Some(200));

        assert_eq!(
            index.iter_from(&-4).collect::<Vec<_>>(),
            vec![(-3, 200), (5, 100)]
        );
        assert_eq!(index.iter_from(&-2).collect::<Vec<_>>(), vec![(5, 100)]);
        assert_eq!(index.iter_from(&1).collect::<Vec<_>>(), vec![(5, 100)]);
        assert_eq!(index.iter_from(&6).collect::<Vec<_>>(), vec![]);

        assert_eq!(index.values().collect::<Vec<_>>(), vec![200, 100]);
    }

    // Example how to migrate from Exonum <= 0.5 implementation of `StorageKey`
    // for signed integers.
    #[test]
    fn test_old_signed_int_key_in_index() {
        use storage::{Database, MapIndex, MemoryDB};

        // Simple wrapper around a signed integer type with the `StorageKey` implementation,
        // which was used in Exonum <= 0.5.
        #[derive(Debug, PartialEq)]
        struct QuirkyI32Key(i32);

        impl StorageKey for QuirkyI32Key {
            fn size(&self) -> usize {
                4
            }

            fn write(&self, buffer: &mut [u8]) {
                BigEndian::write_i32(buffer, self.0);
            }

            fn read(buffer: &[u8]) -> Self {
                QuirkyI32Key(BigEndian::read_i32(buffer))
            }
        }

        let db: Box<Database> = Box::new(MemoryDB::new());
        let mut fork = db.fork();
        {
            let mut index: MapIndex<_, QuirkyI32Key, u64> = MapIndex::new("test_index", &mut fork);
            index.put(&QuirkyI32Key(5), 100);
            index.put(&QuirkyI32Key(-3), 200);
        }
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let index: MapIndex<_, QuirkyI32Key, u64> = MapIndex::new("test_index", snapshot);
        assert_eq!(index.get(&QuirkyI32Key(5)), Some(100));
        assert_eq!(index.get(&QuirkyI32Key(-3)), Some(200));

        // Bunch of counterintuitive behavior here
        assert_eq!(
            index.iter_from(&QuirkyI32Key(-4)).collect::<Vec<_>>(),
            vec![(QuirkyI32Key(-3), 200)]
        );
        assert_eq!(
            index.iter_from(&QuirkyI32Key(-2)).collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(
            index.iter_from(&QuirkyI32Key(1)).collect::<Vec<_>>(),
            vec![(QuirkyI32Key(5), 100), (QuirkyI32Key(-3), 200)]
        );
        assert_eq!(
            index.iter_from(&QuirkyI32Key(6)).collect::<Vec<_>>(),
            vec![(QuirkyI32Key(-3), 200)]
        );

        // Notice the different order of values compared to the previous test
        assert_eq!(index.values().collect::<Vec<_>>(), vec![100, 200]);
    }

    #[test]
    fn test_storage_key_for_system_time_round_trip() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};

        let times = [
            UNIX_EPOCH,
            UNIX_EPOCH + Duration::new(13, 23),
            SystemTime::now(),
            SystemTime::now() + Duration::new(17, 15),
            UNIX_EPOCH + Duration::new(0, u32::max_value()),
            UNIX_EPOCH + Duration::new(i64::max_value() as u64, 0),
            UNIX_EPOCH + Duration::new(i64::max_value() as u64, 999_999_999),
            UNIX_EPOCH + Duration::new(i64::max_value() as u64 - 1, 1_000_000_000),
            UNIX_EPOCH + Duration::new(i64::max_value() as u64 - 4, 4_000_000_000),
            UNIX_EPOCH + Duration::new(i64::max_value() as u64 - 4, u32::max_value()),
        ];

        let mut buffer = [0u8; 12];
        for time in times.iter() {
            time.write(&mut buffer);
            assert_eq!(*time, SystemTime::read(&buffer));
        }
    }

    #[test]
    fn test_storage_key_for_system_time_ordering() {
        use rand::{Rng, thread_rng};
        use std::time::Duration;

        let mut rng = thread_rng();

        let (mut buffer1, mut buffer2) = ([0u8; 12], [0u8; 12]);
        for _ in 0..FUZZ_SAMPLES {
            let time1 = UNIX_EPOCH +
                Duration::new(
                    rng.gen::<u64>() % (i32::max_value() as u64),
                    rng.gen::<u32>() % 1_000_000_000,
                );
            let time2 = UNIX_EPOCH +
                Duration::new(
                    rng.gen::<u64>() % (i32::max_value() as u64),
                    rng.gen::<u32>() % 1_000_000_000,
                );
            time1.write(&mut buffer1);
            time2.write(&mut buffer2);
            assert_eq!(time1.cmp(&time2), buffer1.cmp(&buffer2));
        }
    }

    #[test]
    fn test_system_time_key_in_index() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        use storage::{Database, MapIndex, MemoryDB};

        let db: Box<Database> = Box::new(MemoryDB::new());
        let x1 = UNIX_EPOCH + Duration::new(80, 0);
        let x2 = UNIX_EPOCH + Duration::new(10, 0);
        let y1 = SystemTime::now();
        let y2 = y1 + Duration::new(10, 0);
        let mut fork = db.fork();
        {
            let mut index: MapIndex<_, SystemTime, SystemTime> =
                MapIndex::new("test_index", &mut fork);
            index.put(&x1, y1);
            index.put(&x2, y2);
        }
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let index: MapIndex<_, SystemTime, SystemTime> = MapIndex::new("test_index", snapshot);
        assert_eq!(index.get(&x1), Some(y1));
        assert_eq!(index.get(&x2), Some(y2));

        assert_eq!(
            index.iter_from(&UNIX_EPOCH).collect::<Vec<_>>(),
            vec![(x2, y2), (x1, y1)]
        );
        assert_eq!(
            index
                .iter_from(&(UNIX_EPOCH + Duration::new(20, 0)))
                .collect::<Vec<_>>(),
            vec![(x1, y1)]
        );
        assert_eq!(
            index
                .iter_from(&(UNIX_EPOCH + Duration::new(80, 0)))
                .collect::<Vec<_>>(),
            vec![(x1, y1)]
        );
        assert_eq!(
            index
                .iter_from(&(UNIX_EPOCH + Duration::new(90, 0)))
                .collect::<Vec<_>>(),
            vec![]
        );

        assert_eq!(index.values().collect::<Vec<_>>(), vec![y2, y1]);
    }

    #[test]
    fn str_key() {
        let values = ["eee", "hello world", ""];
        for val in values.iter() {
            let mut buffer = get_buffer(*val);
            val.write(&mut buffer);
            let new_val = str::read(&buffer);
            assert_eq!(new_val, *val);
        }
    }

    #[test]
    fn u8_slice_key() {
        let values: &[&[u8]] = &[&[1, 2, 3], &[255], &[]];
        for val in values.iter() {
            let mut buffer = get_buffer(*val);
            val.write(&mut buffer);
            let new_val = <[u8] as StorageKey>::read(&buffer);
            assert_eq!(new_val, *val);
        }
    }

    fn get_buffer<T: StorageKey + ?Sized>(key: &T) -> Vec<u8> {
        vec![0; key.size()]
    }
}
