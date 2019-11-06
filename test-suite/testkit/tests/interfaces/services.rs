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
        rust::{CallContext, Service},
        BlockchainData, CallInfo, ExecutionError, InstanceId, SnapshotExt,
    },
};
use exonum_derive::{exonum_service, BinaryValue, ObjectHash, ServiceFactory};
use exonum_merkledb::{access::Access, Snapshot};
use exonum_proto::ProtobufConvert;
use serde_derive::{Deserialize, Serialize};

use crate::{
    error::Error,
    interface::{IssueReceiver, IssueReceiverClient},
    proto,
    schema::{Wallet, WalletSchema},
};

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::CreateWallet")]
pub struct TxCreateWallet {
    pub name: String,
}

#[exonum_service]
pub trait WalletInterface {
    fn create(&self, context: CallContext<'_>, arg: TxCreateWallet) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "wallet-service",
    proto_sources = "proto",
    implements("WalletInterface", "IssueReceiver")
)]
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

#[protobuf_convert(source = "proto::Issue")]
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
pub struct TxIssue {
    pub to: PublicKey,
    pub amount: u64,
}

#[exonum_service]
pub trait DepositInterface {
    fn issue(&self, context: CallContext<'_>, arg: TxIssue) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "deposit-service",
    proto_sources = "proto",
    implements("DepositInterface")
)]
pub struct DepositService;

impl DepositService {
    pub const ID: InstanceId = 25;
}

impl Service for DepositService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

impl DepositInterface for DepositService {
    fn issue(&self, mut context: CallContext<'_>, arg: TxIssue) -> Result<(), ExecutionError> {
        context
            .interface::<IssueReceiverClient<'_>>(WalletService::ID)?
            .issue(Issue {
                to: arg.to,
                amount: arg.amount,
            })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::AnyCall")]
pub struct TxAnyCall {
    pub call_info: CallInfo,
    pub interface_name: String,
    pub args: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::RecursiveCall")]
pub struct TxRecursiveCall {
    pub depth: u64,
}

#[exonum_service]
pub trait AnyCall {
    fn call_any(&self, context: CallContext<'_>, arg: TxAnyCall) -> Result<(), ExecutionError>;

    fn call_recursive(
        &self,
        context: CallContext<'_>,
        arg: TxRecursiveCall,
    ) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "any-call-service",
    proto_sources = "proto",
    implements("AnyCall")
)]
pub struct AnyCallService;

impl AnyCallService {
    pub const ID: InstanceId = 26;
}

impl AnyCall for AnyCallService {
    fn call_any(&self, mut context: CallContext<'_>, tx: TxAnyCall) -> Result<(), ExecutionError> {
        context.call_context(tx.call_info.instance_id)?.call(
            tx.interface_name,
            tx.call_info.method_id,
            tx.args,
        )
    }

    fn call_recursive(
        &self,
        mut context: CallContext<'_>,
        arg: TxRecursiveCall,
    ) -> Result<(), ExecutionError> {
        if arg.depth == 1 {
            return Ok(());
        }

        context.call_context(context.instance().id)?.call(
            "",
            1,
            TxRecursiveCall {
                depth: arg.depth - 1,
            },
        )
    }
}

impl Service for AnyCallService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}
