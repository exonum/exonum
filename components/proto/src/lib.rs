// Copyright 2019 The Exonum Team
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
//!
//TODO: revert the example

#[macro_use]
extern crate serde_derive; // Required for Protobuf.

pub use protobuf_convert::*;

pub mod proto;

use chrono::{DateTime, TimeZone, Utc};
use failure::{ensure, format_err, Error};
use protobuf::well_known_types;

use std::collections::HashMap;

use proto::common::BitVec;

#[macro_use]
mod macros;
#[cfg(test)]
mod tests;

/// Used for establishing correspondence between rust struct
/// and protobuf rust struct
pub trait ProtobufConvert: Sized {
    /// Type of the protobuf clone of Self
    type ProtoStruct;

    /// Struct -> ProtoStruct
    fn to_pb(&self) -> Self::ProtoStruct;

    /// ProtoStruct -> Struct
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error>;
}

impl ProtobufConvert for DateTime<Utc> {
    type ProtoStruct = well_known_types::Timestamp;

    fn to_pb(&self) -> well_known_types::Timestamp {
        let mut ts = well_known_types::Timestamp::new();
        ts.set_seconds(self.timestamp());
        ts.set_nanos(self.timestamp_subsec_nanos() as i32);
        ts
    }

    fn from_pb(pb: well_known_types::Timestamp) -> Result<Self, Error> {
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
        ensure!(
            pb <= u32::from(u16::max_value()),
            "u32 is out of range for valid u16"
        );
        Ok(pb as u16)
    }
}

impl ProtobufConvert for i16 {
    type ProtoStruct = i32;

    fn to_pb(&self) -> Self::ProtoStruct {
        i32::from(*self)
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        ensure!(
            pb >= i32::from(i16::min_value()) && pb <= i32::from(i16::max_value()),
            "i32 is out of range for valid i16"
        );
        Ok(pb as i16)
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
    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
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
