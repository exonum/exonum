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

//! Module that contains Protobuf messages used by Exonum.

use failure::Error;

pub use self::schema::{
    blockchain::{Block, TxLocation},
    consensus::{
        BlockRequest, BlockResponse, Connect, ExonumMessage, PeersRequest, Precommit, Prevote,
        PrevotesRequest, Propose, ProposeRequest, SignedMessage, Status, TransactionsRequest,
        TransactionsResponse,
    },
    runtime::{AnyTx, CallInfo, ConfiguredInstanceSpec, GenesisConfig},
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
            "{} is out of range for valid ValidatorId",
            pb
        );
        Ok(ValidatorId(pb as u16))
    }
}
