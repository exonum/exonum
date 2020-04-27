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

//! Protobuf generated structs and traits for conversion.
//!
//! The central part of this module is [`ProtobufConvert`](./trait.ProtobufConvert.html).
//! The main purpose of this trait is to allow
//! users to create a map between their types and the types generated from .proto descriptions, while
//! providing a mechanism for additional validation of protobuf data.
//!
//! Most of the time you do not have to implement this trait because most of the use cases are covered
//! by `#[derive(ProtobufConvert)]` from `exonum_derive` crate.
//!
//! A typical example of such mapping with validation is manual implementation of this trait for `crypto::Hash`.
//! `crypto::Hash` is a fixed sized array of bytes but protobuf does not allow us to express this constraint since
//! only dynamically sized arrays are supported.
//! If you would like to use `Hash` as a part of your
//! protobuf struct, you would have to write a conversion function from protobuf `proto::Hash`(which
//! is dynamically sized array of bytes) to`crypto::Hash` and call it every time when you want to
//! use `crypto::Hash` in your application.
//!
//! The provided `ProtobufConvert` implementation for Hash allows you to embed this field into your
//! struct and generate `ProtobufConvert` for it using `#[derive(ProtobufConvert)]`, which will validate
//! your struct based on the validation function for `Hash`.

#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]
#![warn(clippy::pedantic)]
#![allow(
    // Next `cast_*` lints don't give alternatives.
    clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss,
    // Next lints produce too much noise/false positives.
    clippy::module_name_repetitions, clippy::similar_names, clippy::must_use_candidate,
    clippy::pub_enum_variant_names,
    // '... may panic' lints.
    clippy::indexing_slicing,
    // Too much work to fix.
    clippy::missing_errors_doc
)]

#[macro_use]
extern crate serde_derive; // Required for Protobuf.

pub use protobuf_convert::*;

pub mod proto;

use anyhow::{ensure, format_err, Error};
use chrono::{DateTime, TimeZone, Utc};
use protobuf::well_known_types;
use serde::{de::Visitor, Deserializer, Serializer};

use std::{collections::HashMap, convert::TryFrom, fmt};

use crate::proto::bit_vec::BitVec;

#[cfg(test)]
mod tests;

/// Used for establishing correspondence between a Rust struct and a type generated from Protobuf.
pub trait ProtobufConvert: Sized {
    /// Type generated from the Protobuf definition.
    type ProtoStruct;

    /// Performs conversion to the type generated from Protobuf.
    fn to_pb(&self) -> Self::ProtoStruct;
    /// Performs conversion from the type generated from Protobuf.
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error>;
}

impl ProtobufConvert for DateTime<Utc> {
    type ProtoStruct = well_known_types::Timestamp;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut ts = Self::ProtoStruct::new();
        ts.set_seconds(self.timestamp());
        ts.set_nanos(self.timestamp_subsec_nanos() as i32);
        ts
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Utc.timestamp_opt(pb.get_seconds(), pb.get_nanos() as u32)
            .single()
            .ok_or_else(|| format_err!("Failed to convert timestamp from bytes"))
    }
}

impl ProtobufConvert for String {
    type ProtoStruct = Self;

    fn to_pb(&self) -> Self::ProtoStruct {
        self.clone()
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(pb)
    }
}

impl ProtobufConvert for u16 {
    type ProtoStruct = u32;

    fn to_pb(&self) -> Self::ProtoStruct {
        u32::from(*self)
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        u16::try_from(pb).map_err(|_| format_err!("Value is out of range"))
    }
}

impl ProtobufConvert for i16 {
    type ProtoStruct = i32;

    fn to_pb(&self) -> Self::ProtoStruct {
        i32::from(*self)
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        i16::try_from(pb).map_err(|_| format_err!("Value is out of range"))
    }
}

impl<T> ProtobufConvert for Vec<T>
where
    T: ProtobufConvert,
{
    type ProtoStruct = Vec<T::ProtoStruct>;

    fn to_pb(&self) -> Self::ProtoStruct {
        self.iter().map(ProtobufConvert::to_pb).collect()
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        pb.into_iter()
            .map(ProtobufConvert::from_pb)
            .collect::<Result<Vec<_>, _>>()
    }
}

impl ProtobufConvert for () {
    type ProtoStruct = protobuf::well_known_types::Empty;

    fn to_pb(&self) -> Self::ProtoStruct {
        Self::ProtoStruct::default()
    }

    fn from_pb(_pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(())
    }
}

/// Special case for protobuf bytes.
impl ProtobufConvert for Vec<u8> {
    type ProtoStruct = Vec<u8>;

    fn to_pb(&self) -> Self::ProtoStruct {
        self.clone()
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(pb)
    }
}

// According to protobuf specification only simple scalar types (not floats) and strings can be used
// as a map keys.
impl<K, T, S> ProtobufConvert for HashMap<K, T, S>
where
    K: Eq + std::hash::Hash + std::fmt::Debug + Clone,
    T: ProtobufConvert,
    S: Default + std::hash::BuildHasher,
{
    type ProtoStruct = HashMap<K, T::ProtoStruct, S>;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.iter().map(|(k, v)| (k.clone(), v.to_pb())).collect()
    }
    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, Error> {
        pb.drain()
            .map(|(k, v)| ProtobufConvert::from_pb(v).map(|v| (k, v)))
            .collect::<Result<HashMap<_, _, _>, _>>()
    }
}

impl ProtobufConvert for bit_vec::BitVec {
    type ProtoStruct = BitVec;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut bit_vec = BitVec::new();
        bit_vec.set_data(self.to_bytes());
        bit_vec.set_len(self.len() as u64);
        bit_vec
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        let data = pb.get_data();
        let mut bit_vec = bit_vec::BitVec::from_bytes(data);
        bit_vec.truncate(pb.get_len() as usize);
        Ok(bit_vec)
    }
}

macro_rules! impl_protobuf_convert_scalar {
    ( $( $name:tt ),* )=> {
        $(
            impl ProtobufConvert for $name {
                type ProtoStruct = $name;
                fn to_pb(&self) -> Self::ProtoStruct {
                    *self
                }
                fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
                    Ok(pb)
                }
            }
        )*
    };
}

impl_protobuf_convert_scalar! { bool, u32, u64, i32, i64, f32, f64 }

macro_rules! impl_protobuf_convert_fixed_byte_array {
    ( $( $arr_len:expr ),* ) => {
        $(
            /// Special case for fixed sized arrays.
            impl ProtobufConvert for [u8; $arr_len] {
                type ProtoStruct = Vec<u8>;

                fn to_pb(&self) -> Self::ProtoStruct {
                    self.to_vec()
                }

                fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
                    ensure!(
                        pb.len() == $arr_len,
                        "wrong array size: actual {}, expected {}",
                        pb.len(),
                        $arr_len
                    );

                    Ok({
                        let mut array = [0; $arr_len];
                        array.copy_from_slice(&pb);
                        array
                    })
                }
            }
        )*
    };
}

// We implement array conversion only for most common array sizes that uses
// for example in cryptography.
impl_protobuf_convert_fixed_byte_array! {
    8, 16, 24, 32, 40, 48, 56, 64,
    72, 80, 88, 96, 104, 112, 120, 128,
    160, 256, 512, 1024, 2048
}

/// Marker type for use with `#[serde(with)]`, which provides Protobuf-compatible base64 encoding
/// and decoding. For now, works only on `Vec<u8>` fields.
///
/// The encoder uses the standard base64 alphabet (i.e., `0..9A..Za..z+/`) with no padding.
/// The decoder accepts any of the 4 possible combinations: the standard or the URL-safe alphabet
/// with or without padding.
///
/// If the (de)serializer is not human-readable (e.g., CBOR or `bincode`), the bytes will be
/// (de)serialized without base64 transform, directly as a byte slice.
///
/// # Examples
///
/// ```
/// use exonum_proto::ProtobufBase64;
/// # use serde_derive::*;
/// # use serde_json::json;
///
/// #[derive(Serialize, Deserialize)]
/// struct Test {
///     /// Corresponds to a `bytes buffer = ...` field in Protobuf.
///     #[serde(with = "ProtobufBase64")]
///     buffer: Vec<u8>,
///     // other fields...
/// }
///
/// # fn main() -> anyhow::Result<()> {
/// let test = Test {
///     buffer: b"Hello!".to_vec(),
///     // ...
/// };
/// let obj = serde_json::to_value(&test)?;
/// assert_eq!(obj, json!({ "buffer": "SGVsbG8h", /* ... */ }));
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct ProtobufBase64(());

impl ProtobufBase64 {
    /// Serializes the provided `bytes` with the `serializer`.
    pub fn serialize<S, T>(bytes: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: AsRef<[u8]> + ?Sized,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&base64::encode_config(bytes, base64::STANDARD_NO_PAD))
        } else {
            serializer.serialize_bytes(bytes.as_ref())
        }
    }

    /// Decodes bytes from any of four base64 variations supported as per Protobuf spec
    /// (standard or URL-safe alphabet, with or without padding).
    pub fn decode(value: &str) -> Result<Vec<u8>, base64::DecodeError> {
        // Remove padding if any.
        let value_without_padding = if value.ends_with("==") {
            &value[..value.len() - 2]
        } else if value.ends_with('=') {
            &value[..value.len() - 1]
        } else {
            value
        };

        let is_url_safe = value_without_padding.contains(|ch: char| ch == '-' || ch == '_');
        let config = if is_url_safe {
            base64::URL_SAFE_NO_PAD
        } else {
            base64::STANDARD_NO_PAD
        };
        base64::decode_config(value_without_padding, config)
    }

    /// Deserializes `Vec<u8>` using the provided serializer.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error as DeError;

        struct Base64Visitor;

        impl<'de> Visitor<'de> for Base64Visitor {
            type Value = Vec<u8>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("base64-encoded byte array")
            }

            fn visit_str<E: DeError>(self, value: &str) -> Result<Self::Value, E> {
                ProtobufBase64::decode(value).map_err(E::custom)
            }

            // Needed to guard against non-obvious serialization of flattened fields in `serde`.
            fn visit_bytes<E: DeError>(self, value: &[u8]) -> Result<Self::Value, E> {
                Ok(value.to_vec())
            }
        }

        struct BytesVisitor;

        impl<'de> Visitor<'de> for BytesVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("byte array")
            }

            fn visit_bytes<E: DeError>(self, value: &[u8]) -> Result<Self::Value, E> {
                Ok(value.to_vec())
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(Base64Visitor)
        } else {
            deserializer.deserialize_bytes(BytesVisitor)
        }
    }
}
