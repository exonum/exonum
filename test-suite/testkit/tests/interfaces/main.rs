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

use exonum::{
    crypto::KeyPair,
    messages::{AnyTx, Verified},
    runtime::{
        CallInfo, CommonError, CoreError, ErrorMatch, ExecutionContext, ExecutionError, SnapshotExt,
    },
};
use exonum_rust_runtime::DefaultInstance;
use exonum_testkit::{TestKit, TestKitBuilder};
use pretty_assertions::assert_eq;

use crate::{
    error::Error,
    interface::IssueReceiverMut,
    schema::{Wallet, WalletSchema},
    services::{
        AnyCall, AnyCallService, CallAny, CustomCall, CustomCallInterface, CustomCallService,
        DepositInterface, DepositService, Issue, TxIssue, WalletInterface, WalletService,
    },
};

mod error;
mod interface;
mod schema;
mod services;

fn testkit_with_interfaces() -> TestKit {
    TestKitBuilder::validator()
        .with_logger()
        .with_default_rust_service(WalletService)
        .with_default_rust_service(DepositService)
        .with_default_rust_service(AnyCallService)
        .build()
}

fn execute_transaction(testkit: &mut TestKit, tx: Verified<AnyTx>) -> Result<(), ExecutionError> {
    testkit.create_block_with_transaction(tx).transactions[0]
        .status()
        .map_err(Clone::clone)
}

#[test]
fn test_create_wallet_ok() {
    let mut testkit = testkit_with_interfaces();
    let keypair = KeyPair::random();

    execute_transaction(
        &mut testkit,
        keypair.create_wallet(WalletService::ID, "Alice".into()),
    )
    .expect("Unable to create wallet");
}

#[test]
fn test_create_wallet_fallthrough_auth() {
    let mut testkit = testkit_with_interfaces();
    let keypair = KeyPair::random();

    // Without fallthrough auth, the call should fail: `create_wallet` expects the caller
    // to be external, and it is a service.
    let mut call = AnyCall::new(CallInfo::new(WalletService::ID, 0), "Alice".to_owned());
    let err = execute_transaction(
        &mut testkit,
        keypair.call_any(AnyCallService::ID, call.clone()),
    )
    .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&Error::WrongInterfaceCaller));

    // With fallthrough auth, the call should succeed.
    call.fallthrough_auth = true;
    execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call))
        .expect("Cannot create wallet");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletService::get_schema(&snapshot)
            .wallets
            .get(&keypair.public_key())
            .unwrap()
            .balance,
        0
    );
}

#[test]
fn test_deposit_ok() {
    let mut testkit = testkit_with_interfaces();
    let keypair = KeyPair::random();

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
                to: keypair.public_key(),
                amount: 10_000,
            },
        ),
    )
    .expect("Unable to deposit wallet");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletService::get_schema(&snapshot)
            .wallets
            .get(&keypair.public_key())
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
            to: keypair.public_key(),
            amount: 10_000,
        },
    );
    call.fallthrough_auth = true;
    execute_transaction(
        &mut testkit,
        keypair.call_any(AnyCallService::ID, call.clone()),
    )
    .expect("Unable to deposit more funds");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletService::get_schema(&snapshot)
            .wallets
            .get(&keypair.public_key())
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
    execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call))
        .expect("Unable to deposit funds with high indirection");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletService::get_schema(&snapshot)
            .wallets
            .get(&keypair.public_key())
            .unwrap()
            .balance,
        30_000
    );
}

#[test]
fn test_deposit_invalid_auth() {
    let mut testkit = testkit_with_interfaces();
    let keypair = KeyPair::random();

    execute_transaction(
        &mut testkit,
        keypair.create_wallet(WalletService::ID, "Alice".into()),
    )
    .expect("Unable to create wallet");

    let mut call = AnyCall::new(
        CallInfo::new(DepositService::ID, 0),
        TxIssue {
            to: keypair.public_key(),
            amount: 10_000,
        },
    );
    // Do not set fallthrough auth, as in the previous example.
    let err = execute_transaction(
        &mut testkit,
        keypair.call_any(AnyCallService::ID, call.clone()),
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
        keypair.call_any(AnyCallService::ID, call.clone()),
    )
    .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&Error::UnauthorizedIssuer));
}

#[test]
fn test_deposit_err_issue_without_wallet() {
    let mut testkit = testkit_with_interfaces();
    let keypair = KeyPair::random();

    let err = execute_transaction(
        &mut testkit,
        keypair.deposit(
            DepositService::ID,
            TxIssue {
                to: keypair.public_key(),
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
    let keypair = KeyPair::random();

    execute_transaction(
        &mut testkit,
        keypair.create_wallet(WalletService::ID, "Alice".into()),
    )
    .expect("Unable to create wallet");

    let mut call = AnyCall::new(
        CallInfo::new(DepositService::ID, 0),
        TxIssue {
            to: keypair.public_key(),
            amount: 10_000,
        },
    );
    call.fallthrough_auth = true;
    execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call))
        .expect("Unable to deposit wallet");

    let snapshot = testkit.snapshot();
    assert_eq!(
        WalletService::get_schema(&snapshot)
            .wallets
            .get(&keypair.public_key())
            .unwrap()
            .balance,
        10_000
    );
}

#[test]
fn test_any_call_err_deposit_unauthorized() {
    let mut testkit = testkit_with_interfaces();
    let keypair = KeyPair::random();

    execute_transaction(
        &mut testkit,
        keypair.create_wallet(WalletService::ID, "Alice".into()),
    )
    .expect("Unable to create wallet");

    let mut call = AnyCall::new(
        CallInfo::new(WalletService::ID, 0),
        TxIssue {
            to: keypair.public_key(),
            amount: 10_000,
        },
    );
    call.interface_name = "IssueReceiver".to_owned();
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
    let keypair = KeyPair::random();
    let call = AnyCall::new(CallInfo::new(10_000, 0), ());
    let err =
        execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call)).unwrap_err();

    assert_eq!(err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));
}

#[test]
fn test_any_call_err_unknown_interface() {
    let mut testkit = testkit_with_interfaces();
    let keypair = KeyPair::random();

    let mut call = AnyCall::new(CallInfo::new(WalletService::ID, 0), ());
    call.interface_name = "FooFace".to_owned();
    let err =
        execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call)).unwrap_err();

    assert_eq!(err, ErrorMatch::from_fail(&CommonError::NoSuchInterface));
}

#[test]
fn test_any_call_err_unknown_method() {
    let mut testkit = testkit_with_interfaces();
    let keypair = KeyPair::random();

    let mut call = AnyCall::new(
        CallInfo::new(WalletService::ID, 1),
        TxIssue {
            to: keypair.public_key(),
            amount: 10_000,
        },
    );
    call.interface_name = "IssueReceiver".to_owned();
    let err =
        execute_transaction(&mut testkit, keypair.call_any(AnyCallService::ID, call)).unwrap_err();

    assert_eq!(err, ErrorMatch::from_fail(&CommonError::NoSuchMethod));
}

#[test]
fn test_any_call_err_wrong_arg() {
    let mut testkit = testkit_with_interfaces();
    let keypair = KeyPair::random();

    let inner_call = b"\xfe\xff".to_vec();
    let outer_call = AnyCall::new(CallInfo::new(WalletService::ID, 0), inner_call);
    let err = execute_transaction(
        &mut testkit,
        keypair.call_any(AnyCallService::ID, outer_call),
    )
    .unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&CommonError::MalformedArguments)
            .with_description_containing("invalid utf-8 sequence")
    );
}

#[test]
fn test_any_call_panic_recursion_limit() {
    let mut testkit = testkit_with_interfaces();
    let keypair = KeyPair::random();

    execute_transaction(
        &mut testkit,
        keypair.call_recursive(AnyCallService::ID, ExecutionContext::MAX_CALL_STACK_DEPTH),
    )
    .expect("Call stack depth is enough");

    let err = execute_transaction(
        &mut testkit,
        keypair.call_recursive(
            AnyCallService::ID,
            ExecutionContext::MAX_CALL_STACK_DEPTH + 1,
        ),
    )
    .unwrap_err();

    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::StackOverflow).with_description_containing(&format!(
            "Maximum depth of call stack ({})",
            ExecutionContext::MAX_CALL_STACK_DEPTH
        ))
    );
}

fn execute_custom_call(f: CustomCall) -> (TestKit, Result<(), ExecutionError>) {
    let mut testkit = TestKitBuilder::validator()
        .with_logger()
        .with_default_rust_service(WalletService)
        .with_default_rust_service(CustomCallService::new(f))
        .build();

    let keypair = KeyPair::random();
    let res = execute_transaction(
        &mut testkit,
        keypair.custom_call(CustomCallService::INSTANCE_ID, vec![]),
    );
    (testkit, res)
}

fn assert_access_error(res: Result<(), ExecutionError>) {
    assert_eq!(
        res.unwrap_err(),
        ErrorMatch::any_unexpected().with_description_containing(
            "An attempt to access blockchain data after execution error"
        )
    );
}

#[test]
fn execute_custom_call_ok() {
    let (_, res) = execute_custom_call(|context| {
        context.service_data();
        Ok(())
    });
    res.unwrap();
}

#[test]
fn child_call_error_propagated() {
    let (testkit, res) = execute_custom_call(|mut context| {
        let to = context.caller().author().unwrap();
        // Write data to blockchain.
        WalletSchema::new(context.service_data()).wallets.put(
            &to,
            Wallet {
                name: "Magic".to_string(),
                balance: 102,
            },
        );
        // Ignore child call error.
        let err = context
            .issue(WalletService::ID, Issue { to, amount: 0 })
            .unwrap_err();
        assert_eq!(err, ErrorMatch::from_fail(&Error::UnauthorizedIssuer));
        // Try to access service data.
        context.service_data();
        Ok(())
    });

    assert_access_error(res);
    // Verify that the changes made by `execute_custom_call` have been reverted.
    let snapshot = testkit.snapshot();
    let schema = WalletSchema::new(
        snapshot
            .for_service(CustomCallService::INSTANCE_NAME)
            .unwrap(),
    );
    assert_eq!(schema.wallets.values().count(), 0);
}

#[test]
fn data_inaccessible_on_child_call_error() {
    let (testkit, res) = execute_custom_call(|mut context| {
        let to = context.caller().author().unwrap();
        // Write data to blockchain.
        WalletSchema::new(context.service_data()).wallets.put(
            &to,
            Wallet {
                name: "Magic".to_string(),
                balance: 102,
            },
        );
        // Ignore child call error.
        let err = context
            .issue(WalletService::ID, Issue { to, amount: 0 })
            .unwrap_err();
        assert_eq!(err, ErrorMatch::from_fail(&Error::UnauthorizedIssuer));
        Ok(())
    });

    let err = res.unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::IncorrectCall));
    // Verify that the changes made by `execute_custom_call` have been reverted.
    let snapshot = testkit.snapshot();
    let schema = WalletSchema::new(
        snapshot
            .for_service(CustomCallService::INSTANCE_NAME)
            .unwrap(),
    );
    assert_eq!(schema.wallets.values().count(), 0);
}

#[test]
fn custom_call_err_incorrect_instance_id() {
    let (testkit, res) = execute_custom_call(|mut context| {
        let to = context.caller().author().unwrap();
        // Write data to blockchain.
        WalletSchema::new(context.service_data()).wallets.put(
            &to,
            Wallet {
                name: "Magic".to_string(),
                balance: 102,
            },
        );
        // Ignore child call error.
        let err = context
            .issue(WalletService::ID + 1, Issue { to, amount: 0 })
            .unwrap_err();
        assert_eq!(err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));
        // Try to access blockchain data.
        context.data();
        Ok(())
    });

    res.expect("Blockchain data must be accessible");
    // Verify that the changes made by `execute_custom_call` have been written.
    let snapshot = testkit.snapshot();
    let schema = WalletSchema::new(
        snapshot
            .for_service(CustomCallService::INSTANCE_NAME)
            .unwrap(),
    );
    assert_eq!(
        schema.wallets.values().collect::<Vec<_>>(),
        vec![Wallet {
            name: "Magic".to_string(),
            balance: 102,
        }]
    );
}
