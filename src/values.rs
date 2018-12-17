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

//! A definition of `BinaryForm` trait and implementations for common types.

use std::io::{Read, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use chrono::{DateTime, NaiveDateTime, Utc};
use failure::{self, format_err};
use rust_decimal::Decimal;
use uuid::Uuid;

use super::UniqueHash;
use exonum_crypto::{Hash, PublicKey};

/// A type that can be (de)serialized as a value in the blockchain storage.
///
/// # Examples
///
/// Implementing `BinaryForm` for the type:
///
/// ```
/// use crate::BinaryForm;
/// use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
/// use failure;
/// use std::io::{Read, Write};
///
/// #[derive(Clone)]
/// struct Data {
///     a: i16,
///     b: u32,
/// }
///
/// impl BinaryForm for Data {
///     fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
///         to.write_i16::<LittleEndian>(self.a)?;
///         to.write_u32::<LittleEndian>(self.b)?;
///         Ok(())
///     }
///
///     fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
///         let a = from.read_i16::<LittleEndian>()?;
///         let b = from.read_u32::<LittleEndian>()?;
///         Ok(Self { a, b })
///     }
/// }
/// # fn main() {}
/// ```
pub trait BinaryForm: Sized {
    fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error>;

    fn decode(from: &mut impl Read) -> Result<Self, failure::Error>;

    fn size_hint(&self) -> Option<usize> {
        Some(std::mem::size_of_val(self))
    }

    fn to_bytes(&self) -> Result<Vec<u8>, failure::Error> {
        let mut buf = self
            .size_hint()
            .map_or_else(Vec::default, Vec::with_capacity);
        self.encode(&mut buf)?;
        Ok(buf)
    }

    fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, failure::Error> {
        Self::decode(&mut bytes.as_ref())
    }
}

macro_rules! impl_binary_form_scalar {
    ($type:tt, $write:ident, $read:ident) => {
        impl BinaryForm for $type {
            fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
                use byteorder::WriteBytesExt;
                to.$write(*self).map_err(failure::Error::from)
            }

            fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
                use byteorder::ReadBytesExt;
                from.$read().map_err(failure::Error::from)
            }
        }

        impl UniqueHash for $type {}
    };
    ($type:tt, $write:ident, $read:ident, $len:expr) => {
        impl BinaryForm for $type {
            fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
                use byteorder::{LittleEndian, WriteBytesExt};
                to.$write::<LittleEndian>(*self)
                    .map_err(failure::Error::from)
            }

            fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
                use byteorder::{LittleEndian, ReadBytesExt};
                from.$read::<LittleEndian>().map_err(failure::Error::from)
            }

            fn size_hint(&self) -> Option<usize> {
                Some($len)
            }
        }

        impl UniqueHash for $type {}
    };
}

// Unsigned scalar types
impl_binary_form_scalar! { u8,  write_u8,  read_u8 }
impl_binary_form_scalar! { u16, write_u16, read_u16, 2 }
impl_binary_form_scalar! { u32, write_u32, read_u32, 4 }
impl_binary_form_scalar! { u64, write_u64, read_u64, 8 }
// Signed scalar types
impl_binary_form_scalar! { i8,  write_i8,  read_i8 }
impl_binary_form_scalar! { i16, write_i16, read_i16, 2 }
impl_binary_form_scalar! { i32, write_i32, read_i32, 4 }
impl_binary_form_scalar! { i64, write_i64, read_i64, 8 }

/// No-op implementation.
impl BinaryForm for () {
    fn encode(&self, _to: &mut impl Write) -> Result<(), failure::Error> {
        Ok(())
    }

    fn decode(_from: &mut impl Read) -> Result<Self, failure::Error> {
        Ok(())
    }
}

impl UniqueHash for () {}

impl BinaryForm for bool {
    fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
        (*self as u8).encode(to)
    }

    fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
        let value = u8::decode(from)?;
        match value {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(format_err!("Invalid value for bool: {}", other)),
        }
    }
}

impl UniqueHash for bool {}

impl BinaryForm for Vec<u8> {
    fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
        to.write_all(self.as_ref()).map_err(failure::Error::from)
    }

    fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
        let mut buf = Self::new();
        from.read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl UniqueHash for Vec<u8> {}

impl BinaryForm for String {
    fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
        to.write_all(self.as_ref()).map_err(failure::Error::from)
    }

    fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
        let mut buf = Vec::new();
        from.read_to_end(&mut buf)?;
        Self::from_utf8(buf).map_err(failure::Error::from)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl UniqueHash for String {}

impl BinaryForm for Hash {
    fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
        to.write_all(self.as_ref()).map_err(failure::Error::from)
    }

    fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
        let mut buf = Vec::new();
        from.read_to_end(&mut buf)?;
        Self::from_slice(buf.as_ref())
            .ok_or_else(|| format_err!("Unable to decode value from bytes"))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.as_ref().len())
    }
}

impl BinaryForm for PublicKey {
    fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
        to.write_all(self.as_ref()).map_err(failure::Error::from)
    }

    fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
        let mut buf = Vec::new();
        from.read_to_end(&mut buf)?;
        Self::from_slice(buf.as_ref())
            .ok_or_else(|| format_err!("Unable to decode value from bytes"))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.as_ref().len())
    }
}

impl UniqueHash for PublicKey {}

// FIXME Maybe we should remove this implementations

impl BinaryForm for DateTime<Utc> {
    fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
        to.write_i64::<LittleEndian>(self.timestamp())?;
        to.write_u32::<LittleEndian>(self.timestamp_subsec_nanos())?;
        Ok(())
    }

    fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
        let secs = from.read_i64::<LittleEndian>()?;
        let nanos = from.read_u32::<LittleEndian>()?;
        Ok(Self::from_utc(
            NaiveDateTime::from_timestamp(secs, nanos),
            Utc,
        ))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(12)
    }
}

impl UniqueHash for DateTime<Utc> {}

impl BinaryForm for Uuid {
    fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
        to.write_all(self.as_bytes()).map_err(failure::Error::from)
    }

    fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
        let mut buf = Vec::new();
        from.read_to_end(&mut buf)?;
        Self::from_slice(&buf).map_err(failure::Error::from)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.as_bytes().len())
    }
}

impl UniqueHash for Uuid {}

impl BinaryForm for Decimal {
    fn encode(&self, to: &mut impl Write) -> Result<(), failure::Error> {
        to.write_all(&self.serialize())
            .map_err(failure::Error::from)
    }

    fn decode(from: &mut impl Read) -> Result<Self, failure::Error> {
        let mut buf: [u8; 16] = [0; 16];
        from.read_exact(&mut buf)?;
        Ok(Self::deserialize(buf))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(16)
    }
}

impl UniqueHash for Decimal {}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::str::FromStr;

    use chrono::Duration;

    use super::*;

    fn assert_round_trip_eq<T: BinaryForm + PartialEq + Debug>(values: &[T]) {
        for value in values {
            let bytes = value.to_bytes().unwrap();
            assert_eq!(*value, <T as BinaryForm>::from_bytes(bytes).unwrap());
        }
    }

    macro_rules! impl_test_binary_form_scalar_unsigned {
        ($name:ident, $type:tt) => {
            #[test]
            fn $name() {
                let values = [$type::min_value(), 1, $type::max_value()];
                assert_round_trip_eq(&values);
            }
        };
    }

    macro_rules! impl_test_binary_form_scalar_signed {
        ($name:ident, $type:tt) => {
            #[test]
            fn $name() {
                let values = [$type::min_value(), -1, 0, 1, $type::max_value()];
                assert_round_trip_eq(&values);
            }
        };
    }

    // Impl tests for unsigned scalar types
    impl_test_binary_form_scalar_unsigned! { test_binary_form_round_trip_u8,  u8 }
    impl_test_binary_form_scalar_unsigned! { test_binary_form_round_trip_u32, u32 }
    impl_test_binary_form_scalar_unsigned! { test_binary_form_round_trip_u16, u16 }
    impl_test_binary_form_scalar_unsigned! { test_binary_form_round_trip_u64, u64 }

    // Impl tests for signed scalar types
    impl_test_binary_form_scalar_signed! { test_binary_form_round_trip_i8,  i8 }
    impl_test_binary_form_scalar_signed! { test_binary_form_round_trip_i16, i16 }
    impl_test_binary_form_scalar_signed! { test_binary_form_round_trip_i32, i32 }
    impl_test_binary_form_scalar_signed! { test_binary_form_round_trip_i64, i64 }

    // Tests for the other types

    #[test]
    fn test_binary_form_vec_u8() {
        let values = [vec![], vec![1], vec![1, 2, 3], vec![255; 100]];
        assert_round_trip_eq(&values);
    }

    #[test]
    fn test_binary_form_bool_correct() {
        let values = [true, false];
        assert_round_trip_eq(&values);
    }

    #[test]
    #[should_panic(expected = "Invalid value for bool: 2")]
    fn test_binary_form_bool_incorrect() {
        let bytes = 2.to_bytes().unwrap();
        <bool as BinaryForm>::from_bytes(&bytes).unwrap();
    }

    #[test]
    fn test_binary_form_string() {
        let values: Vec<_> = ["", "e", "2", "hello"]
            .iter()
            .map(|v| v.to_string())
            .collect();
        assert_round_trip_eq(&values);
    }

    #[test]
    fn test_binary_form_datetime() {
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
    fn test_binary_form_uuid() {
        let values = [
            Uuid::nil(),
            Uuid::parse_str("936DA01F9ABD4d9d80C702AF85C822A8").unwrap(),
            Uuid::parse_str("0000002a-000c-0005-0c03-0938362b0809").unwrap(),
        ];
        assert_round_trip_eq(&values);
    }

    #[test]
    fn test_binary_form_decimal() {
        let values = [
            Decimal::from_str("3.14").unwrap(),
            Decimal::from_parts(1102470952, 185874565, 1703060790, false, 28),
            Decimal::new(9497628354687268, 12),
            Decimal::from_str("0").unwrap(),
            Decimal::from_str("-0.000000000000000000019").unwrap(),
        ];
        assert_round_trip_eq(&values);
    }
}
