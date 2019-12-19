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

//! Services set to test calls between services.

pub use crate::interface::Issue;

use exonum::{
    crypto::PublicKey,
    runtime::{
        rust::{CallContext, ChildAuthorization, DefaultInstance, Service},
        AnyTx, CallInfo, ExecutionError, InstanceId, SnapshotExt,
    },
};
use exonum_derive::*;
use exonum_merkledb::{access::Access, BinaryValue, Snapshot};
use exonum_proto::ProtobufConvert;
use serde_derive::{Deserialize, Serialize};

use crate::{
    error::Error,
    interface::{IssueReceiver, IssueReceiverClient},
    proto,
    schema::{Wallet, WalletSchema},
};

#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::CreateWallet")]
pub struct TxCreateWallet {
    pub name: String,
}

#[exonum_interface]
pub trait WalletInterface {
    fn create(&self, context: CallContext<'_>, arg: TxCreateWallet) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("WalletInterface", "IssueReceiver"))]
#[service_factory(artifact_name = "wallet-service", proto_sources = "proto")]
pub struct WalletService;

impl WalletService {
    pub const ID: InstanceId = 24;

    pub fn get_schema<'a>(snapshot: &'a dyn Snapshot) -> WalletSchema<impl Access + 'a> {
        WalletSchema::new(snapshot.for_service(Self::ID).unwrap())
    }
}

impl Service for WalletService {}

impl WalletInterface for WalletService {
    fn create(&self, context: CallContext<'_>, arg: TxCreateWallet) -> Result<(), ExecutionError> {
        let owner = context
            .caller()
            .author()
            .ok_or(Error::WrongInterfaceCaller)?;
        let mut schema = WalletSchema::new(context.service_data());

        if schema.wallets.contains(&owner) {
            return Err(Error::WalletAlreadyExists.into());
        }
        schema.wallets.put(
            &owner,
            Wallet {
                name: arg.name,
                balance: 0,
            },
        );
        Ok(())
    }
}

impl IssueReceiver for WalletService {
    fn issue(&self, context: CallContext<'_>, arg: Issue) -> Result<(), ExecutionError> {
        let instance_id = context
            .caller()
            .as_service()
            .ok_or(Error::WrongInterfaceCaller)?;
        if instance_id != DepositService::ID {
            return Err(Error::UnauthorizedIssuer.into());
        }

        let mut schema = WalletSchema::new(context.service_data());
        let mut wallet = schema.wallets.get(&arg.to).ok_or(Error::WalletNotFound)?;
        wallet.balance += arg.amount;
        schema.wallets.put(&arg.to, wallet);
        Ok(())
    }
}

impl DefaultInstance for WalletService {
    const INSTANCE_ID: u32 = Self::ID;
    const INSTANCE_NAME: &'static str = "wallet";
}

#[protobuf_convert(source = "proto::Issue")]
#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
pub struct TxIssue {
    pub to: PublicKey,
    pub amount: u64,
}

#[exonum_interface]
pub trait DepositInterface {
    fn issue(&self, context: CallContext<'_>, arg: TxIssue) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "deposit-service", proto_sources = "proto")]
#[service_dispatcher(implements("DepositInterface"))]
pub struct DepositService;

impl DepositService {
    pub const ID: InstanceId = 25;
}

impl Service for DepositService {}

impl DepositInterface for DepositService {
    fn issue(&self, mut context: CallContext<'_>, arg: TxIssue) -> Result<(), ExecutionError> {
        // Check authorization of the call.
        if context.caller().author() != Some(arg.to) {
            return Err(Error::UnauthorizedIssuer.into());
        }
        // The child call is authorized by the service.
        context
            .interface::<IssueReceiverClient<'_>>(WalletService::ID)?
            .issue(Issue {
                to: arg.to,
                amount: arg.amount,
            })
    }
}

impl DefaultInstance for DepositService {
    const INSTANCE_ID: u32 = Self::ID;
    const INSTANCE_NAME: &'static str = "deposit";
}

#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::AnyCall")]
pub struct AnyCall {
    pub inner: AnyTx,
    pub interface_name: String,
    pub fallthrough_auth: bool,
}

impl AnyCall {
    pub fn new(call_info: CallInfo, arguments: impl BinaryValue) -> Self {
        Self {
            inner: AnyTx {
                call_info,
                arguments: arguments.into_bytes(),
            },
            fallthrough_auth: false,
            interface_name: String::default(),
        }
    }
}

#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::RecursiveCall")]
pub struct RecursiveCall {
    pub depth: u64,
}

#[exonum_interface]
pub trait CallAny {
    fn call_any(&self, context: CallContext<'_>, arg: AnyCall) -> Result<(), ExecutionError>;

    fn call_recursive(
        &self,
        context: CallContext<'_>,
        arg: RecursiveCall,
    ) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "any-call-service", proto_sources = "proto")]
#[service_dispatcher(implements("CallAny"))]
pub struct AnyCallService;

impl AnyCallService {
    pub const ID: InstanceId = 26;
}

impl CallAny for AnyCallService {
    fn call_any(&self, mut context: CallContext<'_>, tx: AnyCall) -> Result<(), ExecutionError> {
        let auth = if tx.fallthrough_auth {
            ChildAuthorization::Fallthrough
        } else {
            ChildAuthorization::Service
        };
        context
            .call_context(tx.inner.call_info.instance_id, auth)?
            .call(
                tx.interface_name,
                tx.inner.call_info.method_id,
                tx.inner.arguments,
            )
    }

    fn call_recursive(
        &self,
        mut context: CallContext<'_>,
        arg: RecursiveCall,
    ) -> Result<(), ExecutionError> {
        if arg.depth == 1 {
            return Ok(());
        }

        context
            .call_context(context.instance().id, ChildAuthorization::Fallthrough)?
            .call(
                "",
                1,
                RecursiveCall {
                    depth: arg.depth - 1,
                },
            )
    }
}

impl Service for AnyCallService {}

impl DefaultInstance for AnyCallService {
    const INSTANCE_ID: u32 = Self::ID;
    const INSTANCE_NAME: &'static str = "any-call";
}
