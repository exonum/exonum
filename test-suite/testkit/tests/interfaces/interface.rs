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

//! Definition of the interfaces for tests of interservice calls.

use exonum::crypto::PublicKey;
use exonum_derive::{exonum_interface, BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use serde_derive::{Deserialize, Serialize};

use crate::proto;

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Issue")]
pub struct Issue {
    pub to: PublicKey,
    pub amount: u64,
}

#[exonum_interface(interface = "IssueReceiver")]
pub trait IssueReceiver<Ctx> {
    type Output;
    fn issue(&self, ctx: Ctx, arg: Issue) -> Self::Output;
}
