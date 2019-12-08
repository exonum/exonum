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
    runtime::{CallInfo, DispatcherError, ErrorMatch, ExecutionError},
};
use exonum_testkit::{TestKit, TestKitBuilder};

use crate::{
    error::Error,
    services::{
        AnyCall, AnyCallService, DepositInterface, DepositService, TxAnyCall, TxIssue,
        WalletInterface, WalletService,
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
        .with_default_rust_service(WalletService)
        .with_default_rust_service(DepositService)
        .with_default_rust_service(AnyCallService)
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
        keypair.create_wallet(WalletService::ID, "Alice".into()),
    )
    .expect("Unable to create wallet");
}

#[test]
fn test_deposit_ok() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        keypair.create_wallet(WalletService::ID, "Alice".into()),
    )
    .expect("Unable to create wallet");

    execute_transaction(
        &mut testkit,
        keypair.deposit(
            DepositService::ID,
            TxIssue {
                to: keypair.0,
                amount: 10_000,
            },
        ),
    )
    .expect("Unable to deposit wallet");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletService::get_schema(&snapshot)
            .wallets
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
        keypair.deposit(
            DepositService::ID,
            TxIssue {
                to: keypair.0,
                amount: 10_000,
            },
        ),
    )
    .unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&Error::WalletNotFound).for_service(WalletService::ID)
    );
}

#[test]
fn test_any_call_ok_deposit() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        keypair.create_wallet(WalletService::ID, "Alice".into()),
    )
    .expect("Unable to create wallet");

    let call = TxAnyCall {
        call_info: CallInfo {
            instance_id: DepositService::ID,
            method_id: 0,
        },
        interface_name: String::default(),
        args: TxIssue {
            to: keypair.0,
            amount: 10_000,
        }
        .into_bytes(),
    };
    execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call))
        .expect("Unable to deposit wallet");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletService::get_schema(&snapshot)
            .wallets
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
        keypair.create_wallet(WalletService::ID, "Alice".into()),
    )
    .expect("Unable to create wallet");

    let call = TxAnyCall {
        call_info: CallInfo {
            instance_id: WalletService::ID,
            method_id: 0,
        },
        interface_name: "IssueReceiver".to_owned(),
        args: TxIssue {
            to: keypair.0,
            amount: 10_000,
        }
        .into_bytes(),
    };
    let err =
        execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call)).unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&Error::UnauthorizedIssuer).for_service(WalletService::ID)
    );
}

#[test]
fn test_any_call_err_unknown_instance() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let call = TxAnyCall {
        call_info: CallInfo::new(10_000, 0),
        interface_name: String::new(),
        args: vec![],
    };
    let err =
        execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call)).unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::IncorrectInstanceId)
    );
}

#[test]
fn test_any_call_err_unknown_interface() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let call = TxAnyCall {
        call_info: CallInfo {
            instance_id: WalletService::ID,
            method_id: 0,
        },
        interface_name: "FooFace".to_owned(),
        args: vec![],
    };
    let err =
        execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call)).unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::NoSuchInterface)
    );
}

#[test]
fn test_any_call_err_unknown_method() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let call = TxAnyCall {
        call_info: CallInfo {
            instance_id: WalletService::ID,
            method_id: 1,
        },
        interface_name: "IssueReceiver".to_owned(),
        args: TxIssue {
            to: keypair.0,
            amount: 10_000,
        }
        .into_bytes(),
    };
    let err =
        execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call)).unwrap_err();

    assert_eq!(err, ErrorMatch::from_fail(&DispatcherError::NoSuchMethod));
}

#[test]
fn test_any_call_err_wrong_arg() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let call = TxAnyCall {
        call_info: CallInfo {
            instance_id: WalletService::ID,
            method_id: 0,
        },
        interface_name: String::default(),
        args: TxAnyCall {
            interface_name: String::default(),
            call_info: CallInfo::new(10_000, 0),
            args: vec![],
        }
        .into_bytes(),
    };
    let err =
        execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call)).unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::MalformedArguments)
            .with_description_containing("invalid utf-8 sequence")
    );
}

#[test]
fn test_any_call_panic_recursion_limit() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        keypair.call_recursive(AnyCallService::ID, 256),
    )
    .expect("Call stack depth is enough");

    let err = execute_transaction(
        &mut testkit,
        keypair.call_recursive(AnyCallService::ID, 300),
    )
    .unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::StackOverflow)
            .with_description_containing("Maximum depth of call stack (256)")
    );
}
