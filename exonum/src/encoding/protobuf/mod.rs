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

//! Protobuf generated structs and traits for conversion.

// For rust-protobuf generated files.
#![allow(bare_trait_objects)]
#![allow(renamed_and_removed_lints)]

pub use self::blockchain::{Block, ConfigReference, TxLocation};
pub use self::helpers::{BitVec, Hash, PublicKey};
pub use self::protocol::{
    BlockRequest, BlockResponse, Connect, PeersRequest, Precommit, Prevote, PrevotesRequest,
    Propose, ProposeRequest, Status, TransactionsRequest, TransactionsResponse,
};

pub mod helpers;
#[cfg(test)]
pub mod tests;

use bit_vec;
use chrono::{DateTime, TimeZone, Utc};
use protobuf::{well_known_types, Message};

use crypto;
use encoding::Error;
use helpers::{Height, Round, ValidatorId};
use messages::BinaryForm;

mod blockchain;
mod protocol;

/// Used for establishing correspondence between rust struct
/// and protobuf rust struct
pub trait ProtobufConvert: Sized {
    /// Type of the protobuf clone of Self
    type ProtoStruct;

    /// Struct -> ProtoStruct
    fn to_pb(&self) -> Self::ProtoStruct;

    /// ProtoStruct -> Struct
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()>;
}

impl<T> BinaryForm for T
where
    T: Message,
{
    fn encode(&self) -> Result<Vec<u8>, Error> {
        Ok(self.write_to_bytes().unwrap())
    }

    fn decode(buffer: &[u8]) -> Result<Self, Error> {
        let mut pb = Self::new();
        pb.merge_from_bytes(buffer)
            .map_err(|_| "Conversion from protobuf error")?;
        Ok(pb)
    }
}

impl ProtobufConvert for crypto::Hash {
    type ProtoStruct = Hash;

    fn to_pb(&self) -> Hash {
        let mut hash = Hash::new();
        hash.set_data(self.as_ref().to_vec());
        hash
    }

    fn from_pb(pb: Hash) -> Result<Self, ()> {
        let data = pb.get_data();
        if data.len() == crypto::HASH_SIZE {
            crypto::Hash::from_slice(data).ok_or(())
        } else {
            Err(())
        }
    }
}

impl ProtobufConvert for crypto::PublicKey {
    type ProtoStruct = PublicKey;

    fn to_pb(&self) -> PublicKey {
        let mut key = PublicKey::new();
        key.set_data(self.as_ref().to_vec());
        key
    }

    fn from_pb(pb: PublicKey) -> Result<Self, ()> {
        let data = pb.get_data();
        if data.len() == crypto::PUBLIC_KEY_LENGTH {
            crypto::PublicKey::from_slice(data).ok_or(())
        } else {
            Err(())
        }
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

    fn from_pb(pb: BitVec) -> Result<Self, ()> {
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

    fn from_pb(pb: well_known_types::Timestamp) -> Result<Self, ()> {
        Utc.timestamp_opt(pb.get_seconds(), pb.get_nanos() as u32)
            .single()
            .ok_or(())
    }
}

impl ProtobufConvert for String {
    type ProtoStruct = Self;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.clone()
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
        Ok(pb)
    }
}

impl ProtobufConvert for Height {
    type ProtoStruct = u64;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.0
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
        Ok(Height(pb))
    }
}

impl ProtobufConvert for Round {
    type ProtoStruct = u32;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.0
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
        Ok(Round(pb))
    }
}

impl ProtobufConvert for ValidatorId {
    type ProtoStruct = u32;
    fn to_pb(&self) -> Self::ProtoStruct {
        u32::from(self.0)
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
        if pb <= u32::from(u16::max_value()) {
            Ok(ValidatorId(pb as u16))
        } else {
            Err(())
        }
    }
}

impl ProtobufConvert for u32 {
    type ProtoStruct = u32;
    fn to_pb(&self) -> Self::ProtoStruct {
        *self
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
        Ok(pb)
    }
}

impl ProtobufConvert for u64 {
    type ProtoStruct = u64;
    fn to_pb(&self) -> Self::ProtoStruct {
        *self
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
        Ok(pb)
    }
}

impl<T> ProtobufConvert for Vec<T>
where
    T: ProtobufConvert,
{
    type ProtoStruct = Vec<T::ProtoStruct>;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.into_iter().map(|v| v.to_pb()).collect()
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
        pb.into_iter()
            .map(ProtobufConvert::from_pb)
            .collect::<Result<Vec<_>, _>>()
    }
}

/// Special case for protobuf bytes.
impl ProtobufConvert for Vec<u8> {
    type ProtoStruct = Vec<u8>;
    fn to_pb(&self) -> Self::ProtoStruct {
        self.clone()
    }
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
        Ok(pb)
    }
}
