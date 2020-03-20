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

//! Module that contains Protobuf messages used by Exonum.

use anyhow::{ensure, Error};
use exonum_proto::ProtobufConvert;

use std::convert::TryFrom;

use crate::helpers::{Height, Round, ValidatorId};

pub mod schema;

impl ProtobufConvert for Height {
    type ProtoStruct = u64;

    fn to_pb(&self) -> Self::ProtoStruct {
        self.0
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(Self(pb))
    }
}

impl ProtobufConvert for Round {
    type ProtoStruct = u32;

    fn to_pb(&self) -> Self::ProtoStruct {
        self.0
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        Ok(Self(pb))
    }
}

impl ProtobufConvert for ValidatorId {
    type ProtoStruct = u32;

    fn to_pb(&self) -> Self::ProtoStruct {
        u32::from(self.0)
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        ensure!(
            u16::try_from(pb).is_ok(),
            "{} is out of range for valid ValidatorId",
            pb
        );
        Ok(Self(pb as u16))
    }
}
