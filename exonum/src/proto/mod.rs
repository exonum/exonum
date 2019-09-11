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
//! # Examples
//! ```
//! extern crate exonum;
//! #[macro_use] extern crate exonum_derive;
//!
//! use exonum::crypto::{PublicKey, Hash};
//!
//! // See doc_tests.proto for protobuf definitions of this structs.
//!
//! #[derive(ProtobufConvert)]
//! #[exonum(pb = "exonum::proto::schema::doc_tests::MyStructSmall")]
//! struct MyStructSmall {
//!     key: PublicKey,
//!     num_field: u32,
//!     string_field: String,
//! }
//!
//! #[derive(ProtobufConvert)]
//! #[exonum(pb = "exonum::proto::schema::doc_tests::MyStructBig")]
//! struct MyStructBig {
//!     hash: Hash,
//!     my_struct_small: MyStructSmall,
//! }
//! ```

pub use self::schema::{
    blockchain::{Block, ConfigReference, TxLocation},
    consensus::{
        BlockRequest, BlockResponse, Connect, ExonumMessage, PeersRequest, Precommit, Prevote,
        PrevotesRequest, Propose, ProposeRequest, SignedMessage, Status, TransactionsRequest,
        TransactionsResponse,
    },
    helpers::{BitVec, Hash, PublicKey, Signature},
    runtime::{AnyTx, CallInfo},
};

use std::{borrow::Cow, convert::TryFrom};

pub mod schema;

#[macro_use]
mod macros;
#[cfg(test)]
mod tests;

use chrono::{DateTime, TimeZone, Utc};
use exonum_merkledb::BinaryValue;
use failure::Error;
use protobuf::{well_known_types, Message};

use std::collections::HashMap;

use crate::{
    crypto::{self, kx},
    helpers::{Height, Round, ValidatorId},
};

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

impl ProtobufConvert for crypto::Hash {
    type ProtoStruct = Hash;

    fn to_pb(&self) -> Hash {
        let mut hash = Hash::new();
        hash.set_data(self.as_ref().to_vec());
        hash
    }

    fn from_pb(pb: Hash) -> Result<Self, Error> {
        let data = pb.get_data();
        ensure!(data.len() == crypto::HASH_SIZE, "Wrong Hash size");
        crypto::Hash::from_slice(data).ok_or_else(|| format_err!("Cannot convert Hash from bytes"))
    }
}

impl ProtobufConvert for crypto::PublicKey {
    type ProtoStruct = PublicKey;

    fn to_pb(&self) -> PublicKey {
        let mut key = PublicKey::new();
        key.set_data(self.as_ref().to_vec());
        key
    }

    fn from_pb(pb: PublicKey) -> Result<Self, Error> {
        let data = pb.get_data();
        ensure!(
            data.len() == crypto::PUBLIC_KEY_LENGTH,
            "Wrong PublicKey size"
        );
        crypto::PublicKey::from_slice(data)
            .ok_or_else(|| format_err!("Cannot convert PublicKey from bytes"))
    }
}

impl ProtobufConvert for kx::PublicKey {
    type ProtoStruct = PublicKey;

    fn to_pb(&self) -> PublicKey {
        let mut key = PublicKey::new();
        key.set_data(self.as_ref().to_vec());
        key
    }

    fn from_pb(pb: PublicKey) -> Result<Self, Error> {
        let data = pb.get_data();
        ensure!(
            data.len() == crypto::PUBLIC_KEY_LENGTH,
            "Wrong PublicKey size"
        );
        kx::PublicKey::from_slice(data)
            .ok_or_else(|| format_err!("Cannot convert PublicKey from bytes"))
    }
}

impl ProtobufConvert for crypto::Signature {
    type ProtoStruct = Signature;

    fn to_pb(&self) -> Signature {
        let mut sign = Signature::new();
        sign.set_data(self.as_ref().to_vec());
        sign
    }

    fn from_pb(pb: Signature) -> Result<Self, Error> {
        let data = pb.get_data();
        ensure!(
            data.len() == crypto::SIGNATURE_LENGTH,
            "Wrong Signature size"
        );
        crypto::Signature::from_slice(data)
            .ok_or_else(|| format_err!("Cannot convert Signature from bytes"))
    }
}

impl ProtobufConvert for bit_vec::BitVec {
    type ProtoStruct = BitVec;

    fn to_pb(&self) -> BitVec {
        let mut bit_vec = BitVec::new();
        bit_vec.set_data(self.to_bytes());
        bit_vec.set_len(self.len() as u64);
        bit_vec
    }

    fn from_pb(pb: BitVec) -> Result<Self, Error> {
        let data = pb.get_data();
        let mut bit_vec = bit_vec::BitVec::from_bytes(data);
        bit_vec.truncate(pb.get_len() as usize);
        Ok(bit_vec)
    }
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

impl ProtobufConvert for Height {
    type ProtoStruct = u64;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.0
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(Height(pb))
    }
}

impl ProtobufConvert for Round {
    type ProtoStruct = u32;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.0
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(Round(pb))
    }
}

impl ProtobufConvert for ValidatorId {
    type ProtoStruct = u32;
    fn to_pb(&self) -> Self::ProtoStruct {
        u32::from(self.0)
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        ensure!(
            pb <= u32::from(u16::max_value()),
            "u32 is our of range for valid ValidatorId"
        );
        Ok(ValidatorId(pb as u16))
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
            "i32 is our of range for valid i16"
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
    K: Eq + std::hash::Hash + Clone,
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

macro_rules! impl_protobuf_convert_scalar {
    ($name:tt) => {
        impl ProtobufConvert for $name {
            type ProtoStruct = $name;
            fn to_pb(&self) -> Self::ProtoStruct {
                *self
            }
            fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
                Ok(pb)
            }
        }
    };
}

impl_protobuf_convert_scalar!(bool);
impl_protobuf_convert_scalar!(u32);
impl_protobuf_convert_scalar!(u64);
impl_protobuf_convert_scalar!(i32);
impl_protobuf_convert_scalar!(i64);
impl_protobuf_convert_scalar!(f32);
impl_protobuf_convert_scalar!(f64);

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

// TODO Implement proper serialize deserialize [ECR-3222]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Any(well_known_types::Any);

impl Any {
    const TYPE_URL: &'static str = "type.googleapis.com/";

    pub fn new<T>(value: T) -> Self
    where
        T: ProtobufConvert,
        <T as ProtobufConvert>::ProtoStruct: Message,
    {
        Self::from_pb_message(value.to_pb())
    }

    /// Returns true if this instance does not contain any type of data.
    pub fn is_null(&self) -> bool {
        self.0.type_url.is_empty() && self.0.value.is_empty()
    }

    pub fn try_into<T>(self) -> Result<T, failure::Error>
    where
        T: BinaryValue + ProtobufConvert,
        <T as ProtobufConvert>::ProtoStruct: Message,
    {
        let type_url = [
            Self::TYPE_URL,
            protobuf::reflect::MessageDescriptor::for_type::<<T as ProtobufConvert>::ProtoStruct>()
                .full_name(),
        ]
        .concat();
        ensure!(
            self.0.type_url == type_url,
            "Type url mismatch, actual {}, expected {}",
            self.0.type_url,
            type_url
        );
        T::from_bytes(self.0.value.into())
    }

    fn from_pb_message(pb: impl Message) -> Self {
        // See protobuf documentation for clarification.
        // https://developers.google.com/protocol-buffers/docs/proto3#any
        let type_url = [Self::TYPE_URL, pb.descriptor().full_name()].concat();
        let value = pb
            .write_to_bytes()
            .expect("Failed to serialize in BinaryValue for `Any`");

        Self(well_known_types::Any {
            type_url,
            value,
            ..Default::default()
        })
    }
}

impl ProtobufConvert for Any {
    type ProtoStruct = well_known_types::Any;

    fn to_pb(&self) -> Self::ProtoStruct {
        self.0.clone()
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(Self(pb))
    }
}

impl BinaryValue for Any {
    fn to_bytes(&self) -> Vec<u8> {
        self.0
            .write_to_bytes()
            .expect("Failed to serialize in BinaryValue for `Any`")
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut inner = <Self as ProtobufConvert>::ProtoStruct::new();
        inner.merge_from_bytes(bytes.as_ref())?;
        ProtobufConvert::from_pb(inner)
    }
}

impl From<well_known_types::Any> for Any {
    fn from(v: well_known_types::Any) -> Self {
        Self(v)
    }
}

// TODO implement conversions for the other well-known types [ECR-3222]

impl From<()> for Any {
    fn from(_: ()) -> Self {
        let v = well_known_types::Empty::new();
        Self::from_pb_message(v)
    }
}

impl From<String> for Any {
    fn from(s: String) -> Self {
        let mut v = well_known_types::StringValue::new();
        v.set_value(s);
        Self::from_pb_message(v)
    }
}

impl From<&str> for Any {
    fn from(s: &str) -> Self {
        let mut v = well_known_types::StringValue::new();
        v.set_value(s.to_owned());
        Self::from_pb_message(v)
    }
}

impl From<Vec<u8>> for Any {
    fn from(s: Vec<u8>) -> Self {
        let mut v = well_known_types::BytesValue::new();
        v.set_value(s);
        Self::from_pb_message(v)
    }
}

impl From<u64> for Any {
    fn from(s: u64) -> Self {
        let mut v = well_known_types::UInt64Value::new();
        v.set_value(s);
        Self::from_pb_message(v)
    }
}

impl TryFrom<Any> for u64 {
    type Error = failure::Error;

    fn try_from(value: Any) -> Result<Self, Self::Error> {
        let mut v = well_known_types::UInt64Value::new();
        v.merge_from_bytes(&value.0.value)?;
        Ok(v.value)
    }
}

// Think about bincode instead of protobuf. [ECR-3222]
#[macro_export]
macro_rules! impl_binary_key_for_binary_value {
    ($type:ident) => {
        impl exonum_merkledb::BinaryKey for $type {
            fn size(&self) -> usize {
                exonum_merkledb::BinaryValue::to_bytes(self).len()
            }

            fn write(&self, buffer: &mut [u8]) -> usize {
                let bytes = exonum_merkledb::BinaryValue::to_bytes(self);
                buffer.copy_from_slice(&bytes);
                bytes.len()
            }

            fn read(buffer: &[u8]) -> Self::Owned {
                // `unwrap` is safe because only this code uses for
                // serialize and deserialize these keys.
                <Self as exonum_merkledb::BinaryValue>::from_bytes(buffer.into()).unwrap()
            }
        }
    };
}
