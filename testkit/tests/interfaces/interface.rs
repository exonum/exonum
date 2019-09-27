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

use exonum::{
    crypto::PublicKey,
    runtime::{
        rust::{Interface, TransactionContext},
        CallContext, ExecutionError,
    },
};
use exonum_derive::{exonum_service, ProtobufConvert};
use serde_derive::{Deserialize, Serialize};

use crate::proto;

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Issue")]
pub struct Issue {
    pub to: PublicKey,
    pub amount: u64,
}

#[exonum_service(interface = "IssueReceiver")]
pub trait IssueReceiver {
    fn issue(&self, context: TransactionContext, arg: Issue) -> Result<(), ExecutionError>;
}

pub struct IssueReceiverClient<'a>(CallContext<'a>);

impl<'a> IssueReceiverClient<'a> {
    pub fn issue(&self, arg: Issue) -> Result<(), ExecutionError> {
        self.0.call(IssueReceiver::INTERFACE_NAME, 0, arg)
    }
}

impl<'a> From<CallContext<'a>> for IssueReceiverClient<'a> {
    fn from(context: CallContext<'a>) -> Self {
        Self(context)
    }
}
