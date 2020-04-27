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

//! Services set to test calls between services.

pub use crate::interface::Issue;

use exonum::{
    crypto::PublicKey,
    runtime::{AnyTx, CallInfo, ExecutionContext, ExecutionError, InstanceId, SnapshotExt},
};
use exonum_derive::*;
use exonum_merkledb::{access::Access, BinaryValue, Snapshot};
use exonum_rust_runtime::{
    DefaultInstance, FallthroughAuth, GenericCallMut, MethodDescriptor, Service,
};
use serde_derive::{Deserialize, Serialize};

use crate::{
    error::Error,
    interface::IssueReceiver,
    schema::{Wallet, WalletSchema},
};

#[exonum_interface(auto_ids)]
pub trait WalletInterface<Ctx> {
    type Output;
    fn create_wallet(&self, ctx: Ctx, username: String) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("WalletInterface", "IssueReceiver"))]
#[service_factory(artifact_name = "wallet-service")]
pub struct WalletService;

impl WalletService {
    pub const ID: InstanceId = 24;

    pub fn get_schema<'a>(snapshot: &'a dyn Snapshot) -> WalletSchema<impl Access + 'a> {
        WalletSchema::new(snapshot.for_service(Self::ID).unwrap())
    }
}

impl Service for WalletService {}

impl WalletInterface<ExecutionContext<'_>> for WalletService {
    type Output = Result<(), ExecutionError>;

    fn create_wallet(&self, ctx: ExecutionContext<'_>, username: String) -> Self::Output {
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

impl IssueReceiver<ExecutionContext<'_>> for WalletService {
    type Output = Result<(), ExecutionError>;

    fn issue(&self, ctx: ExecutionContext<'_>, arg: Issue) -> Self::Output {
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

#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
pub struct TxIssue {
    pub to: PublicKey,
    pub amount: u64,
}

#[exonum_interface(auto_ids)]
pub trait DepositInterface<Ctx> {
    type Output;
    fn deposit(&self, context: Ctx, arg: TxIssue) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "deposit-service")]
#[service_dispatcher(implements("DepositInterface"))]
pub struct DepositService;

impl DepositService {
    pub const ID: InstanceId = 25;
}

impl Service for DepositService {}

impl DepositInterface<ExecutionContext<'_>> for DepositService {
    type Output = Result<(), ExecutionError>;

    fn deposit(&self, mut ctx: ExecutionContext<'_>, arg: TxIssue) -> Self::Output {
        use crate::interface::IssueReceiverMut;

        // Check authorization of the call.
        if ctx.caller().author() != Some(arg.to) {
            return Err(Error::UnauthorizedIssuer.into());
        }
        // The child call is authorized by the service.
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

#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
pub struct AnyCall {
    pub inner: AnyTx,
    pub interface_name: String,
    pub fallthrough_auth: bool,
}

impl AnyCall {
    pub fn new(call_info: CallInfo, arguments: impl BinaryValue) -> Self {
        Self {
            inner: AnyTx::new(call_info, arguments.into_bytes()),
            fallthrough_auth: false,
            interface_name: String::default(),
        }
    }
}

#[exonum_interface(auto_ids)]
pub trait CallAny<Ctx> {
    type Output;
    fn call_any(&self, context: Ctx, arg: AnyCall) -> Self::Output;
    fn call_recursive(&self, context: Ctx, depth: u64) -> Self::Output;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "any-call-service")]
#[service_dispatcher(implements("CallAny"))]
pub struct AnyCallService;

impl AnyCallService {
    pub const ID: InstanceId = 26;
}

impl CallAny<ExecutionContext<'_>> for AnyCallService {
    type Output = Result<(), ExecutionError>;

    fn call_any(&self, mut ctx: ExecutionContext<'_>, tx: AnyCall) -> Self::Output {
        let call_info = tx.inner.call_info;
        let args = tx.inner.arguments;
        let method = MethodDescriptor::new(&tx.interface_name, call_info.method_id);

        if tx.fallthrough_auth {
            FallthroughAuth(ctx).generic_call_mut(call_info.instance_id, method, args)
        } else {
            ctx.generic_call_mut(call_info.instance_id, method, args)
        }
    }

    fn call_recursive(
        &self,
        mut context: ExecutionContext<'_>,
        depth: u64,
    ) -> Result<(), ExecutionError> {
        if depth == 1 {
            return Ok(());
        }
        let id = context.instance().id;
        context.call_recursive(id, depth - 1)
    }
}

impl Service for AnyCallService {}

impl DefaultInstance for AnyCallService {
    const INSTANCE_ID: u32 = Self::ID;
    const INSTANCE_NAME: &'static str = "any-call";
}

#[exonum_interface(auto_ids)]
pub trait CustomCallInterface<Ctx> {
    type Output;
    fn custom_call(&self, context: Ctx, arg: Vec<u8>) -> Self::Output;
}

pub type CustomCall = fn(ExecutionContext<'_>) -> Result<(), ExecutionError>;

#[derive(ServiceFactory, ServiceDispatcher, Clone)]
#[service_factory(
    artifact_name = "custom-call",
    service_constructor = "Self::new_instance"
)]
#[service_dispatcher(implements("CustomCallInterface"))]
pub struct CustomCallService {
    handler: CustomCall,
}

impl CustomCallService {
    pub fn new(handler: CustomCall) -> Self {
        Self { handler }
    }

    pub fn new_instance(&self) -> Box<dyn Service> {
        Box::new(self.clone())
    }
}

impl DefaultInstance for CustomCallService {
    const INSTANCE_ID: u32 = 112;
    const INSTANCE_NAME: &'static str = "custom-call";
}

impl CustomCallInterface<ExecutionContext<'_>> for CustomCallService {
    type Output = Result<(), ExecutionError>;

    fn custom_call(&self, context: ExecutionContext<'_>, _arg: Vec<u8>) -> Self::Output {
        (self.handler)(context)
    }
}

impl Service for CustomCallService {}

impl std::fmt::Debug for CustomCallService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomCallService").finish()
    }
}
