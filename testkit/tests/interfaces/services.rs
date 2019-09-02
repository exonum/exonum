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

pub use crate::interface::TxIssue;

use exonum::runtime::{
    rust::{Service, TransactionContext},
    CallInfo, ExecutionError, InstanceId,
};
use exonum_derive::{exonum_service, ProtobufConvert, ServiceFactory};
use serde_derive::{Deserialize, Serialize};

use crate::{
    error::Error,
    interface::{IssueReceiver, IssueReceiverClient},
    proto,
    schema::{Wallet, WalletSchema},
};

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::CreateWallet")]
pub struct TxCreateWallet {
    pub name: String,
}

#[exonum_service]
pub trait WalletInterface {
    fn create(
        &self,
        context: TransactionContext,
        arg: TxCreateWallet,
    ) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "wallet-service",
    proto_sources = "proto",
    interfaces(default = "WalletInterface", additional("IssueReceiver"))
)]
pub struct WalletService;

impl WalletService {
    pub const ID: InstanceId = 24;
}

impl Service for WalletService {}

impl WalletInterface for WalletService {
    fn create(
        &self,
        context: TransactionContext,
        arg: TxCreateWallet,
    ) -> Result<(), ExecutionError> {
        let (_, owner) = context
            .caller()
            .as_transaction()
            .ok_or(Error::WrongInterfaceCaller)?;

        let mut wallets = WalletSchema::new(context.fork()).wallets();
        if wallets.contains(owner) {
            return Err(Error::WalletAlreadyExists.into());
        }
        wallets.put(
            owner,
            Wallet {
                name: arg.name,
                balance: 0,
            },
        );
        Ok(())
    }
}

impl IssueReceiver for WalletService {
    fn issue(&self, context: TransactionContext, arg: TxIssue) -> Result<(), ExecutionError> {
        let instance_id = context
            .caller()
            .as_service()
            .ok_or(Error::WrongInterfaceCaller)?;
        if instance_id != DepositService::ID {
            return Err(Error::UnauthorizedIssuer.into());
        }

        let mut wallets = WalletSchema::new(context.fork()).wallets();
        let mut wallet = wallets.get(&arg.to).ok_or(Error::WalletNotFound)?;
        wallet.balance += arg.amount;
        wallets.put(&arg.to, wallet);
        Ok(())
    }
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "deposit-service",
    proto_sources = "proto",
    interfaces(default = "IssueReceiver")
)]
pub struct DepositService;

impl DepositService {
    pub const ID: InstanceId = 25;
}

impl Service for DepositService {}

impl IssueReceiver for DepositService {
    fn issue(&self, context: TransactionContext, arg: TxIssue) -> Result<(), ExecutionError> {
        context
            .interface::<IssueReceiverClient>(WalletService::ID)
            .issue(arg)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::AnyCall")]
pub struct TxAnyCall {
    pub call_info: CallInfo,
    pub args: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::RecursiveCall")]
pub struct TxRecursiveCall {
    pub depth: u64,
}

#[exonum_service]
pub trait AnyCall {
    fn call_any(&self, context: TransactionContext, arg: TxAnyCall) -> Result<(), ExecutionError>;

    fn call_recursive(
        &self,
        context: TransactionContext,
        arg: TxRecursiveCall,
    ) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "any-call-service",
    proto_sources = "proto",
    interfaces(default = "AnyCall")
)]
pub struct AnyCallService;

impl AnyCallService {
    pub const ID: InstanceId = 26;
}

impl AnyCall for AnyCallService {
    fn call_any(&self, context: TransactionContext, tx: TxAnyCall) -> Result<(), ExecutionError> {
        context.call_context(tx.call_info.instance_id).call(
            tx.call_info.interface_name,
            tx.call_info.method_id,
            tx.args,
        )
    }

    fn call_recursive(
        &self,
        context: TransactionContext,
        arg: TxRecursiveCall,
    ) -> Result<(), ExecutionError> {
        if arg.depth == 1 {
            return Ok(())
        }

        context.call_context(context.instance.id).call(
            "",
            1,
            TxRecursiveCall {
                depth: arg.depth - 1,
            },
        )
    }
}

impl Service for AnyCallService {}
