// Copyright 2018 The Exonum Team
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

//! A definition of `StorageValue` trait and implementations for common types.

use byteorder::{ByteOrder, LittleEndian};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use std::{borrow::Cow, mem};

use super::UniqueHash;
use crypto::{Hash, PublicKey};
use encoding::{Field, Offset};
use helpers::Round;
use messages::{MessageBuffer, RawMessage};

/// A type that can be (de)serialized as a value in the blockchain storage.
///
/// `StorageValue` is automatically implemented by the [`encoding_struct!`] and [`transactions!`]
/// macros. In case you need to implement it manually, use little-endian encoding
/// for integer types for compatibility with modern architectures.
///
/// # Examples
///
/// Implementing `StorageValue` for the type:
///
/// ```
/// # extern crate exonum;
/// # extern crate byteorder;
/// use std::borrow::Cow;
/// use exonum::storage::StorageValue;
/// use exonum::crypto::{self, CryptoHash, Hash};
/// use byteorder::{LittleEndian, ByteOrder};
///
/// struct Data {
///     a: i16,
///     b: u32,
/// }
///
/// impl CryptoHash for Data {
///     fn hash(&self) -> Hash {
///         let mut buffer = [0; 6];
///         LittleEndian::write_i16(&mut buffer[0..2], self.a);
///         LittleEndian::write_u32(&mut buffer[2..6], self.b);
///         crypto::hash(&buffer)
///     }
/// }
///
/// impl StorageValue for Data {
///     fn into_bytes(self) -> Vec<u8> {
///         let mut buffer = vec![0; 6];
///         LittleEndian::write_i16(&mut buffer[0..2], self.a);
///         LittleEndian::write_u32(&mut buffer[2..6], self.b);
///         buffer
///     }
///
///     fn from_bytes(value: Cow<[u8]>) -> Self {
///         let a = LittleEndian::read_i16(&value[0..2]);
///         let b = LittleEndian::read_u32(&value[2..6]);
///         Data { a, b }
///     }
/// }
/// # fn main() {}
/// ```
///
/// [`encoding_struct!`]: ../macro.encoding_struct.html
/// [`transactions!`]: ../macro.transactions.html
pub trait StorageValue: UniqueHash + Sized {
    /// Serialize a value into a vector of bytes.
    fn into_bytes(self) -> Vec<u8>;

    /// Deserialize a value from bytes.
    fn from_bytes(value: Cow<[u8]>) -> Self;
}

/// No-op implementation.
impl StorageValue for () {
    fn into_bytes(self) -> Vec<u8> {
        Vec::new()
    }

    fn from_bytes(_value: Cow<[u8]>) -> Self {
        ()
    }
}

impl StorageValue for bool {
    fn into_bytes(self) -> Vec<u8> {
        vec![self as u8]
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        assert_eq!(value.len(), 1);

        match value[0] {
            0 => false,
            1 => true,
            value => panic!("Invalid value for bool: {}", value),
        }
    }
}

impl StorageValue for u8 {
    fn into_bytes(self) -> Vec<u8> {
        vec![self]
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        assert_eq!(value.len(), 1);
        value[0]
    }
}

/// Uses little-endian encoding.
impl StorageValue for u16 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; 2];
        LittleEndian::write_u16(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_u16(value.as_ref())
    }
}

/// Uses little-endian encoding.
impl StorageValue for u32 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; 4];
        LittleEndian::write_u32(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_u32(value.as_ref())
    }
}

/// Uses little-endian encoding.
impl StorageValue for u64 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u64>()];
        LittleEndian::write_u64(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_u64(value.as_ref())
    }
}

impl StorageValue for i8 {
    fn into_bytes(self) -> Vec<u8> {
        vec![self as u8]
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        assert_eq!(value.len(), 1);
        value[0] as i8
    }
}

/// Uses little-endian encoding.
impl StorageValue for i16 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; 2];
        LittleEndian::write_i16(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_i16(value.as_ref())
    }
}

/// Uses little-endian encoding.
impl StorageValue for i32 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; 4];
        LittleEndian::write_i32(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_i32(value.as_ref())
    }
}

/// Uses little-endian encoding.
impl StorageValue for i64 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; 8];
        LittleEndian::write_i64(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_i64(value.as_ref())
    }
}

impl StorageValue for Hash {
    fn into_bytes(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        Self::from_slice(value.as_ref()).unwrap()
    }
}

impl StorageValue for PublicKey {
    fn into_bytes(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        Self::from_slice(value.as_ref()).unwrap()
    }
}

impl StorageValue for RawMessage {
    fn into_bytes(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        Self::new(MessageBuffer::from_vec(value.into_owned()))
    }
}

impl StorageValue for Vec<u8> {
    fn into_bytes(self) -> Vec<u8> {
        self
    }

    fn from_bytes(value: Cow<[u8]>) -> Vec<u8> {
        value.into_owned()
    }
}

/// Uses UTF-8 string serialization.
impl StorageValue for String {
    fn into_bytes(self) -> Vec<u8> {
        Self::into_bytes(self)
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        Self::from_utf8(value.into_owned()).unwrap()
    }
}

/// Uses little-endian encoding.
impl StorageValue for DateTime<Utc> {
    fn into_bytes(self) -> Vec<u8> {
        let secs = self.timestamp();
        let nanos = self.timestamp_subsec_nanos();

        let mut buffer = vec![0; 12];
        LittleEndian::write_i64(&mut buffer[0..8], secs);
        LittleEndian::write_u32(&mut buffer[8..12], nanos);
        buffer
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        let secs = LittleEndian::read_i64(&value[0..8]);
        let nanos = LittleEndian::read_u32(&value[8..12]);
        Self::from_utc(NaiveDateTime::from_timestamp(secs, nanos), Utc)
    }
}

/// Uses little-endian encoding.
impl StorageValue for Duration {
    fn into_bytes(self) -> Vec<u8> {
        let mut buffer = vec![0; Self::field_size() as usize];
        let from: Offset = 0;
        let to: Offset = Self::field_size();
        self.write(&mut buffer, from, to);
        buffer
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        #![allow(unsafe_code)]
        let from: Offset = 0;
        let to: Offset = Self::field_size();
        unsafe { Self::read(&value, from, to) }
    }
}

impl StorageValue for Round {
    fn into_bytes(self) -> Vec<u8> {
        self.0.into_bytes()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        Round(<u32 as StorageValue>::from_bytes(value))
    }
}

impl StorageValue for Uuid {
    fn into_bytes(self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        Uuid::from_slice(&value).unwrap()
    }
}

impl StorageValue for Decimal {
    fn into_bytes(self) -> Vec<u8> {
        self.serialize().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        let mut buf: [u8; 16] = [0; 16];
        buf.copy_from_slice(&value);
        Self::deserialize(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Debug;
    use std::str::FromStr;

    #[test]
    fn u8_round_trip() {
        let values = [u8::min_value(), 1, u8::max_value()];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn i8_round_trip() {
        let values = [i8::min_value(), -1, 0, 1, i8::max_value()];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn u16_round_trip() {
        let values = [u16::min_value(), 1, u16::max_value()];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn i16_round_trip() {
        let values = [i16::min_value(), -1, 0, 1, i16::max_value()];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn u32_round_trip() {
        let values = [u32::min_value(), 1, u32::max_value()];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn i32_round_trip() {
        let values = [i32::min_value(), -1, 0, 1, i32::max_value()];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn u64_round_trip() {
        let values = [u64::min_value(), 1, u64::max_value()];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn i64_round_trip() {
        let values = [i64::min_value(), -1, 0, 1, i64::max_value()];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn bool_round_trip() {
        let values = [false, true];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn vec_round_trip() {
        let values = [vec![], vec![1], vec![1, 2, 3], vec![255; 100]];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn string_round_trip() {
        let values: Vec<_> = ["", "e", "2", "hello"]
            .iter()
            .map(|v| v.to_string())
            .collect();

        assert_round_trip_eq(&values);
    }

    #[test]
    fn storage_value_for_system_time_round_trip() {
        use chrono::TimeZone;

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
    fn storage_value_for_duration_round_trip() {
        let durations = [
            Duration::zero(),
            Duration::max_value(),
            Duration::min_value(),
            Duration::nanoseconds(999_999_999),
            Duration::nanoseconds(-999_999_999),
            Duration::seconds(42) + Duration::nanoseconds(15),
            Duration::seconds(-42) + Duration::nanoseconds(-15),
        ];

        assert_round_trip_eq(&durations);
    }

    #[test]
    fn round_round_trip() {
        let values = [
            Round::zero(),
            Round::first(),
            Round(100),
            Round(u32::max_value()),
        ];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn uuid_round_trip() {
        let values = [
            Uuid::nil(),
            Uuid::parse_str("936DA01F9ABD4d9d80C702AF85C822A8").unwrap(),
            Uuid::parse_str("0000002a-000c-0005-0c03-0938362b0809").unwrap(),
        ];

        assert_round_trip_eq(&values);
    }

    #[test]
    fn decimal_round_trip() {
        let values = [
            Decimal::from_str("3.14").unwrap(),
            Decimal::from_parts(1102470952, 185874565, 1703060790, false, 28),
            Decimal::new(9497628354687268, 12),
            Decimal::from_str("0").unwrap(),
            Decimal::from_str("-0.000000000000000000019").unwrap(),
        ];

        assert_round_trip_eq(&values);
    }

    fn assert_round_trip_eq<T: StorageValue + Clone + PartialEq + Debug>(values: &[T]) {
        for value in values.into_iter() {
            let bytes = value.clone().into_bytes();
            assert_eq!(
                *value,
                <T as StorageValue>::from_bytes(Cow::Borrowed(&bytes))
            );
        }
    }
}
