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

#![allow(bare_trait_objects)]
#![allow(renamed_and_removed_lints)]

//! Protobuf generated structs
//!
mod blockchain;
mod helpers;
mod protocol;

pub use self::blockchain::{Block, ConfigReference, TxLocation};
pub use self::helpers::{BitVec, Hash, PublicKey};
pub use self::protocol::{
    BlockRequest, BlockResponse, Connect, PeersRequest, Precommit, Prevote, PrevotesRequest,
    Propose, ProposeRequest, Status, TransactionsRequest, TransactionsResponse,
};

use chrono::{DateTime, TimeZone, Utc};
use crypto;
use encoding::Error;
use helpers::{Height, Round, ValidatorId};
use messages::BinaryForm;
use protobuf::{well_known_types, Message, RepeatedField};

/// Used for establishing correspondence between rust struct
/// and protobuf rust struct
pub trait ToProtobuf: Sized {
    /// Type of the protobuf clone of Self
    type ProtoStruct: Message;

    /// Struct -> ProtoStruct
    fn to_pb(&self) -> Self::ProtoStruct;

    /// ProtoStruct -> Struct
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()>;
}

impl<T> BinaryForm for T
where
    T: ToProtobuf,
{
    fn encode(&self) -> Result<Vec<u8>, Error> {
        Ok(self.to_pb().write_to_bytes().unwrap())
    }

    fn decode(buffer: &[u8]) -> Result<Self, Error> {
        let mut pb = <Self as ToProtobuf>::ProtoStruct::new();
        pb.merge_from_bytes(buffer).unwrap();
        Ok(Self::from_pb(pb).unwrap())
    }
}

/// Used for establishing correspondence between rust struct field
/// and protobuf value
pub trait ProtobufValue: Sized {
    /// Type of value that is returned with pb.take_field() method
    type ProtoValue;
    /// RustField -> ProtobufRustField
    fn to_pb_field(&self) -> Self::ProtoValue;
    /// ProtobufRustField -> RustField
    fn from_pb_field(pb: Self::ProtoValue) -> Result<Self, ()>;
}

impl<T> ProtobufValue for T
where
    T: ToProtobuf,
{
    type ProtoValue = <Self as ToProtobuf>::ProtoStruct;

    fn to_pb_field(&self) -> Self::ProtoValue {
        self.to_pb()
    }

    fn from_pb_field(pb: Self::ProtoValue) -> Result<Self, ()> {
        Self::from_pb(pb)
    }
}

impl ToProtobuf for crypto::Hash {
    type ProtoStruct = Hash;

    fn to_pb(&self) -> Hash {
        let mut hash = Hash::new();
        hash.set_data(self.as_ref().to_vec());
        hash
    }

    fn from_pb(pb: Hash) -> Result<Self, ()> {
        let data = pb.get_data();
        if data.len() == crypto::PUBLIC_KEY_LENGTH {
            Ok(crypto::Hash::from_slice(data).unwrap())
        } else {
            Err(())
        }
    }
}

impl ToProtobuf for crypto::PublicKey {
    type ProtoStruct = PublicKey;

    fn to_pb(&self) -> PublicKey {
        let mut key = PublicKey::new();
        key.set_data(self.as_ref().to_vec());
        key
    }

    fn from_pb(pb: PublicKey) -> Result<Self, ()> {
        let data = pb.get_data();
        if data.len() == crypto::PUBLIC_KEY_LENGTH {
            Ok(crypto::PublicKey::from_slice(data).unwrap())
        } else {
            Err(())
        }
    }
}

impl ToProtobuf for bit_vec::BitVec {
    type ProtoStruct = BitVec;

    fn to_pb(&self) -> BitVec {
        let mut bit_vec = BitVec::new();
        bit_vec.set_data(self.to_bytes());
        bit_vec.set_len(self.len() as u64);
        bit_vec
    }

    fn from_pb(pb: BitVec) -> Result<Self, ()> {
        let data = pb.get_data();
        let mut bit_vec = bit_vec::BitVec::from_bytes(data);
        bit_vec.truncate(pb.get_len() as usize);
        Ok(bit_vec)
    }
}

impl ToProtobuf for DateTime<Utc> {
    type ProtoStruct = well_known_types::Timestamp;

    fn to_pb(&self) -> well_known_types::Timestamp {
        let mut ts = well_known_types::Timestamp::new();
        ts.set_seconds(self.timestamp());
        ts.set_nanos(self.timestamp_subsec_nanos() as i32);
        ts
    }

    fn from_pb(pb: well_known_types::Timestamp) -> Result<Self, ()> {
        Ok(Utc.timestamp(pb.get_seconds(), pb.get_nanos() as u32))
    }
}

impl ProtobufValue for String {
    type ProtoValue = Self;
    fn to_pb_field(&self) -> Self::ProtoValue {
        self.clone()
    }
    fn from_pb_field(pb: Self::ProtoValue) -> Result<Self, ()> {
        Ok(pb)
    }
}

impl ProtobufValue for Height {
    type ProtoValue = u64;
    fn to_pb_field(&self) -> Self::ProtoValue {
        self.0
    }
    fn from_pb_field(pb: Self::ProtoValue) -> Result<Self, ()> {
        Ok(Height(pb))
    }
}

impl ProtobufValue for Round {
    type ProtoValue = u32;
    fn to_pb_field(&self) -> Self::ProtoValue {
        self.0
    }
    fn from_pb_field(pb: Self::ProtoValue) -> Result<Self, ()> {
        Ok(Round(pb))
    }
}

impl ProtobufValue for ValidatorId {
    type ProtoValue = u32;
    fn to_pb_field(&self) -> Self::ProtoValue {
        u32::from(self.0)
    }
    fn from_pb_field(pb: Self::ProtoValue) -> Result<Self, ()> {
        Ok(ValidatorId(pb as u16))
    }
}

impl ProtobufValue for u32 {
    type ProtoValue = u32;
    fn to_pb_field(&self) -> Self::ProtoValue {
        *self
    }
    fn from_pb_field(pb: Self::ProtoValue) -> Result<Self, ()> {
        Ok(pb)
    }
}

impl ProtobufValue for u64 {
    type ProtoValue = u64;
    fn to_pb_field(&self) -> Self::ProtoValue {
        *self
    }
    fn from_pb_field(pb: Self::ProtoValue) -> Result<Self, ()> {
        Ok(pb)
    }
}

impl ProtobufValue for Vec<u8> {
    type ProtoValue = Vec<u8>;
    fn to_pb_field(&self) -> Self::ProtoValue {
        self.clone()
    }
    fn from_pb_field(pb: Self::ProtoValue) -> Result<Self, ()> {
        Ok(pb)
    }
}

impl<T> ProtobufValue for Vec<T>
where
    T: ProtobufValue,
{
    type ProtoValue = RepeatedField<T::ProtoValue>;
    fn to_pb_field(&self) -> Self::ProtoValue {
        RepeatedField::from_vec(self.into_iter().map(|v| v.to_pb_field()).collect())
    }
    fn from_pb_field(pb: Self::ProtoValue) -> Result<Self, ()> {
        let vec = pb
            .into_iter()
            .map(|v| ProtobufValue::from_pb_field(v).unwrap())
            .collect();
        Ok(vec)
    }
}
