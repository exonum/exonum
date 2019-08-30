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

//! Interfaces definition for the tests of interservice calls.

use exonum::{
    runtime::{ExecutionError, rust::{TransactionContext, service::CallContext}},
    crypto::PublicKey,
    merkledb::BinaryValue,
};
use exonum_derive::{ProtobufConvert, exonum_service};
use serde_derive::{Serialize, Deserialize};

use crate::proto;

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Issue")]
pub struct TxIssue {
    pub to: PublicKey,
    pub amount: u64,
}

#[exonum_service]
pub trait IssueReceiver {
    fn issue(&self, context: TransactionContext, arg: TxIssue) -> Result<(), ExecutionError>;
}

pub struct IssueReceiverClient<'a> {
    interface_name: String,
    context: CallContext<'a>,
}

impl<'a> IssueReceiverClient<'a> {
    pub fn issue(&self, arg: TxIssue) -> Result<(), ExecutionError> {
        self.context
            .call(self.interface_name.clone(), 0, arg.into_bytes().as_ref())
    }
}

impl<'a> From<CallContext<'a>> for IssueReceiverClient<'a> {
    fn from(context: CallContext<'a>) -> Self {
        Self {
            context,
            interface_name: "IssueReceiver".to_owned(),
        }
    }
}
