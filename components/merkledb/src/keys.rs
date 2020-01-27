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

//! A definition of `BinaryKey` trait and implementations for common types.

use byteorder::{BigEndian, ByteOrder};
use chrono::{DateTime, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use exonum_crypto::{Hash, PublicKey, Signature, HASH_SIZE, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};

/// A type that can be (de)serialized as a key in the blockchain storage.
///
/// Since keys are sorted in the serialized form, the big-endian encoding should be used
/// with unsigned integer types. Note, however, that the big-endian encoding
/// will not sort signed integer types in the natural order; therefore, they are
/// mapped to the corresponding unsigned type by adding a constant to the source value.
///
/// # Examples
///
/// ```
/// use std::mem;
/// use exonum_merkledb::BinaryKey;
///
/// #[derive(Clone)]
/// struct Key {
///     a: i16,
///     b: u32,
/// }
///
/// impl BinaryKey for Key {
///     fn size(&self) -> usize {
///         mem::size_of_val(&self.a) + mem::size_of_val(&self.b)
///     }
///
///     fn write(&self, buffer: &mut [u8]) -> usize {
///         self.a.write(&mut buffer[0..2]);
///         self.b.write(&mut buffer[2..6]);
///         self.size()
///     }
///
///     fn read(buffer: &[u8]) -> Self {
///         let a = i16::read(&buffer[0..2]);
///         let b = u32::read(&buffer[2..6]);
///         Key { a, b }
///     }
/// }
/// # // Check the natural ordering of keys
/// # let (mut x, mut y) = (vec![0_u8; 6], vec![0_u8; 6]);
/// # Key { a: -1, b: 2 }.write(&mut x);
/// # Key { a: 1, b: 513 }.write(&mut y);
/// # assert!(x < y);
/// # // Check the roundtrip
/// # let key = Key::read(&x);
/// # assert_eq!(key.a, -1);
/// # assert_eq!(key.b, 2);
/// ```
pub trait BinaryKey: ToOwned {
    /// Returns the size of the serialized key in bytes.
    fn size(&self) -> usize;

    /// Serializes the key into the specified buffer of bytes.
    ///
    /// The caller must guarantee that the size of the buffer is equal to the precalculated size
    /// of the serialized key returned via `size()`. Returns number of written bytes.
    /// The provided buffer may be uninitialized; an implementor must not read from it.
    // TODO: Should be unsafe? (ECR-174)
    fn write(&self, buffer: &mut [u8]) -> usize;

    /// Deserializes the key from the specified buffer of bytes.
    // TODO: Should be unsafe? (ECR-174)
    fn read(buffer: &[u8]) -> Self::Owned;
}

/// No-op implementation.
impl BinaryKey for () {
    fn size(&self) -> usize {
        0
    }

    fn write(&self, _buffer: &mut [u8]) -> usize {
        // no-op
        self.size()
    }

    fn read(_buffer: &[u8]) -> Self::Owned {}
}

impl BinaryKey for u8 {
    fn size(&self) -> usize {
        1
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer[0] = *self;
        self.size()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        buffer[0]
    }
}

/// Uses encoding with the values mapped to `u8`
/// by adding the corresponding constant (`128`) to the value.
impl BinaryKey for i8 {
    fn size(&self) -> usize {
        1
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer[0] = self.wrapping_add(Self::min_value()) as u8;
        self.size()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        buffer[0].wrapping_sub(Self::min_value() as u8) as Self
    }
}

// spell-checker:ignore utype, itype, vals, ints

macro_rules! storage_key_for_ints {
    ($utype:ident, $itype:ident, $size:expr, $read_method:ident, $write_method:ident) => {
        /// Uses big-endian encoding.
        impl BinaryKey for $utype {
            fn size(&self) -> usize {
                $size
            }

            fn write(&self, buffer: &mut [u8]) -> usize {
                BigEndian::$write_method(buffer, *self);
                self.size()
            }

            fn read(buffer: &[u8]) -> Self {
                BigEndian::$read_method(buffer)
            }
        }

        /// Uses big-endian encoding with the values mapped to the unsigned format
        /// by adding the corresponding constant to the value.
        impl BinaryKey for $itype {
            fn size(&self) -> usize {
                $size
            }

            fn write(&self, buffer: &mut [u8]) -> usize {
                BigEndian::$write_method(buffer, self.wrapping_add(Self::min_value()) as $utype);
                self.size()
            }

            fn read(buffer: &[u8]) -> Self {
                BigEndian::$read_method(buffer).wrapping_sub(Self::min_value() as $utype) as Self
            }
        }
    };
}

storage_key_for_ints! {u16, i16, 2, read_u16, write_u16}
storage_key_for_ints! {u32, i32, 4, read_u32, write_u32}
storage_key_for_ints! {u64, i64, 8, read_u64, write_u64}
storage_key_for_ints! {u128, i128, 16, read_u128, write_u128}

macro_rules! storage_key_for_crypto_types {
    ($type:ident, $size:expr) => {
        impl BinaryKey for $type {
            fn size(&self) -> usize {
                $size
            }

            fn write(&self, buffer: &mut [u8]) -> usize {
                buffer[..self.size()].copy_from_slice(self.as_ref());
                self.size()
            }

            fn read(buffer: &[u8]) -> Self {
                Self::from_slice(buffer).unwrap()
            }
        }
    };
}

storage_key_for_crypto_types! {Hash, HASH_SIZE}
storage_key_for_crypto_types! {PublicKey, PUBLIC_KEY_LENGTH}
storage_key_for_crypto_types! {Signature, SIGNATURE_LENGTH}

impl BinaryKey for Vec<u8> {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer[..self.size()].copy_from_slice(self);
        self.size()
    }

    fn read(buffer: &[u8]) -> Self {
        buffer.to_vec()
    }
}

impl BinaryKey for [u8] {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer[..self.size()].copy_from_slice(self);
        self.size()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        Vec::<u8>::read(buffer)
    }
}

impl BinaryKey for [u8; 32] {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer[..self.size()].copy_from_slice(self);
        self.size()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        let mut value = [0_u8; 32];
        value.copy_from_slice(buffer);
        value
    }
}

/// Uses UTF-8 string serialization.
impl BinaryKey for String {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer[..self.size()].copy_from_slice(self.as_bytes());
        self.size()
    }

    #[cfg(feature = "yolo")]
    #[allow(unsafe_code)]
    fn read(buffer: &[u8]) -> Self::Owned {
        // SAFETY:
        // As with other `BinaryKey` implementations, we assume that data read by the key
        // is trusted, i.e., was created by a previous call to `write()`.
        unsafe { std::str::from_utf8_unchecked(buffer).to_string() }
    }

    #[cfg(not(feature = "yolo"))]
    fn read(buffer: &[u8]) -> Self::Owned {
        const ERROR_MSG: &str =
            "Error reading UTF-8 string from the database. \
             Probable reason is data schema mismatch; for example, data was written to \
             `MapIndex<u64, _>` and is read as `MapIndex<str, _>`";
        std::str::from_utf8(buffer).expect(ERROR_MSG).to_string()
    }
}

impl BinaryKey for str {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer[..self.size()].copy_from_slice(self.as_bytes());
        self.size()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        String::read(buffer)
    }
}

/// `chrono::DateTime` uses only 12 bytes in the storage. It is represented by number of seconds
/// since `1970-01-01 00:00:00 UTC`, which are stored in the first 8 bytes as per the `BinaryKey`
/// implementation for `i64`, and nanoseconds, which are stored in the remaining 4 bytes as per
/// the `BinaryKey` implementation for `u32`.
impl BinaryKey for DateTime<Utc> {
    fn size(&self) -> usize {
        12
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        let secs = self.timestamp();
        let nanos = self.timestamp_subsec_nanos();
        secs.write(&mut buffer[0..8]);
        nanos.write(&mut buffer[8..12]);
        self.size()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        let secs = i64::read(&buffer[0..8]);
        let nanos = u32::read(&buffer[8..12]);
        Self::from_utc(NaiveDateTime::from_timestamp(secs, nanos), Utc)
    }
}

impl BinaryKey for Uuid {
    fn size(&self) -> usize {
        16
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer.copy_from_slice(self.as_bytes());
        self.size()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        Self::from_slice(buffer).unwrap()
    }
}

impl BinaryKey for Decimal {
    fn size(&self) -> usize {
        16
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer.copy_from_slice(&self.serialize());
        self.size()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(buffer);
        Self::deserialize(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::CopyAccessExt;

    use std::{fmt::Debug, str::FromStr};

    use chrono::{Duration, TimeZone};
    use hex::FromHex;

    // Number of samples for fuzz testing
    const FUZZ_SAMPLES: usize = 100_000;

    macro_rules! test_storage_key_for_int_type {
        (full $type:ident, $size:expr => $test_name:ident) => {
            #[test]
            #[allow(clippy::replace_consts)]
            fn $test_name() {
                use std::iter::once;

                const MIN: $type = std::$type::MIN;
                const MAX: $type = std::$type::MAX;

                // Roundtrip
                let mut buffer = [0_u8; $size];
                for x in (MIN..MAX).chain(once(MAX)) {
                    x.write(&mut buffer);
                    assert_eq!($type::read(&buffer), x);
                }

                // Ordering
                let (mut x_buffer, mut y_buffer) = ([0_u8; $size], [0_u8; $size]);
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
                use rand::{distributions::Standard, thread_rng, Rng};
                let rng = thread_rng();

                // Fuzzed roundtrip
                let mut buffer = [0_u8; $size];
                let handpicked_vals = vec![$type::min_value(), $type::max_value()];
                for x in rng
                    .sample_iter(&Standard)
                    .take(FUZZ_SAMPLES)
                    .chain(handpicked_vals)
                {
                    x.write(&mut buffer);
                    assert_eq!($type::read(&buffer), x);
                }

                // Fuzzed ordering
                let (mut x_buffer, mut y_buffer) = ([0_u8; $size], [0_u8; $size]);
                let mut vals: Vec<$type> = rng.sample_iter(&Standard).take(FUZZ_SAMPLES).collect();
                vals.sort();
                for w in vals.windows(2) {
                    let (x, y) = (w[0], w[1]);
                    if x == y {
                        continue;
                    }

                    x.write(&mut x_buffer);
                    y.write(&mut y_buffer);
                    assert!(x_buffer < y_buffer);
                }
            }
        };
    }

    test_storage_key_for_int_type! {full  u8, 1 => test_storage_key_for_u8}
    test_storage_key_for_int_type! {full  i8, 1 => test_storage_key_for_i8}
    test_storage_key_for_int_type! {full u16, 2 => test_storage_key_for_u16}
    test_storage_key_for_int_type! {full i16, 2 => test_storage_key_for_i16}
    test_storage_key_for_int_type! {fuzz u32, 4 => test_storage_key_for_u32}
    test_storage_key_for_int_type! {fuzz i32, 4 => test_storage_key_for_i32}
    test_storage_key_for_int_type! {fuzz u64, 8 => test_storage_key_for_u64}
    test_storage_key_for_int_type! {fuzz i64, 8 => test_storage_key_for_i64}
    test_storage_key_for_int_type! {fuzz u128, 16 => test_storage_key_for_u128}
    test_storage_key_for_int_type! {fuzz i128, 16 => test_storage_key_for_i128}

    #[test]
    fn test_signed_int_key_in_index() {
        use crate::{Database, MapIndex, TemporaryDB};

        let db: Box<dyn Database> = Box::new(TemporaryDB::default());
        let fork = db.fork();
        {
            let mut index: MapIndex<_, i32, u64> = fork.get_map("test_index");
            index.put(&5, 100);
            index.put(&-3, 200);
        }
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let index: MapIndex<_, i32, u64> = snapshot.get_map("test_index");
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

    // Example how to migrate from Exonum <= 0.5 implementation of `BinaryKey`
    // for signed integers.
    #[test]
    fn test_old_signed_int_key_in_index() {
        use crate::{Database, MapIndex, TemporaryDB};

        // Simple wrapper around a signed integer type with the `BinaryKey` implementation,
        // which was used in Exonum <= 0.5.
        #[derive(Debug, PartialEq, Clone)]
        struct QuirkyI32Key(i32);

        impl BinaryKey for QuirkyI32Key {
            fn size(&self) -> usize {
                4
            }

            fn write(&self, buffer: &mut [u8]) -> usize {
                BigEndian::write_i32(buffer, self.0);
                self.size()
            }

            fn read(buffer: &[u8]) -> Self {
                Self(BigEndian::read_i32(buffer))
            }
        }

        let db: Box<dyn Database> = Box::new(TemporaryDB::default());
        let fork = db.fork();
        {
            let mut index: MapIndex<_, QuirkyI32Key, u64> = fork.get_map("test_index");
            index.put(&QuirkyI32Key(5), 100);
            index.put(&QuirkyI32Key(-3), 200);
        }
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let index: MapIndex<_, QuirkyI32Key, u64> = snapshot.get_map("test_index");
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
    fn test_storage_key_for_chrono_date_time_round_trip() {
        let times = [
            Utc.timestamp(0, 0),
            Utc.timestamp(13, 23),
            Utc::now(),
            Utc::now() + Duration::seconds(17) + Duration::nanoseconds(15),
            Utc.timestamp(0, 999_999_999),
            Utc.timestamp(0, 1_500_000_000), // leap second
        ];

        assert_round_trip_eq(&times);
    }

    #[test]
    fn test_storage_key_for_system_time_ordering() {
        use rand::{thread_rng, Rng};

        let mut rng = thread_rng();

        let (mut buffer1, mut buffer2) = ([0_u8; 12], [0_u8; 12]);
        for _ in 0..FUZZ_SAMPLES {
            let time1 = Utc.timestamp(
                rng.gen::<i64>() % i64::from(i32::max_value()),
                rng.gen::<u32>() % 1_000_000_000,
            );
            let time2 = Utc.timestamp(
                rng.gen::<i64>() % i64::from(i32::max_value()),
                rng.gen::<u32>() % 1_000_000_000,
            );
            time1.write(&mut buffer1);
            time2.write(&mut buffer2);
            assert_eq!(time1.cmp(&time2), buffer1.cmp(&buffer2));
        }
    }

    #[test]
    fn test_system_time_key_in_index() {
        use crate::{Database, MapIndex, TemporaryDB};

        let db: Box<dyn Database> = Box::new(TemporaryDB::default());
        let x1 = Utc.timestamp(80, 0);
        let x2 = Utc.timestamp(10, 0);
        let y1 = Utc::now();
        let y2 = y1 + Duration::seconds(10);
        let fork = db.fork();
        {
            let mut index: MapIndex<_, DateTime<Utc>, DateTime<Utc>> = fork.get_map("test_index");
            index.put(&x1, y1);
            index.put(&x2, y2);
        }
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let index: MapIndex<_, DateTime<Utc>, DateTime<Utc>> = snapshot.get_map("test_index");
        assert_eq!(index.get(&x1), Some(y1));
        assert_eq!(index.get(&x2), Some(y2));

        assert_eq!(
            index.iter_from(&Utc.timestamp(0, 0)).collect::<Vec<_>>(),
            vec![(x2, y2), (x1, y1)]
        );
        assert_eq!(
            index.iter_from(&Utc.timestamp(20, 0)).collect::<Vec<_>>(),
            vec![(x1, y1)]
        );
        assert_eq!(
            index.iter_from(&Utc.timestamp(80, 0)).collect::<Vec<_>>(),
            vec![(x1, y1)]
        );
        assert_eq!(
            index.iter_from(&Utc.timestamp(90, 0)).collect::<Vec<_>>(),
            vec![]
        );

        assert_eq!(index.values().collect::<Vec<_>>(), vec![y2, y1]);
    }

    #[test]
    fn test_str_key() {
        let values = ["eee", "hello world", ""];
        for val in &values {
            let mut buffer = get_buffer(*val);
            val.write(&mut buffer);
            let new_val = str::read(&buffer);
            assert_eq!(new_val, *val);
        }
    }

    #[test]
    #[cfg(not(feature = "yolo"))]
    #[should_panic(expected = "Error reading UTF-8 string")]
    fn test_str_key_error() {
        let buffer = &[0xfe_u8, 0xfd];
        str::read(buffer);
    }

    #[test]
    fn test_u8_slice_key() {
        let values: &[&[u8]] = &[&[1, 2, 3], &[255], &[]];
        for val in values.iter() {
            let mut buffer = get_buffer(*val);
            val.write(&mut buffer);
            let new_val = <[u8] as BinaryKey>::read(&buffer);
            assert_eq!(new_val, *val);
        }
    }

    #[test]
    fn test_hash_round_trip() {
        let hashes =
            [
                Hash::from_hex("326c1da1a00b5b4c85929dac57f3c99ceea82ed2941173d879c57b8f21ae8c78")
                    .unwrap(),
            ];
        assert_round_trip_eq(&hashes);
    }

    #[test]
    fn test_public_key_round_trip() {
        let hashes = [PublicKey::from_hex(
            "1e38d80b8a9786648a471b11a9624a9519215743df7321938d70bac73dae3b84",
        )
        .unwrap()];
        assert_round_trip_eq(&hashes);
    }

    #[test]
    fn test_signature_round_trip() {
        let hashes = [Signature::from_hex("326c1da1a00b5b4c85929dac57f3c99ceea82ed2941173d879c57b8f21ae8c781e38d80b8a9786648a471b11a9624a9519215743df7321938d70bac73dae3b84").unwrap()];
        assert_round_trip_eq(&hashes);
    }

    #[test]
    fn test_uuid_round_trip() {
        let uuids = [
            Uuid::nil(),
            Uuid::parse_str("936DA01F9ABD4d9d80C702AF85C822A8").unwrap(),
            Uuid::parse_str("0000002a-000c-0005-0c03-0938362b0809").unwrap(),
        ];

        assert_round_trip_eq(&uuids);
    }

    #[test]
    fn test_decimal_round_trip() {
        let decimals = [
            Decimal::from_str("3.14").unwrap(),
            Decimal::from_parts(1_102_470_952, 185_874_565, 1_703_060_790, false, 28),
            Decimal::new(9_497_628_354_687_268, 12),
            Decimal::from_str("0").unwrap(),
            Decimal::from_str("-0.000000000000000000019").unwrap(),
        ];

        assert_round_trip_eq(&decimals);
    }

    fn assert_round_trip_eq<T>(values: &[T])
    where
        T: BinaryKey + PartialEq<<T as ToOwned>::Owned> + Debug,
        <T as ToOwned>::Owned: Debug,
    {
        for original_value in values.iter() {
            let mut buffer = get_buffer(original_value);
            original_value.write(&mut buffer);
            let new_value = <T as BinaryKey>::read(&buffer);
            assert_eq!(*original_value, new_value);
        }
    }

    fn get_buffer<T: BinaryKey + ?Sized>(key: &T) -> Vec<u8> {
        vec![0; key.size()]
    }
}
