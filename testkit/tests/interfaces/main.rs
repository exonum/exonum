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

use exonum::{
    crypto,
    merkledb::BinaryValue,
    messages::{AnyTx, Verified},
    proto::Any,
    runtime::{self, dispatcher, rust::Transaction, CallInfo, ExecutionError},
};
use exonum_testkit::{InstanceCollection, TestKit, TestKitBuilder};

use crate::{
    error::Error,
    schema::WalletSchema,
    services::{
        AnyCallService, DepositService, TxAnyCall, TxCreateWallet, TxIssue, TxRecursiveCall,
        WalletService,
    },
};

mod error;
mod interface;
mod proto;
mod schema;
mod services;

fn testkit_with_interfaces() -> TestKit {
    TestKitBuilder::validator()
        .with_logger()
        .with_service(InstanceCollection::new(WalletService).with_instance(
            WalletService::ID,
            "wallet",
            Any::default(),
        ))
        .with_service(InstanceCollection::new(DepositService).with_instance(
            DepositService::ID,
            "deposit",
            Any::default(),
        ))
        .with_service(InstanceCollection::new(AnyCallService).with_instance(
            AnyCallService::ID,
            "any-call",
            Any::default(),
        ))
        .create()
}

fn execute_transaction(testkit: &mut TestKit, tx: Verified<AnyTx>) -> Result<(), ExecutionError> {
    testkit.create_block_with_transaction(tx).transactions[0]
        .status()
        .map_err(Clone::clone)
}

#[test]
fn test_create_wallet_ok() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        TxCreateWallet {
            name: "Alice".into(),
        }
        .sign(WalletService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to create wallet");
}

#[test]
fn test_deposit_ok() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        TxCreateWallet {
            name: "Alice".into(),
        }
        .sign(WalletService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to create wallet");

    execute_transaction(
        &mut testkit,
        TxIssue {
            to: keypair.0,
            amount: 10_000,
        }
        .sign(DepositService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to deposit wallet");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletSchema::new(&snapshot)
            .wallets()
            .get(&keypair.0)
            .unwrap()
            .balance,
        10_000
    );
}

#[test]
fn test_deposit_err_issue_without_wallet() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let err = execute_transaction(
        &mut testkit,
        TxIssue {
            to: keypair.0,
            amount: 10_000,
        }
        .sign(DepositService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(err.kind, Error::WalletNotFound.into());
}

#[test]
fn test_any_call_ok_deposit() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        TxCreateWallet {
            name: "Alice".into(),
        }
        .sign(WalletService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to create wallet");

    execute_transaction(
        &mut testkit,
        TxAnyCall {
            call_info: CallInfo {
                interface_name: "IssueReceiver".into(),
                instance_id: DepositService::ID,
                method_id: 0,
            },
            args: TxIssue {
                to: keypair.0,
                amount: 10_000,
            }
            .into_bytes(),
        }
        .sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to deposit wallet");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletSchema::new(&snapshot)
            .wallets()
            .get(&keypair.0)
            .unwrap()
            .balance,
        10_000
    );
}

#[test]
fn test_any_call_err_deposit_unauthorized() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        TxCreateWallet {
            name: "Alice".into(),
        }
        .sign(WalletService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to create wallet");

    let err = execute_transaction(
        &mut testkit,
        TxAnyCall {
            call_info: CallInfo {
                interface_name: "IssueReceiver".to_owned(),
                instance_id: WalletService::ID,
                method_id: 0,
            },
            args: TxIssue {
                to: keypair.0,
                amount: 10_000,
            }
            .into_bytes(),
        }
        .sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(err.kind, Error::UnauthorizedIssuer.into());
}

#[test]
fn test_any_call_err_unknown_instance() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let err = execute_transaction(
        &mut testkit,
        TxAnyCall {
            call_info: CallInfo::new(10_000, 0),
            args: vec![],
        }
        .sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(err.kind, dispatcher::Error::IncorrectInstanceId.into());
}

#[test]
fn test_any_call_err_unknown_interface() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let err = execute_transaction(
        &mut testkit,
        TxAnyCall {
            call_info: CallInfo {
                interface_name: "FooFace".to_owned(),
                instance_id: WalletService::ID,
                method_id: 0,
            },
            args: vec![],
        }
        .sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(err.kind, dispatcher::Error::NoSuchInterface.into());
}

#[test]
fn test_any_call_err_unknown_method() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let err = execute_transaction(
        &mut testkit,
        TxAnyCall {
            call_info: CallInfo {
                interface_name: "IssueReceiver".to_owned(),
                instance_id: WalletService::ID,
                method_id: 1,
            },
            args: TxIssue {
                to: keypair.0,
                amount: 10_000,
            }
            .into_bytes(),
        }
        .sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(err.kind, dispatcher::Error::NoSuchMethod.into());
}

#[test]
fn test_any_call_err_wrong_arg() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let err = execute_transaction(
        &mut testkit,
        TxAnyCall {
            call_info: CallInfo {
                interface_name: String::default(),
                instance_id: WalletService::ID,
                method_id: 0,
            },
            args: TxAnyCall {
                call_info: CallInfo::new(10_000, 0),
                args: vec![],
            }
            .into_bytes(),
        }
        .sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(err.kind, runtime::rust::Error::ArgumentsParseError.into());
}

#[test]
fn test_any_call_panic_recursion_limit() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        TxRecursiveCall { depth: 256 }.sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .expect("Call stack depth is enough");

    let err = execute_transaction(
        &mut testkit,
        TxRecursiveCall { depth: 300 }.sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert!(err
        .to_string()
        .contains("Maximum depth of call stack has been reached"));
}
