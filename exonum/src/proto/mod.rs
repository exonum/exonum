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

use failure::Error;

pub use self::schema::{
    blockchain::{Block, TxLocation},
    consensus::{
        BlockRequest, BlockResponse, Connect, ExonumMessage, PeersRequest, Precommit, Prevote,
        PrevotesRequest, Propose, ProposeRequest, SignedMessage, Status, TransactionsRequest,
        TransactionsResponse,
    },
    runtime::{AnyTx, CallInfo},
};
use crate::helpers::{Height, Round, ValidatorId};
use exonum_proto::ProtobufConvert;

pub mod schema;

#[cfg(test)]
mod tests;

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
            "u32 is out of range for valid ValidatorId"
        );
        Ok(ValidatorId(pb as u16))
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
