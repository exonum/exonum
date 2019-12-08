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

//! Services set to test interservice calls.

pub use crate::interface::Issue;

use exonum::{
    crypto::{Hash, PublicKey},
    runtime::{
        rust::{CallContext, DefaultInstance, GenericCallMut, MethodDescriptor, Service},
        BlockchainData, CallInfo, ExecutionError, InstanceId, SnapshotExt,
    },
};
use exonum_derive::*;
use exonum_merkledb::{access::Access, Snapshot};
use exonum_proto::ProtobufConvert;
use serde_derive::{Deserialize, Serialize};

use crate::{
    error::Error,
    interface::IssueReceiver,
    proto,
    schema::{Wallet, WalletSchema},
};

#[exonum_interface]
pub trait WalletInterface<Ctx> {
    fn create_wallet(&self, ctx: Ctx, username: String) -> _;
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

impl Service for WalletService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

impl WalletInterface<CallContext<'_>> for WalletService {
    type Output = Result<(), ExecutionError>;

    fn create_wallet(&self, ctx: CallContext<'_>, username: String) -> Self::Output {
        let owner = ctx.caller().author().ok_or(Error::WrongInterfaceCaller)?;
        let mut schema = WalletSchema::new(ctx.service_data());

        if schema.wallets.contains(&owner) {
            return Err(Error::WalletAlreadyExists.into());
        }
        schema.wallets.put(
            &owner,
            Wallet {
                name: username,
                balance: 0,
            },
        );
        Ok(())
    }
}

impl IssueReceiver<CallContext<'_>> for WalletService {
    type Output = Result<(), ExecutionError>;

    fn issue(&self, ctx: CallContext<'_>, arg: Issue) -> Self::Output {
        let instance_id = ctx
            .caller()
            .as_service()
            .ok_or(Error::WrongInterfaceCaller)?;
        if instance_id != DepositService::ID {
            return Err(Error::UnauthorizedIssuer.into());
        }

        let mut schema = WalletSchema::new(ctx.service_data());
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
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
pub struct TxIssue {
    pub to: PublicKey,
    pub amount: u64,
}

#[exonum_interface]
pub trait DepositInterface<Ctx> {
    fn deposit(&self, context: Ctx, arg: TxIssue) -> _;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "deposit-service", proto_sources = "proto")]
#[service_dispatcher(implements("DepositInterface"))]
pub struct DepositService;

impl DepositService {
    pub const ID: InstanceId = 25;
}

impl Service for DepositService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

impl DepositInterface<CallContext<'_>> for DepositService {
    type Output = Result<(), ExecutionError>;

    fn deposit(&self, mut ctx: CallContext<'_>, arg: TxIssue) -> Self::Output {
        use crate::interface::IssueReceiverMut;

        ctx.issue(
            WalletService::ID,
            Issue {
                to: arg.to,
                amount: arg.amount,
            },
        )
    }
}

impl DefaultInstance for DepositService {
    const INSTANCE_ID: u32 = Self::ID;
    const INSTANCE_NAME: &'static str = "deposit";
}

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::AnyCall")]
pub struct TxAnyCall {
    pub call_info: CallInfo,
    pub interface_name: String,
    pub args: Vec<u8>,
}

#[exonum_interface]
pub trait AnyCall<Ctx> {
    fn call_any(&self, context: Ctx, arg: TxAnyCall) -> _;
    fn call_recursive(&self, context: Ctx, depth: u64) -> _;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "any-call-service", proto_sources = "proto")]
#[service_dispatcher(implements("AnyCall"))]
pub struct AnyCallService;

impl AnyCallService {
    pub const ID: InstanceId = 26;
}

impl AnyCall<CallContext<'_>> for AnyCallService {
    type Output = Result<(), ExecutionError>;

    fn call_any(&self, mut ctx: CallContext<'_>, tx: TxAnyCall) -> Self::Output {
        let method_descriptor =
            MethodDescriptor::new(&tx.interface_name, "", tx.call_info.method_id);
        ctx.generic_call_mut(tx.call_info.instance_id, method_descriptor, tx.args)
    }

    fn call_recursive(
        &self,
        mut context: CallContext<'_>,
        depth: u64,
    ) -> Result<(), ExecutionError> {
        if depth == 1 {
            return Ok(());
        }
        context.call_recursive(context.instance().id, depth - 1)
    }
}

impl Service for AnyCallService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

impl DefaultInstance for AnyCallService {
    const INSTANCE_ID: u32 = Self::ID;
    const INSTANCE_NAME: &'static str = "any-call";
}
