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
    messages::{AnyTx, Verified},
    runtime::{rust::Transaction, CallInfo, DispatcherError, ErrorMatch, ExecutionError},
};
use exonum_testkit::{TestKit, TestKitBuilder};

use crate::{
    error::Error,
    services::{
        AnyCall, AnyCallService, DepositService, RecursiveCall, TxCreateWallet, TxIssue,
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
        TxCreateWallet {
            name: "Alice".into(),
        }
        .sign(WalletService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to create wallet");
}

#[test]
fn test_create_wallet_fallthrough_auth() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    // Without fallthrough auth, the call should fail: `create_wallet` expects the caller
    // to be external, and it is a service.
    let mut call = AnyCall::new(
        CallInfo::new(WalletService::ID, 0),
        TxCreateWallet {
            name: "Alice".into(),
        },
    );
    let err = execute_transaction(
        &mut testkit,
        call.clone().sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&Error::WrongInterfaceCaller));

    // With fallthrough auth, the call should succeed.
    call.fallthrough_auth = true;
    execute_transaction(
        &mut testkit,
        call.sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .expect("Cannot create wallet");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletService::get_schema(&snapshot)
            .wallets
            .get(&keypair.0)
            .unwrap()
            .balance,
        0
    );
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
        WalletService::get_schema(&snapshot)
            .wallets
            .get(&keypair.0)
            .unwrap()
            .balance,
        10_000
    );

    // Use indirection via `AnyCallService` to deposit some more funds.
    // Since inner transactions are not checked for uniqueness, depositing the same amount again
    // should work fine.
    let mut call = AnyCall::new(
        CallInfo::new(DepositService::ID, 0),
        TxIssue {
            to: keypair.0,
            amount: 10_000,
        },
    );
    call.fallthrough_auth = true;
    execute_transaction(
        &mut testkit,
        call.clone().sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to deposit more funds");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletService::get_schema(&snapshot)
            .wallets
            .get(&keypair.0)
            .unwrap()
            .balance,
        20_000
    );

    // Add some more indirection layers.
    let mut call = call;
    for _ in 0..10 {
        call = AnyCall::new(CallInfo::new(AnyCallService::ID, 0), call);
        call.fallthrough_auth = true; // Must be set to `true` in all calls!
    }
    execute_transaction(
        &mut testkit,
        call.sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to deposit funds with high indirection");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletService::get_schema(&snapshot)
            .wallets
            .get(&keypair.0)
            .unwrap()
            .balance,
        30_000
    );
}

#[test]
fn test_deposit_invalid_auth() {
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

    let mut call = AnyCall::new(
        CallInfo::new(DepositService::ID, 0),
        TxIssue {
            to: keypair.0,
            amount: 10_000,
        },
    );
    // Do not set fallthrough auth, as in the previous example.
    let err = execute_transaction(
        &mut testkit,
        call.clone().sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&Error::UnauthorizedIssuer));

    call.fallthrough_auth = true;
    for i in 0..10 {
        call = AnyCall::new(CallInfo::new(AnyCallService::ID, 0), call);
        if i != 5 {
            call.fallthrough_auth = true;
        }
    }
    // Since there is no uninterrupted chain of fallthrough auth, the authorization should fail
    // for the deposit service.
    let err = execute_transaction(
        &mut testkit,
        call.clone().sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&Error::UnauthorizedIssuer));
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
        TxCreateWallet {
            name: "Alice".into(),
        }
        .sign(WalletService::ID, keypair.0, &keypair.1),
    )
    .expect("Unable to create wallet");

    let mut call = AnyCall::new(
        CallInfo::new(DepositService::ID, 0),
        TxIssue {
            to: keypair.0,
            amount: 10_000,
        },
    );
    call.fallthrough_auth = true;
    execute_transaction(
        &mut testkit,
        call.sign(AnyCallService::ID, keypair.0, &keypair.1),
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

    let mut call = AnyCall::new(
        CallInfo::new(WalletService::ID, 0),
        TxIssue {
            to: keypair.0,
            amount: 10_000,
        },
    );
    call.interface_name = "IssueReceiver".to_owned();
    let err = execute_transaction(
        &mut testkit,
        call.sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&Error::UnauthorizedIssuer).for_service(WalletService::ID)
    );
}

#[test]
fn test_any_call_err_unknown_instance() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let err = execute_transaction(
        &mut testkit,
        AnyCall::new(CallInfo::new(10_000, 0), ()).sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::IncorrectInstanceId)
    );
}

#[test]
fn test_any_call_err_unknown_interface() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let mut call = AnyCall::new(CallInfo::new(WalletService::ID, 0), ());
    call.interface_name = "FooFace".to_owned();
    let err = execute_transaction(
        &mut testkit,
        call.sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::NoSuchInterface)
    );
}

#[test]
fn test_any_call_err_unknown_method() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let mut call = AnyCall::new(
        CallInfo::new(WalletService::ID, 1),
        TxIssue {
            to: keypair.0,
            amount: 10_000,
        },
    );
    call.interface_name = "IssueReceiver".to_owned();
    let err = execute_transaction(
        &mut testkit,
        call.sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(err, ErrorMatch::from_fail(&DispatcherError::NoSuchMethod));
}

#[test]
fn test_any_call_err_wrong_arg() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    let inner_call = AnyCall::new(CallInfo::new(10_000, 0), ());
    let err = execute_transaction(
        &mut testkit,
        AnyCall::new(CallInfo::new(WalletService::ID, 0), inner_call).sign(
            AnyCallService::ID,
            keypair.0,
            &keypair.1,
        ),
    )
    .unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::MalformedArguments)
            .with_description_containing("Utf8Error")
    );
}

#[test]
fn test_any_call_panic_recursion_limit() {
    let mut testkit = testkit_with_interfaces();
    let keypair = crypto::gen_keypair();

    execute_transaction(
        &mut testkit,
        RecursiveCall { depth: 256 }.sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .expect("Call stack depth is enough");

    let err = execute_transaction(
        &mut testkit,
        RecursiveCall { depth: 257 }.sign(AnyCallService::ID, keypair.0, &keypair.1),
    )
    .unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::StackOverflow)
            .with_description_containing("Maximum depth of call stack (256)")
    );
}
